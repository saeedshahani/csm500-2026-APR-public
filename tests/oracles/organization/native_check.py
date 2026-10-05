"""Test oracle against admitted Rust table executions and imported traces."""
import copy,gzip,hashlib,itertools,json,subprocess,sys
from pathlib import Path
from .campaign import read,write,SEEDS

def main(binary,evidence,dest):
    dest=Path(dest);dest.mkdir(parents=True,exist_ok=False);rows=[]
    def invoke(v,name,valid=True,verify=False):
        inp=dest/(name+'-input.json');inp.write_text(json.dumps(v,sort_keys=True)+'\n')
        result=subprocess.run([binary,'trace-check' if verify else 'model-run',str(inp)],text=True,capture_output=True)
        if valid:
            assert result.returncode==0,(name,result.stderr)
            out=json.loads(result.stdout)
            # Separate implementation of sequential transition evaluation.
            table={(x['from'],x['action']):x['target'] for x in v['transitions']}
            current=v['start'];expected=[current]
            for a in v['word']:current=table[current,a];expected.append(current)
            assert out['projected']==expected and out['portable_trace_verified']
            assert out['admitted_steps']==len(v['word'])
            write(dest/(name+'-output.json.gz'),out)
            rows.append({'name':name,'steps':len(v['word']),'trace_verified':True})
            return out
        assert result.returncode!=0,(name,'invalid input accepted')
        rows.append({'name':name,'rejected':True,'diagnostic':result.stderr.strip()})
    # Exhaustive two-state binary deterministic tables, both start states,
    # all length-two words. This checks all 16*2*4 = 128 configurations.
    last=None
    for table in range(16):
        transitions=[{'from':str(s),'action':a,'target':str((table>>(2*s+a))&1)} for s in range(2) for a in range(2)]
        for start in range(2):
            for word in itertools.product(range(2),repeat=2):
                v={'states':['0','1'],'start':str(start),'transitions':transitions,'word':list(word)}
                last=invoke(v,f'exhaustive-{table}-{start}-{word[0]}{word[1]}')
    exemplar=v
    for seed in SEEDS:
        case=read(Path(evidence)/f'acquisition-learned-{seed}.json.gz')
        if not case['functional']:continue
        g=case['final_graph'];v={'states':g['environment']['states'],'start':g['environment']['at'],
            'transitions':list(g['root']['model'].values()),'word':[0,1,1,0,0,1,0,1],
            'checkpoint_sha256':hashlib.sha256((Path(evidence)/f'acquisition-learned-{seed}.json.gz').read_bytes()).hexdigest()}
        invoke(v,f'acquired-{seed}')
    for name,edit in [('duplicate',lambda v:v['transitions'].append(v['transitions'][0])),
                      ('unknown_target',lambda v:v['transitions'][0].update(target='missing')),
                      ('unknown_start',lambda v:v.update(start='missing')),
                      ('invalid_action',lambda v:v.update(word=[2])),
                      ('word_bound',lambda v:v.update(word=[0]*17)),
                      ('unknown_transition',lambda v:v.update(transitions=[])),
                      ('zero_work_budget',lambda v:v.update(native_max_work=0))]:
        bad=copy.deepcopy(exemplar);edit(bad);invoke(bad,name,valid=False)
    tampered=copy.deepcopy(last['trace']);tampered['initial_sha256']='0'*64
    invoke(tampered,'tampered_trace',valid=False,verify=True)
    (dest/'summary.json').write_text(json.dumps(rows,indent=2,sort_keys=True)+'\n')
    (dest/'manifest.json').write_text(json.dumps({p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(dest.iterdir()) if p.is_file()},indent=2,sort_keys=True)+'\n')
    print(json.dumps({'native_cases':sum('steps' in r for r in rows),'native_steps':sum(r.get('steps',0) for r in rows),'rejections':sum(r.get('rejected',False) for r in rows)}))
if __name__=='__main__':main(*sys.argv[1:])
