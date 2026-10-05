"""Historical bounded Python campaign for differential testing only."""
import copy,gzip,hashlib,json,sys
from pathlib import Path
from .system import System,fresh,inherit
from .observe import inspect,audit,functional

SEEDS=[920003,920009,920021,920033,920041,920057,920063,920081]
EVENTS=[{'at':899,'operation':'checker_fault'}, {'at':900,'operation':'environment_swap'},
        {'at':1100,'operation':'recognition_scramble'}]

def write(path,obj):
    data=(json.dumps(obj,sort_keys=True,separators=(',',':'))+'\n').encode()
    path.write_bytes(gzip.compress(data,mtime=0))

def read(path):return json.loads(gzip.decompress(Path(path).read_bytes()))

def initial(seed,mode='learned',**kw):
    g=fresh(seed=seed,recognition=mode,n=128,energy=36000,source=400000,packet=512,**kw)
    g['root']['background_until']=800;g['root']['events']=copy.deepcopy(EVENTS)
    return g

def evaluate(vm,epochs):
    founder_programs=set(x for x in vm.r['programs'] if x)
    old_updates=vm.r['recognition_updates'];old_repairs=vm.r['checker_repairs']
    timeline=[]
    for i in range(epochs):
        vm.tick()
        if (i+1)%50==0:
            audit(vm.g);timeline.append(inspect(vm.g))
    audit(vm.g);last=inspect(vm.g)
    return {'final':last,'functional':functional(last),'timeline':timeline,
            'founder_programs':len(founder_programs),
            'remaining_founder_programs':len(founder_programs.intersection(vm.g)),
            'new_recognition_updates':vm.r['recognition_updates']-old_updates,
            'new_checker_repairs':vm.r['checker_repairs']-old_repairs}

def heldout(g):
    # Exact probability of every prefix agreeing, with a frozen decoder.
    # Conditioning on agreement leaves one state, so multiplication suffices.
    r=g['root'];measurement=inspect(g);distributions=measurement['action_distributions']
    expected_passed=total=0
    for start in g['environment']['states']:
        for word in range(64):
            predicted=start;probability=1.0
            for shift in range(5,-1,-1):
                action=(word>>shift)&1
                record=r['model'].get(predicted+':'+str(action))
                if record is None:probability=0.0;break
                probability *= sum(p for physical,p in enumerate(distributions[action])
                                   if g[predicted]['next'][str(physical)]==record['target'])
                predicted=record['target']
            expected_passed+=probability;total+=1
    return {'passed':int(expected_passed) if measurement['decoder_deterministic'] else None,
            'expected_passed':expected_passed,'total':total,
            'deterministic':measurement['decoder_deterministic'],
            'scope':'exact prefix-agreement probability at frozen checkpoint; not continued noisy dynamics'}

def save(dest,name,initial_graph,vm,result):
    result={'name':name,**result,'heldout':heldout(vm.g)}
    write(dest/(name+'.json.gz'),{'initial':initial_graph,'final_graph':vm.g,**result})
    return result

def main(dest):
    dest=Path(dest);dest.mkdir(parents=True,exist_ok=False);rows=[]
    for seed in SEEDS:
        for mode in ('learned','fixed','random','generic','fixed_unscrambled'):
            g=initial(seed,'fixed' if mode=='fixed_unscrambled' else mode)
            if mode=='fixed_unscrambled':g['root']['events']=copy.deepcopy(EVENTS[:2])
            vm=System(g);result=evaluate(vm,1600)
            rows.append(save(dest,f'acquisition-{mode}-{seed}',g,vm,result))
        base=System(initial(seed));base.run(850);audit(base.g)
        for condition in ('complete','no_learning','no_planning','no_checker_renewal','single_checker',
                          'common_fault','no_recognition_revision','no_feedback','no_copying','no_resources'):
            g=copy.deepcopy(base.g);r=g['root']
            settings={'no_learning':('learn',False),'no_planning':('use_model',False),
                      'no_checker_renewal':('renew_checkers',False),'single_checker':('independent_check',False),
                      'no_recognition_revision':('revise_recognition',False),'no_feedback':('feedback',False)}
            if condition in settings:r[settings[condition][0]]=settings[condition][1]
            if condition=='common_fault':r['events'][0]['common_mode']=True
            if condition=='no_copying':r['rates'][1]=0
            if condition=='no_resources':
                r['spent_energy']+=r['energy']+r['source'];r['energy']=r['source']=0
            vm=System(g);result=evaluate(vm,750)
            rows.append(save(dest,f'ablation-{condition}-{seed}',g,vm,result))
        parent=read(dest/f'acquisition-learned-{seed}.json.gz')['final_graph']
        for condition in ('both64','both32','no_model','template','sequence','model_only','altered_code'):
            b=32 if condition=='both32' else 64
            mode=condition if condition in ('template','sequence') else 'none' if condition=='model_only' else 'both'
            world=inherit(parent,seed+100000,bottleneck=b,transfer_model=condition!='no_model',mode=mode,energy=12000)
            before=System(parent).inventory()
            for pool in before:
                assert before[pool]==[a+b for a,b in zip(System(world['parent']).inventory()[pool],System(world['child']).inventory()[pool])]
            assert parent['root']['initial_energy']==sum(world[k]['root']['initial_energy'] for k in world)
            for state in world.values():audit(state)
            vm=System(world['child'])
            if condition=='altered_code':
                for i,name in enumerate(vm.r['pools']['sequence'][:]):
                    if name is not None:assert vm.replace('sequence',i,'p'+str(int(name[1:])^((1<<vm.r['length'])-1)),initialization=True)
            g=copy.deepcopy(vm.g);birth=inspect(g);birth_words=heldout(g)
            result=evaluate(vm,600)
            result.update({'parent_functional':functional(inspect(parent)),'birth':birth,'birth_heldout':birth_words,
                           'same_code_as_parent':inspect(vm.g)['code']==inspect(parent)['code']})
            rows.append(save(dest,f'inheritance-{condition}-{seed}',g,vm,result))
            write(dest/f'world-{condition}-{seed}.json.gz',world)
        print('finished seed',seed,flush=True)
    # Deliberately scarce symbol stock and longer pre-enumerated sequence banks.
    for length in (4,6,8):
        for seed in SEEDS[:3]:
            g=initial(seed,length=length);vm=System(g);result=evaluate(vm,1600)
            rows.append(save(dest,f'length-{length}-{seed}',g,vm,result))
    for factor in (.05,.15):
        for seed in SEEDS[:3]:
            g=initial(seed,symbol_factor=factor);vm=System(g);result=evaluate(vm,1600)
            rows.append(save(dest,f'scarcity-{factor}-{seed}',g,vm,result))
    (dest/'summary.json').write_text(json.dumps(rows,indent=2,sort_keys=True)+'\n')
    manifest={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(dest.iterdir()) if p.is_file()}
    (dest/'manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main(sys.argv[1])
