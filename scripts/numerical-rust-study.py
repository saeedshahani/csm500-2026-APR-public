"""Differential tests only: canonical Rust versus frozen reference and independent intervals.

The application never imports this driver or its Python references. No archived
source or historical measurement is modified. Report identity covers current Rust.
"""
import argparse
import hashlib
import json
import random
import subprocess
import sys
from fractions import Fraction as Q
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT / 'benchmarks/results/numerical-components-01/source/numerical_components'
sys.path.insert(0, str(REFERENCE))
from adapter import checked_task, solve, residual_radius


def enclosure(task, p):
    scale = 1 << 70
    lower, upper = [0]*len(task), [scale]*len(task)
    for _ in range(600):
        lows, highs = [], []
        for row in task:
            lo = max(Q(a['reward'])*scale + Q(a['boundary'])*p*scale
                     + sum(Q(w)*v for w, v in zip(a['weights'], lower)) for a in row)
            hi = max(Q(a['reward'])*scale + Q(a['boundary'])*p*scale
                     + sum(Q(w)*v for w, v in zip(a['weights'], upper)) for a in row)
            lows.append(lo.numerator // lo.denominator)
            highs.append(-(-hi.numerator // hi.denominator))
        if (lower, upper) == (lows, highs):
            break
        lower, upper = lows, highs
    return [(Q(lo, scale), Q(hi, scale)) for lo, hi in zip(lower, upper)]


def generated(rng, n):
    rows = []
    for _ in range(n):
        actions = []
        for _ in range(rng.randint(1, 2)):
            weights = [rng.randrange(8) for _ in range(n)]
            if not sum(weights): weights[0] = 1
            mass = Q(rng.randrange(1, 7), 8)
            reward = Q(rng.randrange(17), 16)
            boundary = Q(rng.randrange(17-int(reward*16)), 16)
            actions.append({'weights': [str(mass*w/sum(weights)) for w in weights],
                            'reward': str((1-mass)*reward), 'boundary': str((1-mass)*boundary)})
        rows.append(actions)
    return rows


def require(ok, message):
    if not ok: raise RuntimeError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', required=True)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    rng = random.Random(1005202602)
    runs, bounds, publications = [], 0, 0
    for index in range(80):
        task = generated(rng, 1+index % 4)
        p = Q(rng.randrange(33), 32)
        changes = [Q(rng.randrange(33), 32) for _ in range(4)]
        intervals = [enclosure(task, parent) for parent in [p]+changes]
        for mode in ('raw', 'compiled', 'direct'):
            steps, bits = index % 65, 8+index % 17
            operations = []
            for tick, boundary in enumerate(changes, 1):
                operations.extend([
                    {'operation':'observe','boundary':str(boundary),'sequence':tick,'tick':tick},
                    {'operation':'assess','receipt':0,'tolerance':'1/1000','now':tick,'max_age':32}])
            request = {'identity':f'case-{index}-{mode}','task':task,'boundary':str(p),
                       'mode':mode,'steps':steps,'bits':bits,'tolerance':'1/1000','operations':operations}
            run = subprocess.run([args.cli,'numerical-run','-'],input=json.dumps(request),
                                 text=True,capture_output=True,timeout=60,check=True)
            report = json.loads(run.stdout)
            expected, _ = solve(checked_task(task), p, mode, steps, bits)
            radius, residual, q = residual_radius(checked_task(task),p,expected)
            data = report['receipts'][0]
            require(tuple(map(Q,data['values'])) == expected, 'value disagreement')
            require(tuple(map(Q,[data['radius'],data['residual'],data['q']])) == (radius,residual,q), 'bound disagreement')
            for publication, interval in zip(report['publications'],intervals,strict=True):
                packet = publication['packet']
                require(publication['trace_checked'] and publication['replayed'], 'unverified publication')
                check = subprocess.run([args.cli,'numerical-check','-'], input=json.dumps(packet),
                                       text=True,capture_output=True,timeout=30,check=True)
                require(json.loads(check.stdout)['consistent'], 'packet rejected')
                radius_now = Q(packet['radius_now'])
                for value, (lo,hi) in zip(expected,interval,strict=True):
                    require(lo <= hi and hi-lo < Q(1,10**16), 'interval too wide')
                    require(max(abs(value-lo),abs(value-hi)) <= radius_now+(hi-lo), 'error bound failed')
                    bounds += 1
                publications += 1
            runs.append({'case':index,'mode':mode,'values':data['values'],'radius':data['radius'],
                         'statuses':[x['packet']['status'] for x in report['publications']]})
    sources = list((ROOT/'crates/engine/src/numerical').glob('*.rs')) + [ROOT/p for p in [
        'Cargo.lock','crates/engine/Cargo.toml','crates/cli/src/commands.rs','scripts/numerical-rust-study.py',
        'crates/engine/tests/numerical.rs','crates/cli/tests/numerical.rs','proofs/numerical/LocalCycles.lean']]
    report = {'schema':'numerical-rust-correspondence-v1','tasks':80,'solver_runs':len(runs),
              'verified_publications':publications,'coordinate_bound_checks':bounds,'seed':1005202602,
              'cli_sha256':hashlib.sha256(Path(args.cli).read_bytes()).hexdigest(),
              'sources':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(sources)},
              'runs':runs,'scope':'Same-host finite correspondence and interval checks; not performance or whole-program proof.'}
    with Path(args.output).open('x') as stream: json.dump(report,stream,indent=2); stream.write('\n')
    print(json.dumps({key:report[key] for key in ['tasks','solver_runs','verified_publications','coordinate_bound_checks']}))


if __name__ == '__main__': main()
