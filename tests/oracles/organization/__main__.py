"""Historical Python flow oracle. Product use is cli organization-run."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from . import System, audit, inherit, inspect, functional
from .campaign import initial, evaluate, heldout, write


def demo(destination, native, seed=920003):
    """One continuous acquisition → renewal → inheritance → native history flow."""
    dest = Path(destination)
    dest.mkdir(parents=True, exist_ok=False)
    parent = System(initial(seed))
    parent_result = evaluate(parent, 1600)
    world = inherit(parent.g, seed + 100000, bottleneck=64, energy=12000)
    for graph in world.values():
        audit(graph)
    child = System(world['child'])
    child_result = evaluate(child, 600)
    checkpoint = child.g
    request = {'states': checkpoint['environment']['states'],
               'start': checkpoint['environment']['at'],
               'transitions': list(checkpoint['root']['model'].values()),
               'word': [0, 1, 1, 0, 0, 1, 0, 1]}
    (dest / 'model.json').write_text(json.dumps(request, indent=2) + '\n')
    result = subprocess.run([str(Path(native).resolve()), 'model-run', str(dest / 'model.json')],
                            capture_output=True, text=True, check=True)
    native_result = json.loads(result.stdout)
    (dest / 'native.json').write_text(json.dumps(native_result, indent=2) + '\n')
    trace = dest / 'trace.json'
    trace.write_text(json.dumps(native_result['trace']) + '\n')
    # Re-import in a separate native process, with normal CLI admission checks.
    subprocess.run([str(Path(native).resolve()), 'trace-check', str(trace)],
                   capture_output=True, text=True, check=True)
    measurement = inspect(checkpoint)
    environmental_agreement = None
    if measurement['decoder_deterministic']:
        current = request['start']; actual = [current]
        for action in request['word']:
            current = checkpoint[current]['next'][str(measurement['code'][action])]
            actual.append(current)
        environmental_agreement = actual == native_result['projected']
    write(dest / 'lineage.json.gz', {'parent': parent.g, 'allocation': world, 'child': child.g})
    report = {'schema': 'organization-flow-v2', 'seed': seed,
              'parent': parent_result, 'child': child_result,
              'child_heldout': heldout(child.g),
              'native_steps': native_result['admitted_steps'],
              'native_trace_verified': native_result['portable_trace_verified'],
              'separate_process_trace_verified': True,
              'environmental_agreement': environmental_agreement,
              'same_code': inspect(parent.g)['code'] == measurement['code'],
              'scope': 'Host acquisition and inheritance; admitted finite-word native execution.'}
    (dest / 'report.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    (dest / 'manifest.json').write_text(json.dumps({p.name: hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(dest.iterdir()) if p.is_file()}, indent=2, sort_keys=True) + '\n')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    run = sub.add_parser('demo', help='acquire, renew, inherit and execute a learned model')
    run.add_argument('--output', required=True, help='new directory; existing outputs are never overwritten')
    run.add_argument('--native', default='target/debug/cli', help='built project CLI')
    run.add_argument('--seed', type=int, default=920003)
    args = parser.parse_args()
    try:
        report = demo(args.output, args.native, args.seed)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(2, f'organization: {error}\n')
    print(json.dumps({'parent_functional': report['parent']['functional'],
                      'child_functional': report['child']['functional'],
                      'environmental_agreement': report['environmental_agreement'],
                      'native_steps': report['native_steps'], 'output': args.output}))


if __name__ == '__main__':
    main()
