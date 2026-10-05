"""Differential acceptance of canonical Rust against frozen independent Python evidence.

This driver does not implement the dynamics or run the historical simulator.
It reconstructs declared inputs, then compares complete states, metrics,
allocations and held-out measurements, not only success counts.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess


EVENTS = [{'at': 899, 'operation': 'checker_fault'},
          {'at': 900, 'operation': 'environment_swap'},
          {'at': 1100, 'operation': 'recognition_scramble'}]


def request_for(name):
    group, seed = name.rsplit('-', 1)
    config = {'seed': int(seed), 'n': 128, 'energy': 36000, 'source': 400000,
              'packet': 512, 'background_until': 800}
    request = {'config': config, 'events': [dict(e) for e in EVENTS], 'stages': [{'epochs': 1600}]}
    if group.startswith('acquisition-'):
        mode = group.removeprefix('acquisition-')
        config['recognition'] = 'fixed' if mode == 'fixed_unscrambled' else mode
        if mode == 'fixed_unscrambled': request['events'] = request['events'][:2]
    elif group.startswith('ablation-'):
        condition = group.removeprefix('ablation-')
        stage = {'epochs': 750}
        controls = {'no_learning': 'learn', 'no_planning': 'use_model',
                    'no_checker_renewal': 'renew_checkers', 'single_checker': 'independent_check',
                    'no_recognition_revision': 'revise_recognition', 'no_feedback': 'feedback',
                    'no_copying': 'copying'}
        if condition in controls: stage['controls'] = {controls[condition]: False}
        if condition == 'common_fault': request['events'][0]['common_mode'] = True
        if condition == 'no_resources': stage['drain_resources'] = True
        request['stages'] = [{'epochs': 850}, stage]
    elif group.startswith('inheritance-'):
        condition = group.removeprefix('inheritance-')
        mode = condition if condition in ('template', 'sequence') else 'none' if condition == 'model_only' else 'both'
        request['inheritance'] = {'seed': int(seed)+100000, 'bottleneck': 32 if condition == 'both32' else 64,
                                 'energy': 12000, 'mode': mode, 'transfer_model': condition != 'no_model',
                                 'stages': [{'epochs': 600, 'complement_operators': condition == 'altered_code'}]}
    elif group.startswith('length-'): config['length'] = int(group.removeprefix('length-'))
    elif group.startswith('scarcity-'): config['symbol_factor'] = float(group.removeprefix('scarcity-'))
    else: raise ValueError(f'unknown condition: {name}')
    return request


def difference(a, b, path=''):
    if type(a) is dict and type(b) is dict:
        if a.keys() != b.keys(): return path + ': keys ' + str(a.keys() ^ b.keys())
        for key in a:
            result = difference(a[key], b[key], path+'/'+key)
            if result: return result
    elif type(a) is list and type(b) is list:
        if len(a) != len(b): return path + ': length'
        for index, (x,y) in enumerate(zip(a,b)):
            result = difference(x,y,path+'/'+str(index))
            if result: return result
    elif a != b: return f'{path}: {a!r} != {b!r}'
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', required=True)
    parser.add_argument('--evidence', default='benchmarks/results/organization-02')
    parser.add_argument('--output', required=True)
    parser.add_argument('--limit', type=int)
    args = parser.parse_args()
    root = Path(args.evidence)
    repository = Path(__file__).resolve().parents[1]
    sources = [p for p in (repository/'crates').rglob('*') if p.is_file()
               and p.suffix in ('.rs', '.toml')]
    sources += [repository/name for name in ('Cargo.toml','Cargo.lock','rust-toolchain.toml',
                'scripts/organization-rust-study.py','examples/organization/renewal.json')]
    source_hashes = {str(p.relative_to(repository)): hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in sorted(sources)}
    binary_hash = hashlib.sha256(Path(args.cli).read_bytes()).hexdigest()
    manifest = json.loads((root/'manifest.json').read_text())
    for name, expected in manifest.items():
        assert hashlib.sha256((root/name).read_bytes()).hexdigest() == expected, name
    rows = json.loads((root/'campaign/summary.json').read_text())
    if args.limit is not None: rows = rows[:args.limit]
    outcomes = []
    for row in rows:
        name = row['name']
        case = json.loads(gzip.decompress((root/'campaign'/f'{name}.json.gz').read_bytes()))
        request = request_for(name)
        result = subprocess.run([args.cli, 'organization-run', '-'], input=json.dumps(request),
                                text=True, capture_output=True, timeout=60, check=True)
        actual = json.loads(result.stdout)
        is_child = 'inheritance' in request
        endpoint = actual['child' if is_child else 'parent']
        for label, expected, value in [
            ('graph',case['final_graph'],endpoint['graph']),
            ('measurement',case['final'],endpoint['summary']['measurement']),
            ('heldout',case['heldout'],endpoint['summary']['heldout']),
            ('functional',case['functional'],endpoint['summary']['functional']),
            ('timeline',case['timeline'],endpoint['timeline'][-len(case['timeline']):])]:
            error = difference(expected,value)
            if error: raise AssertionError(f'{name}/{label}: {error}')
        if is_child:
            world = json.loads(gzip.decompress((root/'campaign'/f'world-{name.removeprefix("inheritance-")}.json.gz').read_bytes()))
            error = difference(world,actual['allocation'])
            if error: raise AssertionError(f'{name}/allocation: {error}')
        outcomes.append({'name':name,'complete_state_equal':True,'functional':case['functional']})
        print(f'{len(outcomes)}/{len(rows)} {name}', flush=True)
    path = Path(args.output)
    assert source_hashes == {name:hashlib.sha256((repository/name).read_bytes()).hexdigest()
                             for name in source_hashes}, 'source changed during campaign'
    assert binary_hash == hashlib.sha256(Path(args.cli).read_bytes()).hexdigest(), 'binary changed during campaign'
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('x') as stream:
        json.dump({'schema':'organization-rust-correspondence-v1','cases':len(outcomes),
                   'rust_sources':source_hashes, 'cli_sha256':binary_hash,
                   'source_manifest_sha256':hashlib.sha256((root/'source-manifest.json').read_bytes()).hexdigest(),
                   'outcomes':outcomes,'scope':'Exact finite differential evidence, not a verified implementation refinement.'},stream,indent=2)
        stream.write('\n')


if __name__ == '__main__': main()
