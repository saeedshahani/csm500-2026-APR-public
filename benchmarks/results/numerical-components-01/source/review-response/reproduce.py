"""Freeze the hybrid adapter and dependencies, test, prove, execute and audit."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

FILES = ['experiment.py', 'reproduce.py', 'LocalCycles.lean', 'protocol.md']
COMPONENT_FILES = ['adapter.py', 'test_adapter.py', 'domain.py', 'graph.py', 'oracle.py', 'workflow-request.json', 'README.md']


def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def verify(out):
    out=Path(out)
    manifest=json.loads((out/'manifest.json').read_text())
    for name, expected in manifest['artifacts'].items():
        path=out/name
        if not path.is_file() or sha(path)!=expected: raise ValueError('artifact mismatch: '+name)
    actual={str(p.relative_to(out)) for p in out.rglob('*') if p.is_file() and p.name!='manifest.json'}
    if actual!=set(manifest['artifacts']): raise ValueError('unexpected artifact set')
    return {'artifacts_verified':len(actual)}


def compare(left,right):
    verify(left);verify(right)
    def semantic(value):
        if isinstance(value,dict):return {k:semantic(v) for k,v in value.items() if k!='seconds'}
        if isinstance(value,list):return list(map(semantic,value))
        return value
    a=json.loads((Path(left)/'campaign/results.json').read_text())
    b=json.loads((Path(right)/'campaign/results.json').read_text())
    if semantic(a)!=semantic(b):raise ValueError('semantic results differ')
    traces=list((Path(left)/'campaign').glob('*/trace.json'))+[Path(left)/'cli-example/trace.json']
    for source in traces:
        target=Path(right)/source.relative_to(left)
        if source.read_bytes()!=target.read_bytes():raise ValueError('trace differs')
    return {'semantic_results_equal':True,'native_traces_byte_identical':len(traces)}


def run(args):
    base=Path(__file__).resolve().parent
    out=Path(args.out).resolve();out.mkdir(exist_ok=False)
    frozen=out/'source/review-response';frozen.mkdir(parents=True)
    component=base.parent/'numerical_components'
    if not component.is_dir():component=base.parents[4]/'scripts/numerical_components'
    deps=out/'source/numerical_components';deps.mkdir()
    origins={base/name:frozen/name for name in FILES}
    origins.update({component/name:deps/name for name in COMPONENT_FILES})
    before={str(p):sha(p) for p in origins}
    for source,dest in origins.items():shutil.copy2(source,dest)
    env=dict(os.environ, PYTHONDONTWRITEBYTECODE='1')
    env['PATH']=str(Path(args.lean_bin).resolve())+os.pathsep+env['PATH']
    # Preserve native portable defaults; explicitly disable any inherited larger envelope.
    env.pop('GRAPH_EXECUTION_ENVELOPE',None)
    commands=[
        [sys.executable,'-B','-m','unittest','discover','-s',str(deps),'-p','test_*.py','-v'],
        [str(Path(args.lean_bin)/'lake'),'--dir',str(Path(args.mathlib).resolve()),'env','lean',str(frozen/'LocalCycles.lean')],
        [sys.executable,'-B',str(frozen/'experiment.py'),'--out',str(out/'campaign'),'--cli',str(Path(args.cli).resolve())],
        [sys.executable,'-B',str(deps/'adapter.py'),str(deps/'workflow-request.json'),
         '--cli',str(Path(args.cli).resolve()),'--out',str(out/'cli-example')]]
    records=[]
    for i,command in enumerate(commands):
        process=subprocess.run(command,env=env,text=True,capture_output=True,timeout=180)
        (out/f'check-{i}.stdout').write_text(process.stdout)
        (out/f'check-{i}.stderr').write_text(process.stderr)
        records.append({'argv':command,'exit':process.returncode})
        (out/'commands.json').write_text(json.dumps(records,indent=2)+'\n')
        if process.returncode:raise RuntimeError(f'check {i} failed: {process.stderr}\n{process.stdout}')
    assert before=={str(p):sha(p) for p in origins}
    metadata={'python':sys.version,'executable':sys.executable,'platform':sys.platform,
              'cli_sha256':sha(args.cli),'lean_sha256':sha(Path(args.lean_bin)/'lean'),
              'mathlib_manifest_sha256':sha(Path(args.mathlib)/'lake-manifest.json'),
              'source_unchanged_during_run':True,
              'sources':{str(dest.relative_to(out)):sha(dest) for dest in origins.values()}}
    (out/'environment.json').write_text(json.dumps(metadata,indent=2)+'\n')
    manifest={'artifacts':{str(p.relative_to(out)):sha(p) for p in sorted(out.rglob('*')) if p.is_file()}}
    (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(verify(out)))


if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--out');p.add_argument('--cli');p.add_argument('--mathlib');p.add_argument('--lean-bin')
    p.add_argument('--verify');p.add_argument('--compare',nargs=2);a=p.parse_args()
    if a.verify:print(json.dumps(verify(a.verify)))
    elif a.compare:print(json.dumps(compare(*a.compare)))
    else:
        if not all((a.out,a.cli,a.mathlib,a.lean_bin)):p.error('run requires out, cli, mathlib and lean-bin')
        run(a)
