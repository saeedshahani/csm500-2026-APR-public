"""Bounded hybrid Bellman-component adapter. No native arithmetic claim.

All public calls are serialized. The task/checker, session registry and sampled
input stream are trusted. Digests bind content; they are NOT signatures.
"""
from dataclasses import dataclass
from fractions import Fraction as Q
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import threading
import time

from domain import (Action, Meter, action, bellman, boundary_lipschitz,
                    certify, gaussian, synthesize, upper, validate_task)
from graph import rule
from itertools import product

MAX_BITS = 128
MAX_STEPS = 4096
MAX_RECEIPTS = 64
MAX_SOURCE = 32768
MAX_TRACE = 2_000_000


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True)


def digest(value):
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def fraction(value):
    """Strict bounded external rational parser; reject floats and huge integers."""
    if type(value) is Q:
        result = value
    elif type(value) is int:
        if value.bit_length() > MAX_BITS:
            raise ValueError('rational input cap')
        result = Q(value)
    elif type(value) is str and len(value) <= 80:
        try:
            result = Q(value)
        except (ValueError, ZeroDivisionError):
            raise ValueError('invalid rational') from None
    else:
        raise ValueError('rational must be bounded exact data')
    if max(abs(result.numerator).bit_length(), result.denominator.bit_length()) > MAX_BITS:
        raise ValueError('rational input cap')
    return result


def natural(value, maximum=1_000_000_000):
    if type(value) is not int or not 0 <= value <= maximum:
        raise ValueError('integer outside contract')
    return value


def task_data(task):
    return [[{'weights': list(map(str, a.weights)), 'reward': str(a.reward),
              'boundary': str(a.boundary)} for a in row] for row in task]


def checked_task(data):
    if type(data) not in (list, tuple) or not 1 <= len(data) <= 4:
        raise ValueError('one to four states required')
    rows = []
    policies = 1
    for row in data:
        if type(row) not in (list, tuple) or not 1 <= len(row) <= 16:
            raise ValueError('action cap')
        policies *= len(row)
        if policies > 16:
            raise ValueError('policy cap')
        actions = []
        for a in row:
            if type(a) is not dict or set(a) != {'weights', 'reward', 'boundary'}:
                raise ValueError('action schema')
            if type(a['weights']) not in (tuple, list) or len(a['weights']) != len(data):
                raise ValueError('weight dimension')
            actions.append(Action(tuple(map(fraction, a['weights'])), fraction(a['reward']),
                                  fraction(a['boundary'])))
        rows.append(tuple(actions))
    task = tuple(rows)
    if not validate_task(task):
        raise ValueError('invalid substochastic task')
    return task


def rounded(values, bits):
    scale = 1 << bits
    return tuple(Q(v.numerator * scale // v.denominator, scale) for v in values)


def solve(task, p, mode, steps, bits):
    natural(steps, MAX_STEPS)
    if type(bits) is not int or not 4 <= bits <= 64:
        raise ValueError('precision cap')
    meter = Meter()
    start = time.monotonic()
    if mode == 'raw':
        y = tuple(Q(0) for _ in task)
        for _ in range(steps):
            y = rounded(bellman(task, y, p, meter), bits)
    elif mode == 'compiled':
        program = synthesize(task, meter)
        if not certify(task, program, meter)[0]:
            raise ValueError('compiler certificate rejected')
        y = rounded(tuple(upper(row, p, meter) for row in program), bits)
    elif mode == 'direct':
        # Direct policy solving, independent of the compiled envelope construction.
        y = None
        n = len(task)
        for policy in product(*(range(len(row)) for row in task)):
            selected = [task[i][policy[i]] for i in range(n)]
            matrix = [[Q(i == j)-selected[i].weights[j] for j in range(n)] for i in range(n)]
            candidate = tuple(row[0] for row in gaussian(matrix,
                [(a.reward+a.boundary*p,) for a in selected], meter))
            if bellman(task, candidate, p, meter) == candidate:
                y = rounded(candidate, bits)
                break
        if y is None:
            raise ValueError('no optimal policy found')
    else:
        raise ValueError('unknown solver')
    return y, {'mode': mode, 'steps_requested': steps, 'bits': bits,
               'rational_ops': meter.ops, 'seconds': time.monotonic()-start}


def residual_radius(task, p, values):
    """Banach residual bound. Exact recomputation includes all output rounding."""
    if len(values) != len(task) or any(not 0 <= y <= 1 for y in values):
        raise ValueError('output outside task cube')
    q = max(sum(a.weights) for row in task for a in row)
    residual = max(abs(v-y) for v, y in zip(bellman(task, values, p), values))
    return min(Q(1), residual/(1-q)), residual, q


@dataclass(frozen=True)
class Receipt:
    content: str
    key: str

    def data(self):
        return json.loads(self.content)


class Component:
    """One trusted in-process owner. A new process requires fresh computation.

    Departure/revision invalidates prior receipts. Rejoining creates a new epoch.
    A contract fault latches until explicit reconfiguration/rejoin.
    """
    def __init__(self, identity, task, boundary='0', tick=0):
        if type(identity) is not str or not identity or len(identity) > 64:
            raise ValueError('identity cap')
        self._lock = threading.RLock()
        self.identity = identity
        self.epoch = 0
        self.revision = 0
        self.active = True
        self.fault = None
        self.registry = {}
        self.sequence = 0
        self.variation = Q(0)
        self.tick = natural(tick)
        self.task = checked_task(task)
        self.p = fraction(boundary)
        if not 0 <= self.p <= 1:
            raise ValueError('boundary outside cube')

    def observe(self, boundary, sequence, tick, declared_movement=None):
        with self._lock:
            if not self.active or self.fault:
                raise ValueError('inactive or violated contract')
            p = fraction(boundary)
            natural(sequence); natural(tick)
            movement = abs(p-self.p)
            declared = movement if declared_movement is None else fraction(declared_movement)
            if not 0 <= p <= 1 or sequence != self.sequence+1 or tick < self.tick or declared < movement:
                self.fault = 'observation_contract_violation'
                raise ValueError(self.fault)
            # Sum is exact and privately computed; a caller cannot lower this ledger.
            self.variation += movement
            if self.variation.numerator.bit_length() > 8192 or self.variation.denominator.bit_length() > 8192:
                self.fault = 'variation_resource_cap'
                raise ValueError(self.fault)
            self.p, self.sequence, self.tick = p, sequence, tick

    def revise(self, task, boundary, tick):
        with self._lock:
            replacement = checked_task(task)
            p = fraction(boundary)
            natural(tick)
            if not 0 <= p <= 1 or tick < self.tick:
                raise ValueError('invalid revision snapshot')
            self.task, self.p, self.tick = replacement, p, tick
            self.revision += 1
            self.sequence, self.variation, self.fault = 0, Q(0), None

    def depart(self):
        with self._lock:
            self.active = False

    def rejoin(self, boundary, tick):
        with self._lock:
            if self.active:
                raise ValueError('departure required')
            self.revise(task_data(self.task), boundary, tick)
            self.epoch += 1
            self.active = True

    def compute(self, mode='raw', steps=128, bits=24):
        with self._lock:
            if not self.active or self.fault:
                raise ValueError('inactive or violated contract')
            if len(self.registry) >= MAX_RECEIPTS:
                raise ValueError('receipt retention cap; create a new component session')
            y, cost = solve(self.task, self.p, mode, steps, bits)
            radius, residual, q = residual_radius(self.task, self.p, y)
            data = {'schema': 't2-receipt-v1', 'identity': self.identity,
                    'epoch': self.epoch, 'revision': self.revision,
                    'task': task_data(self.task), 'task_digest': digest(task_data(self.task)),
                    'boundary': str(self.p), 'sequence': self.sequence, 'tick': self.tick,
                    'variation': str(self.variation), 'values': list(map(str, y)),
                    'radius': str(radius), 'residual': str(residual), 'q': str(q),
                    'mode': mode, 'bits': bits, 'steps': steps}
            content = canonical(data)
            receipt = Receipt(content, digest(data))
            self.registry[receipt.key] = content
            return receipt, cost

    def assess(self, receipt, tolerance, now, max_age=32):
        with self._lock:
            eps = fraction(tolerance)
            natural(now); natural(max_age)
            if not 0 <= eps <= 1:
                raise ValueError('tolerance outside cube')
            if now < self.tick:
                self.fault = 'clock_regression'
                raise ValueError(self.fault)
            self.tick = now
            # Registry comparison is origin authentication within this process only.
            known = (type(receipt) is Receipt and self.registry.get(receipt.key) == receipt.content)
            if not known:
                raise ValueError('unknown, evicted, stale-version or tampered receipt')
            data = receipt.data()
            if (not self.active or self.fault or data['revision'] != self.revision
                    or data['epoch'] != self.epoch or data['task_digest'] != digest(task_data(self.task))):
                status, radius = 'stale', None
            elif now-data['tick'] > max_age:
                status, radius = 'stale', None
            else:
                values = tuple(map(Q, data['values']))
                radius, _, _ = residual_radius(self.task, Q(data['boundary']), values)
                radius = min(Q(1), radius+boundary_lipschitz(self.task)*(self.variation-Q(data['variation'])))
                status = 'certified' if radius <= eps else 'uncertified'
            return {'schema': 't2-publication-v1', 'receipt': data, 'receipt_digest': receipt.key,
                    'status': status, 'radius_now': None if radius is None else str(radius),
                    'tolerance': str(eps), 'checked_at': now, 'max_age': max_age,
                    'observed_sequence': self.sequence, 'observed_variation': str(self.variation),
                    'current': {'epoch': self.epoch, 'revision': self.revision,
                                'task_digest': digest(task_data(self.task)), 'boundary': str(self.p),
                                'active': self.active, 'fault': self.fault},
                    'assumptions': ['complete sampled input stream', 'protected task and host checker',
                                    'serialized host calls', 'logical clock', 'checked native CLI']}

    def publish(self, receipt, tolerance, now, cli, out, max_age=32):
        with self._lock:
            packet = self.assess(receipt, tolerance, now, max_age)
            return native_publish(packet, cli, out)


def publication_source(packet):
    """One graph step transports the exact checked packet, as an opaque value."""
    payload = canonical(packet)
    if len(payload.encode()) > 8192:
        raise ValueError('packet byte cap')
    label = packet['status']
    if label not in ('certified', 'uncertified', 'stale'):
        raise ValueError('publication status')
    atoms = {'p': 'Constructor("T2Component")',
             'v': 'PatternVariable[Constructor] Free',
             'pending': 'Record[ObjectLink]("pending",p,v)',
             'done': f'Record[ObjectLink]("{label}",p,v)'}
    body = rule('publish', atoms, ['p', 'v', 'pending'], ['p', 'v', 'done'])
    source = ('program {\n'+body+'\natom component = Constructor("T2Component");\n'
              +'atom value = Constructor('+json.dumps(payload)+');\n'
              +'atom pending = Record[ObjectLink]("pending",component,value);\n}\n')
    if len(source.encode()) > MAX_SOURCE:
        raise ValueError('source byte cap')
    return source


def verify_packet(packet):
    """Recompute mathematical certificate from exported data, not provenance.

    A forged, internally consistent task/context still needs the owning host's
    authorization. trace-check likewise does not authenticate a physical sensor.
    """
    d, current = packet['receipt'], packet['current']
    if packet['schema'] != 't2-publication-v1' or d['schema'] != 't2-receipt-v1':
        raise ValueError('certificate schema')
    for value in (d['boundary'], current['boundary'], packet['tolerance']):
        if not 0 <= fraction(value) <= 1:
            raise ValueError('certificate outside task cube')
    for value in (packet['checked_at'], packet['max_age'], packet['observed_sequence'],
                  d['tick'], d['sequence'], d['revision'], d['epoch'],
                  current['revision'], current['epoch']):
        natural(value)
    if packet['observed_sequence'] < d['sequence'] and current['revision'] == d['revision']:
        raise ValueError('observation sequence regression')
    if Q(d['variation']) < 0 or Q(packet['observed_variation']) < 0:
        raise ValueError('negative variation')
    if digest(d) != packet['receipt_digest'] or digest(d['task']) != d['task_digest']:
        raise ValueError('receipt digest mismatch')
    task = checked_task(d['task'])
    values = tuple(map(fraction, d['values']))
    original, residual, q = residual_radius(task, fraction(d['boundary']), values)
    if (Q(d['radius']), Q(d['residual']), Q(d['q'])) != (original, residual, q):
        raise ValueError('incorrect original bound')
    stale = (not current['active'] or current['fault'] is not None
             or any(d[key] != current[key] for key in ('epoch', 'revision', 'task_digest'))
             or packet['checked_at']-d['tick'] > packet['max_age'])
    if packet['checked_at'] < d['tick']:
        raise ValueError('publication before computation')
    if stale:
        if packet['status'] != 'stale' or packet['radius_now'] is not None:
            raise ValueError('stale context promoted')
        return
    movement = Q(packet['observed_variation'])-Q(d['variation'])
    if movement < abs(Q(current['boundary'])-Q(d['boundary'])):
        raise ValueError('movement does not cover observed endpoints')
    radius = min(Q(1), original+boundary_lipschitz(task)*movement)
    status = 'certified' if radius <= fraction(packet['tolerance']) else 'uncertified'
    if Q(packet['radius_now']) != radius or packet['status'] != status:
        raise ValueError('incorrect publication bound or decision')


def native_publish(packet, cli, out):
    """Not a public authentication service: caller must run Component.assess first."""
    out = Path(out)
    out.mkdir(exist_ok=False, parents=True)
    verify_packet(packet)
    source = publication_source(packet)
    (out/'packet.json').write_text(json.dumps(packet, indent=2)+'\n')
    (out/'publication.graph').write_text(source)
    start = time.monotonic()
    commands = [[str(cli), 'export', '-', '--max-steps', '2', '--max-work', '1000000'],
                [str(cli), 'trace-check', '-'], [str(cli), 'replay', '-']]
    trace_text = None
    for index, command in enumerate(commands):
        run = subprocess.run(command, input=source if index == 0 else trace_text,
                             text=True, capture_output=True, timeout=30)
        (out/f'command-{index}.json').write_text(json.dumps({'argv': command,
            'exit': run.returncode, 'stderr': run.stderr}, indent=2)+'\n')
        if run.returncode:
            raise RuntimeError(f'native command failed: {run.stderr}')
        if len(run.stdout.encode()) > MAX_TRACE:
            raise ValueError('trace/replay output byte cap')
        if index == 0:
            trace_text = run.stdout
            (out/'trace.json').write_text(trace_text)
        elif index == 2:
            (out/'replayed.graph').write_text(run.stdout)
    trace = json.loads(trace_text)
    assert len(trace['events']) == 1
    initial = trace['initial']['atoms']
    final = trace['checkpoints'][-1]['graph']['atoms']
    ids = [key for key, atom in initial.items() if atom['kind'] == {'Constructor': canonical(packet)}]
    assert len(ids) == 1
    value_id = ids[0]
    assert final[value_id] == initial[value_id]
    links = [a for a in final.values() if isinstance(a['kind'], dict) and 'Record' in a['kind']
             and a['kind']['Record']['role'] == 'ObjectLink' and len(a['fields']) == 2
             and a['fields'][1] == value_id]
    assert len(links) == 1 and links[0]['kind']['Record']['label'] == packet['status']
    assert int(trace['checkpoints'][-1]['state']) == 1
    result = {'status': packet['status'], 'source_bytes': len(source.encode()),
              'trace_bytes': len(trace_text.encode()), 'atoms': len(final), 'events': 1,
              'trace_checked': True, 'replayed': True, 'seconds': time.monotonic()-start,
              'cli_sha256': hashlib.sha256(Path(cli).read_bytes()).hexdigest()}
    (out/'result.json').write_text(json.dumps(result, indent=2)+'\n')
    return result


def main():
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request', type=Path)
    parser.add_argument('--cli', required=True)
    parser.add_argument('--out', required=True)
    args = parser.parse_args()
    if args.request.stat().st_size > MAX_SOURCE:
        raise ValueError('request byte cap')
    request = json.loads(args.request.read_text())
    expected = {'identity', 'task', 'boundary', 'mode', 'steps', 'bits', 'tolerance'}
    if type(request) is not dict or set(request) != expected:
        raise ValueError('request schema')
    component = Component(request['identity'], request['task'], request['boundary'])
    receipt, cost = component.compute(request['mode'], request['steps'], request['bits'])
    result = component.publish(receipt, request['tolerance'], 0, args.cli, args.out)
    result['solver_cost'] = cost
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
