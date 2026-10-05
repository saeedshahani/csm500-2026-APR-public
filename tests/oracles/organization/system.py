"""Test oracle: prediction, checking, renewal and inheritance.

No analysis module, target code, external fitness function or correctness oracle
is imported here. Fixed primitive meanings, sensing, production affordances and
generic repair/reference programs remain declared operational assumptions.
"""
import copy
import math
from collections import deque

MASK = (1 << 64) - 1


def draw(state, n):
    assert n > 0
    limit = (1 << 64) - (1 << 64) % n
    while True:
        x = state['rng']
        x ^= (x << 13) & MASK
        x ^= x >> 7
        x ^= (x << 17) & MASK
        state['rng'] = x & MASK
        if state['rng'] < limit:
            return state['rng'] % n


def bits(word, length):
    return [(word >> (length - 1 - i)) & 1 for i in range(length)]


def fresh(seed=810001, n=64, rooms=8, length=4, recognition='learned',
          energy=18000, source=200000, packet=256, symbol_factor=1,
          initial='uniform', columns=None):
    if type(seed) is not int or not 0 < seed <= MASK:
        raise ValueError('seed must be a nonzero unsigned 64-bit integer')
    for value, low, high, name in ((n, 1, 1024, 'capacity'), (rooms, 3, 32, 'rooms'),
                                  (length, 3, 10, 'sequence length')):
        if type(value) is not int or not low <= value <= high:
            raise ValueError(f'{name} must be in {low}..{high}')
    if recognition not in ('learned', 'fixed', 'random', 'generic') or initial not in ('uniform', 'core', 'vacant'):
        raise ValueError('unsupported initialization or recognition mode')
    if any(type(v) is not int or v < 0 for v in (energy, source, packet)):
        raise ValueError('resource quantities must be nonnegative integers')
    if not math.isfinite(symbol_factor) or not 0 <= symbol_factor <= 100:
        raise ValueError('symbol factor must be finite and in 0..100')
    words = 1 << length
    root = {'tag': 'organization', 'rng': seed, 'epoch': 0, 'attempts': 0,
            'n': n, 'length': length, 'energy': energy, 'source': source,
            'initial_energy': energy + source, 'spent_energy': 0,
            'packet': packet, 'recognition': recognition,
            'rates': [100, 400, 5, 5, 10], 'basal': 1,
            'feedback': True, 'learn': True, 'use_model': True,
            'renew_checkers': True, 'independent_check': True,
            'revise_recognition': True, 'background_until': 500,
            'pools': {'template': [None] * n, 'sequence': [None] * n},
            'ages': {'template': [None] * n, 'sequence': [None] * n},
            'reserves': {pool: [int(n * length * symbol_factor)] * 2
                         for pool in ('template', 'sequence')},
            'initial_symbols': {}, 'programs': [None] * n,
            'created': {'template': 0, 'sequence': 0, 'program': 0},
            'removed': {'template': 0, 'sequence': 0, 'program': 0},
            'next_program': 0, 'model': {}, 'production': {}, 'production_order': [],
            'clock_id': [seed],
            'recognition_updates': 0, 'model_updates': 0,
            'detected_errors': 0, 'checker_disagreements': 0,
            'checker_repairs': 0, 'checked': 0, 'harvests': 0,
            'blocked_energy': 0, 'blocked_symbols': 0,
            'actions': 0, 'history': [], 'events': [], 'event_cursor': 0,
            'ancestry': None}
    g = {'root': root,
         'blueprint:0': {'tag': 'program_template', 'instructions': ['equal']},
         'blueprint:1': {'tag': 'program_template', 'instructions': ['difference']},
         'repair:0': {'tag': 'repair_reference', 'instructions': ['equal']}}
    columns = list(range(length)) if columns is None else columns
    assert sorted(columns) == list(range(length))
    permute = lambda w: sum(bits(w, length)[columns[i]] << (length - 1 - i)
                            for i in range(length))
    core = [permute(1), permute(3)]
    embedding = {core[0]: (0, 0), core[1]: (1, 1),
                 (words - 1) ^ core[0]: (0, 1), (words - 1) ^ core[1]: (1, 0)}
    root['core'] = core  # Used by the read-only observer and fixed reference only.
    for word in range(words):
        g[f't{word}'] = {'tag': 'template', 'sequence': bits(word, length), 'count': 0}
        g[f'p{word}'] = {'tag': 'sequence', 'sequence': bits(word, length), 'count': 0}
        if word in embedding:
            c, a = embedding[word]
            targets = ([f't{core[1-c]}'] if recognition == 'fixed' else
                       [f't{i}' for i in range(words)] if recognition == 'generic' else
                       [f't{draw(root, words)}'])
            g[f'p{word}'].update({'input_symbol': c, 'output_symbol': a,
                                  'recognizes': targets})
    root['operators'] = [name for name, node in g.items() if 'input_symbol' in node]
    # Opaque room names and permuted physical action labels. The learner reads
    # these transitions only by performing actions; it starts with no model.
    order = list(range(rooms))
    for i in range(rooms - 1, 0, -1):
        j = draw(root, i + 1)
        order[i], order[j] = order[j], order[i]
    flip = draw(root, 2)
    for i, room in enumerate(order):
        g[f'e{room}'] = {'tag': 'environment_state', 'next': {
            str(flip): f'e{order[(i+1) % rooms]}',
            str(1-flip): f'e{order[(i-1) % rooms]}'}}
    g['environment'] = {'tag': 'environment', 'states': [f'e{i}' for i in range(rooms)],
                         'at': f'e{order[0]}', 'target': f'e{order[rooms//2]}',
                         'version': 0}
    vm = System(g)
    if initial != 'vacant':
        for i in range(n):
            word = core[i % 2] if initial == 'core' else i % words
            vm.replace('template', i, f't{word}', initialization=True)
            vm.replace('sequence', i, 'p0', initialization=True)
    root = vm.r
    root['initial_symbols'] = vm.inventory()
    return vm.g


class System:
    def __init__(self, graph):
        self.g = copy.deepcopy(graph)

    @property
    def r(self):
        return self.g['root']

    def inventory(self):
        result = {}
        for pool in ('template', 'sequence'):
            total = self.r['reserves'][pool][:]
            for name in self.r['pools'][pool]:
                if name is not None:
                    for a in self.g[name]['sequence']:
                        total[a] += 1
            result[pool] = total
        return result

    def pay(self, amount=1):
        if self.r['energy'] < amount:
            self.r['blocked_energy'] += 1
            return False
        self.r['energy'] -= amount
        self.r['spent_energy'] += amount
        return True

    def weights(self):
        w = [[self.r['basal']] * 2 for _ in range(2)]
        if self.r['feedback']:
            for name in self.r['operators']:
                node = self.g[name]
                w[node['input_symbol']][node['output_symbol']] += node['count']
        return w

    def replace(self, pool, position, name, initialization=False):
        r = self.r
        old = r['pools'][pool][position]
        remaining = r['reserves'][pool][:]
        if old is not None:
            for a in self.g[old]['sequence']:
                remaining[a] += 1
        if name is not None:
            for a in self.g[name]['sequence']:
                remaining[a] -= 1
        if min(remaining) < 0:
            r['blocked_symbols'] += 1
            return False
        r['reserves'][pool] = remaining
        if old is not None:
            self.g[old]['count'] -= 1
            r['removed'][pool] += 1
        r['pools'][pool][position] = name
        r['ages'][pool][position] = None
        if pool == 'sequence' and r['programs'][position] is not None:
            del self.g[r['programs'][position]]
            r['programs'][position] = None
            r['removed']['program'] += 1
        if name is not None:
            self.g[name]['count'] += 1
            r['ages'][pool][position] = 0 if initialization else r['attempts'] + 1
            r['created'][pool] += 1
            node = self.g[name]
            if pool == 'sequence' and 'input_symbol' in node and r['renew_checkers']:
                role = node['input_symbol']
                key = f'program:{r["next_program"]}'
                r['next_program'] += 1
                self.g[key] = {'tag': 'program_instance', 'role': role,
                               'instructions': self.g[f'blueprint:{role}']['instructions'][:],
                               'created': r['attempts'] + 1, 'operator': name}
                r['programs'][position] = key
                r['created']['program'] += 1
        return True

    def population_step(self):
        r = self.r
        rates = r['rates'][:]
        if r['epoch'] >= r['background_until']:
            rates[4] = 0
        z = draw(r, sum(rates)); ch = 0
        while z >= rates[ch]:
            z -= rates[ch]; ch += 1
        r['attempts'] += 1
        n = r['n']; words = 1 << r['length']
        # Removal remains possible at zero energy: resource exhaustion cannot
        # manufacture apparent stability by freezing existing live instances.
        if ch in (2, 3):
            self.replace('template' if ch == 2 else 'sequence', draw(r, n), None)
            return
        if not self.pay():
            return
        if ch == 0:
            source = r['pools']['template'][draw(r, n)]
            if source is None:
                return
            weights = self.weights(); product = 0
            for c in self.g[source]['sequence']:
                a = int(draw(r, sum(weights[c])) >= weights[c][0])
                product = 2 * product + a
            name = f'p{product}'
            success = self.replace('sequence', draw(r, n), name)
            # This is an observed construction event, not a lookup of desired
            # assignments. Unproductive products remain in the observations.
            if success:
                if source not in r['production']:
                    r['production_order'].append(source)
                log = r['production'].setdefault(source, [])
                log.append(self.g[name].get('input_symbol', -1))
                del log[:-128]
        elif ch == 1:
            source = r['pools']['template'][draw(r, n)]
            operator = r['pools']['sequence'][draw(r, n)]
            if source is not None and operator is not None:
                if source in self.g[operator].get('recognizes', []):
                    self.replace('template', draw(r, n), source)
        else:
            self.replace('template', draw(r, n), f't{draw(r, words)}')

    def programs(self):
        out = {0: [], 1: []}
        for key in self.r['programs']:
            if key is not None:
                out[self.g[key]['role']].append(key)
        return out

    def run_checker(self, key, prediction, observed):
        result = None
        for opcode in self.g[key]['instructions']:
            if opcode == 'equal':
                result = prediction == observed
            elif opcode == 'difference':
                # Separate implementation of the negative test, using a
                # singleton symmetric difference on opaque observations.
                result = not ({prediction} ^ {observed})
            elif opcode == 'accept':
                result = True
            elif opcode == 'reject':
                result = False
            else:
                raise ValueError('unsupported resident instruction')
        return result

    def revise_recognition(self, programs):
        r = self.r
        if not r['revise_recognition'] or r['recognition'] in ('fixed', 'generic'):
            return
        if not programs[0] or not programs[1]:
            return
        for name in r['operators']:
            node = self.g[name]
            # Complementary functional dependency is a supplied generic law;
            # which template produces the needed role is learned from events.
            needed = 1 - node['input_symbol']
            if r['recognition'] == 'random':
                target = f't{draw(r, 1 << r["length"])}'
            else:
                choices = [(sum(x == needed for x in log) + 1, len(log) + 2, t)
                           for t in r['production_order'] for log in [r['production'][t]]
                           if len(log) >= 12 and self.g[t]['count'] > 0]
                if not choices:
                    continue
                # Deterministic rational comparison; ties sampled uniformly.
                best = []
                for hit, total, target in choices:
                    if not best or hit * best[0][1] > best[0][0] * total:
                        best = [(hit, total, target)]
                    elif hit * best[0][1] == best[0][0] * total:
                        best.append((hit, total, target))
                target = best[draw(r, len(best))][2]
                current = node['recognizes'][0]
                old_log = r['production'].get(current, [])
                # Hysteresis prevents changes caused by one noisy observation.
                if len(old_log) >= 12 and self.g[current]['count'] > 0:
                    old_hits = sum(x == needed for x in old_log) + 1
                    hit, total, _ = next(v for v in best if v[2] == target)
                    if 20 * hit * (len(old_log) + 2) <= (20 * old_hits + len(old_log) + 2) * total:
                        target = current
            if node['recognizes'] != [target]:
                r['recognition_updates'] += 1
                node['recognizes'] = [target]

    def plan(self, start, target):
        model = self.r['model']
        queue = deque([(start, [])]); seen = {start}
        while queue:
            state, path = queue.popleft()
            if state == target:
                return path
            for action in (0, 1):
                record = model.get(state + ':' + str(action))
                if record is None:
                    continue
                nxt = record['target']
                if nxt not in seen:
                    seen.add(nxt); queue.append((nxt, path + [action]))
        return None

    def act(self, programs):
        r = self.r; env = self.g['environment']
        if not self.pay():
            return
        before = env['at']; target = env['target']
        path = self.plan(before, target) if r['use_model'] and programs[0] and programs[1] else None
        unseen = [a for a in (0, 1) if before + ':' + str(a) not in r['model']]
        action = (unseen[draw(r, len(unseen))] if unseen else
                  path[0] if path else draw(r, 2))
        if not unseen and r['epoch'] % 4 == 0:
            action = min((0, 1), key=lambda a: r['model'][before + ':' + str(a)].get('last_checked', -1))
        key = before + ':' + str(action)
        claim = r['model'].get(key)
        predicted = claim['target'] if claim else None
        # Prediction is committed before accessing the environment transition.
        pending = {'from': before, 'action': action, 'predicted': predicted,
                   'epoch': r['epoch']}
        weights = self.weights(); row = weights[action]
        realized = row.index(max(row)) if row[0] != row[1] else draw(r, 2)
        after = self.g[before]['next'][str(realized)]
        env['at'] = after
        witness = dict(pending, observed=after, realized_action=realized)
        primary = secondary = None
        if programs[0]:
            primary = self.run_checker(programs[0][0], predicted, after)
        if programs[1] and r['independent_check']:
            secondary = self.run_checker(programs[1][0], predicted, after)
        if primary is not None and (secondary is not None or not r['independent_check']):
            agreed = secondary is None or primary == secondary
            if not agreed:
                r['checker_disagreements'] += 1
                # The repair reference is resident and supplied, not an oracle
                # that inspects the hidden environment transition table.
                replacement = self.g['repair:0']['instructions'][:]
                self.g['blueprint:0']['instructions'] = replacement
                for program in programs[0]:
                    self.g[program]['instructions'] = replacement[:]
                r['checker_repairs'] += 1
            else:
                r['checked'] += 1
                if predicted is not None and not primary:
                    r['detected_errors'] += 1
                if r['learn'] and (predicted is None or not primary):
                    r['model'][key] = {'from': before, 'action': action, 'target': after,
                                       'witness_epoch': r['epoch'], 'witness_clock': r['clock_id'][:],
                                       'last_checked': r['epoch'], 'revision': 1 + (claim or {}).get('revision', 0)}
                    r['model_updates'] += 1
                elif claim is not None and primary:
                    claim['last_checked'] = r['epoch']
        witness.update({'primary': primary, 'secondary': secondary,
                        'energy_before_collection': r['energy']})
        if after == target and r['source']:
            amount = min(r['packet'], r['source'])
            r['source'] -= amount; r['energy'] += amount; r['harvests'] += 1
            choices = [x for x in env['states'] if x != target]
            env['target'] = choices[draw(r, len(choices))]
            witness['collected'] = amount
        r['actions'] += 1
        r['history'].append(witness)

    def intervene(self, event):
        r = self.r; kind = event['operation']
        if kind == 'environment_swap':
            for name in self.g['environment']['states']:
                node = self.g[name]; node['next']['0'], node['next']['1'] = node['next']['1'], node['next']['0']
            self.g['environment']['version'] += 1
        elif kind == 'checker_fault':
            roles = (0, 1) if event.get('common_mode') else (0,)
            for role in roles:
                self.g[f'blueprint:{role}']['instructions'] = ['accept']
                for key in self.programs()[role]:
                    self.g[key]['instructions'] = ['accept']
        elif kind == 'recognition_scramble':
            for name in r['operators']:
                self.g[name]['recognizes'] = [f't{draw(r, 1 << r["length"])}']
        elif kind == 'set':
            r[event['field']] = copy.deepcopy(event['value'])
        elif kind == 'erase_sequences':
            for i in range(r['n']):
                self.replace('sequence', i, None)
        else:
            raise ValueError(kind)

    def tick(self, attempts=None):
        r = self.r
        while r['event_cursor'] < len(r['events']):
            event = r['events'][r['event_cursor']]
            if event['at'] > r['epoch']:
                break
            self.intervene(event); r['event_cursor'] += 1
        for _ in range(r['n'] if attempts is None else attempts):
            self.population_step()
        programs = self.programs()
        self.revise_recognition(programs)
        self.act(programs)
        r['epoch'] += 1
        assert r['energy'] + r['source'] + r['spent_energy'] == r['initial_energy']
        return self.g

    def run(self, epochs):
        for _ in range(epochs):
            self.tick()
        return self.g


def inherit(parent, seed, bottleneck=32, transfer_model=True, mode='both', energy=4000):
    """A supplied child-allocation operation, with explicit finite transfers.

    The returned world retains both subgraphs. Population references, symbol
    inventories, energy and finite source stock are accounted across them.
    Model records are copied separately from population bottlenecks and counted.
    """
    if type(seed) is not int or not 0 < seed <= MASK:
        raise ValueError('seed must be a nonzero unsigned 64-bit integer')
    if type(bottleneck) is not int or not 0 <= bottleneck <= parent['root']['n']:
        raise ValueError('bottleneck must lie within the population capacity')
    if mode not in ('both', 'template', 'sequence', 'none') or type(energy) is not int or energy < 0:
        raise ValueError('invalid inheritance mode or energy allocation')
    p = System(parent); pr = p.r
    child = copy.deepcopy(parent); c = System(child); cr = c.r
    for pool in ('template', 'sequence'):
        for i in range(cr['n']):
            c.replace(pool, i, None)
    # Allocate no duplicate symbol stock: reserves are moved from the parent.
    for pool in ('template', 'sequence'):
        cr['reserves'][pool] = [x // 2 for x in pr['reserves'][pool]]
        pr['reserves'][pool] = [x-y for x,y in zip(pr['reserves'][pool],cr['reserves'][pool])]
    cr['rng'] = seed; cr['epoch'] = 0; cr['attempts'] = 0
    cr['model'] = copy.deepcopy(pr['model']) if transfer_model else {}
    cr['clock_id'] = pr['clock_id'] + [seed]
    for record in cr['model'].values():
        # Preserve validation AGE across the reset. Witness epochs continue to
        # identify the original lineage clock, including across generations.
        record['last_checked'] = record.get('last_checked', -1) - pr['epoch']
    cr['history'] = []; cr['events'] = []; cr['event_cursor'] = 0
    cr['background_until'] = 0
    for key in ('recognition_updates', 'model_updates', 'detected_errors', 'checked',
                'checker_disagreements', 'checker_repairs', 'harvests',
                'blocked_energy', 'blocked_symbols', 'actions'):
        cr[key] = 0
    cr['created'] = {'template': 0, 'sequence': 0, 'program': 0}
    cr['removed'] = {'template': 0, 'sequence': 0, 'program': 0}
    for pool in ('template', 'sequence'):
        enabled = mode == 'both' or mode == pool
        if not enabled:
            continue
        positions = list(range(pr['n']))
        for j in range(bottleneck):
            k = j + draw(cr, len(positions) - j)
            positions[j], positions[k] = positions[k], positions[j]
        for j, pos in enumerate(positions[:bottleneck]):
            name = pr['pools'][pool][pos]
            if name is None:
                continue
            # Transfer the instance plus its bound symbols, without spending
            # a construction step or importing symbol stock from outside.
            for a in p.g[name]['sequence']:
                cr['reserves'][pool][a] += 1
            assert c.replace(pool, j, name, initialization=True)
            for a in p.g[name]['sequence']:
                pr['reserves'][pool][a] -= 1
            p.replace(pool, pos, None)
    cr['energy'] = min(energy, pr['energy']); pr['energy'] -= cr['energy']
    cr['source'] = pr['source'] // 2; pr['source'] -= cr['source']
    cr['spent_energy'] = 0
    cr['initial_energy'] = cr['energy'] + cr['source']
    pr['initial_energy'] -= cr['initial_energy']
    cr['initial_symbols'] = c.inventory(); pr['initial_symbols'] = p.inventory()
    cr['ancestry'] = {'parent_epoch': pr['epoch'], 'bottleneck': bottleneck,
                       'model_records': len(cr['model']), 'mode': mode}
    return {'parent': p.g, 'child': c.g}
