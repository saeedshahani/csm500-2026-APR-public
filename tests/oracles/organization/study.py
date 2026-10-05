"""Standalone evidence verification and source-bundle reproduction."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile

from .campaign import read, write, heldout
from .observe import audit, inspect, functional

ROOT = Path(__file__).resolve().parents[3]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compare_outputs(reference, run):
    """Require the complete scientific output set, not just generated survivors."""
    manifest = json.loads((reference / 'manifest.json').read_text())
    expected = {name for name in manifest if name == 'analysis.json'
                or Path(name).parts[0] in ('campaign', 'native', 'flow')}
    actual = {str(path.relative_to(run)) for path in run.rglob('*') if path.is_file()}
    if not expected or actual != expected:
        raise ValueError(f'reproduction artifact set mismatch: missing={sorted(expected - actual)}, '
                         f'unexpected={sorted(actual - expected)}')
    for name in sorted(expected):
        if (run / name).read_bytes() != (reference / name).read_bytes():
            raise ValueError(f'reproduction mismatch: {name}')
    return len(expected)


def verify(collection):
    root = Path(collection)
    manifest = json.loads((root / 'manifest.json').read_text())
    for name, expected in manifest.items():
        if digest(root / name) != expected:
            raise ValueError(f'evidence digest mismatch: {name}')
    rows = json.loads((root / 'campaign/summary.json').read_text())
    for row in rows:
        case = read(root / 'campaign' / (row['name'] + '.json.gz'))
        audit(case['initial']); audit(case['final_graph'])
        observed = inspect(case['final_graph'])
        if json.loads(json.dumps(observed)) != case['final'] or functional(observed) != case['functional']:
            raise ValueError(f'measurement mismatch: {row["name"]}')
        if heldout(case['final_graph']) != case['heldout']:
            raise ValueError(f'held-out measurement mismatch: {row["name"]}')
        if {k: v for k, v in case.items() if k not in ('initial', 'final_graph')} != row:
            raise ValueError(f'summary attribution mismatch: {row["name"]}')
    print(json.dumps({'verified_artifacts': len(manifest), 'recounted_conditions': len(rows)}))


def freeze(collection):
    root = Path(collection)
    bundle = root / 'source.tar.gz'
    if bundle.exists():
        raise ValueError('source bundle already exists; use a new evidence collection')
    files = []
    for directory in ('crates', 'examples', 'tests/oracles', 'tests/organization', 'proofs/organization'):
        files.extend(p for p in (ROOT / directory).rglob('*') if p.is_file()
                     and '__pycache__' not in p.parts and p.suffix != '.pyc'
                     and 'target' not in p.parts)
    files.extend(ROOT / name for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'tests/__init__.py',
                 'scripts/organization-study.py', 'benchmarks/protocols/organization-v2.json'))
    files = sorted(set(files))
    manifest = {str(p.relative_to(ROOT)): digest(p) for p in files}
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode='w') as archive:
        for path in files:
            info = tarfile.TarInfo(str(path.relative_to(ROOT)))
            data = path.read_bytes(); info.size = len(data); info.mode = 0o644
            archive.addfile(info, io.BytesIO(data))
    bundle.write_bytes(gzip.compress(buffer.getvalue(), mtime=0))
    (root / 'source-manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    # Covers scientific outputs and the exact runnable source; never itself.
    artifacts = {str(p.relative_to(root)): digest(p) for p in sorted(root.rglob('*')) if p.is_file()
                 and p != root / 'manifest.json'}
    (root / 'manifest.json').write_text(json.dumps(artifacts, indent=2, sort_keys=True) + '\n')
    print(json.dumps({'source_files': len(files), 'artifacts': len(artifacts)}))


def reproduce(collection, destination, lean=None):
    reference = Path(collection).resolve(); dest = Path(destination).resolve()
    verify(reference)
    dest.mkdir(parents=True, exist_ok=False)
    source = dest / 'source'; source.mkdir()
    with tarfile.open(reference / 'source.tar.gz', 'r:gz') as archive:
        for entry in archive.getmembers():
            name = Path(entry.name)
            if name.is_absolute() or '..' in name.parts or not entry.isfile():
                raise ValueError('unsafe source archive entry')
            output = source / name; output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(archive.extractfile(entry).read())
    for name, expected in json.loads((reference / 'source-manifest.json').read_text()).items():
        if digest(source / name) != expected:
            raise ValueError(f'source digest mismatch: {name}')
    if (source / 'docs/research/stretch').exists():
        raise ValueError('source export unexpectedly depends on exploratory work')
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1', CARGO_TARGET_DIR=str(dest / 'target'))
    env.pop('PYTHONPATH', None)
    binary = str(dest / 'target/debug/cli')
    oracle = 'models.organization' if (source / 'models/organization').exists() else 'tests.oracles.organization'
    # The flow test must exercise this reconstruction, never skip or use an
    # inherited binary from the caller's workspace.
    env['ORGANIZATION_NATIVE'] = binary
    commands = [['cargo', 'build', '--locked', '--offline', '-p', 'cli'],
                [sys.executable, '-m', 'unittest', 'discover', '-s', 'tests/organization', '-v']]
    run = dest / 'run'
    commands += [[sys.executable, '-m', oracle + '.campaign', str(run / 'campaign')],
                 [sys.executable, '-m', oracle + '.analyse', str(run / 'campaign'), str(run / 'analysis.json')],
                 [sys.executable, '-m', oracle + '.native_check', binary, str(run / 'campaign'), str(run / 'native')],
                 [sys.executable, '-m', oracle, 'demo', '--native', binary, '--output', str(run / 'flow')]]
    if lean:
        commands += [[str(Path(lean).resolve()), 'proofs/organization/Composition.lean']]
    with (dest / 'commands.log').open('w') as log:
        for command in commands:
            print('running', command[:5], flush=True)
            log.write(json.dumps(command) + '\n'); log.flush()
            subprocess.run(command, cwd=source, env=env, stdout=log, stderr=log, check=True)
    compared = compare_outputs(reference, run)
    report = {'schema': 'organization-reproduction-v2', 'source_export_excludes_exploratory_work': True,
              'compared_artifacts': compared, 'byte_identical': True,
              'lean_checked': bool(lean), 'source_files': len(json.loads((reference / 'source-manifest.json').read_text())),
              'scope': 'Fresh source reconstruction on this host, not independent cross-platform replication.'}
    (dest / 'reproduction.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['verify', 'freeze', 'reproduce'])
    parser.add_argument('collection')
    parser.add_argument('--destination')
    parser.add_argument('--lean')
    args = parser.parse_args()
    if args.command == 'verify': verify(args.collection)
    elif args.command == 'freeze': freeze(args.collection)
    elif args.destination: reproduce(args.collection, args.destination, args.lean)
    else: parser.error('reproduce requires --destination pointing to a new directory')


if __name__ == '__main__': main()
