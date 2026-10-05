import copy,json,math,unittest
from tests.oracles.organization.system import System,fresh,inherit,draw
from tests.oracles.organization.observe import audit,inspect
from tests.oracles.organization.theory import positive_equilibria,blocks


def stable():
    vm=System(fresh(seed=12345,recognition='fixed',initial='core'))
    for i in range(vm.r['n']):vm.replace('sequence',i,'p'+str(vm.r['core'][i%2]))
    vm.r['background_until']=0
    return vm

class TheoryTests(unittest.TestCase):
    def test_review_counterexample_and_all_zero_rate_boundaries(self):
        t,r,dg,dp=100,400,0,5
        delta=(r-2*dg)**2-8*r*dg*dp/t
        self.assertGreater(delta,0);self.assertEqual((r-2*dg-math.sqrt(delta))/(2*r),0)
        for rates in ((t,r,dg,dp),(0,400,5,5),(100,0,5,5),(100,400,5,0),(-1,400,5,5),(math.inf,400,5,5),(math.nan,400,5,5)):
            with self.assertRaises(ValueError):positive_equilibria(*rates)
    def test_repeated_root_and_no_positive_root_excluded(self):
        # r=8,dg=1,t=16,dp=9 gives discriminant zero exactly.
        for rates in ((16,8,1,9),(1,1,1,1),(1,10,1,100)):
            with self.assertRaises(ValueError):positive_equilibria(*rates)
    def test_equilibria_and_full_mode_stability(self):
        checks=0
        for t in (20,50,100,200):
            for r in (100,200,400,800):
                for dg in (1,3,5):
                    for dp in (1,2,5):
                        try:roots=positive_equilibria(t,r,dg,dp)
                        except ValueError:continue
                        for i,(g,a) in enumerate(roots):
                            self.assertTrue(0<g<1 and 0<a<1)
                            self.assertAlmostEqual(r*a*g*(1-g)/2-dg*g,0,places=9)
                            self.assertAlmostEqual(t*g*(1-a)-dp*a,0,places=9)
                            total,imbalance=blocks(t,r,dg,dp,g,a)
                            det=lambda m:m[0][0]*m[1][1]-m[0][1]*m[1][0]
                            self.assertLess(total[0][0]+total[1][1],0)
                            self.assertTrue(det(total)>0 if i else det(total)<0)
                            self.assertGreater(det(imbalance),0)
                            self.assertLess(imbalance[0][0]+imbalance[1][1],0)
                            imbalance[0][1]*=-1;self.assertLess(det(imbalance),0);checks+=1
        self.assertGreater(checks,200)

class SystemTests(unittest.TestCase):
    def test_determinism(self):
        g=fresh(seed=5678);a=System(g);b=System(g);a.run(80);b.run(80);self.assertEqual(a.g,b.g)
    def test_random_replacements_conserve_and_count(self):
        vm=stable()
        for _ in range(1000):
            pool=('template','sequence')[draw(vm.r,2)];i=draw(vm.r,vm.r['n']);w=draw(vm.r,17)
            vm.replace(pool,i,None if w==16 else ('t' if pool=='template' else 'p')+str(w));audit(vm.g)
    def test_symbol_scarcity_is_binding(self):
        vm=System(fresh(symbol_factor=.02));vm.run(50);audit(vm.g);self.assertGreater(vm.r['blocked_symbols'],0)
    def test_starvation_does_not_freeze_instances(self):
        vm=stable();r=vm.r;r['spent_energy']+=r['energy']+r['source'];r['energy']=r['source']=0
        vm.run(2500);audit(vm.g)
        self.assertTrue(all(x is None for pool in r['pools'].values() for x in pool));self.assertGreater(r['blocked_energy'],0)
    def test_committed_prediction_checked_against_observation(self):
        vm=stable();before=vm.g['environment']['at']
        for a in (0,1):vm.r['model'][before+':'+str(a)]={'from':before,'action':a,'target':'wrong','revision':1}
        vm.act(vm.programs());w=vm.r['history'][-1]
        self.assertEqual(w['predicted'],'wrong');self.assertFalse(w['primary']);self.assertFalse(w['secondary'])
        self.assertEqual(vm.r['model'][before+':'+str(w['action'])]['target'],w['observed'])
    def test_independent_checker_repairs_fault_without_accessing_hidden_table(self):
        vm=stable();env=copy.deepcopy({k:v for k,v in vm.g.items() if v['tag']=='environment_state'})
        vm.intervene({'operation':'checker_fault'});vm.act(vm.programs())
        self.assertEqual(vm.r['checker_repairs'],1)
        self.assertEqual(vm.g['blueprint:0']['instructions'],['equal'])
        self.assertEqual(env,{k:v for k,v in vm.g.items() if v['tag']=='environment_state'})
    def test_common_mode_false_acceptance_remains_undetected(self):
        vm=stable();before=vm.g['environment']['at']
        for a in (0,1):vm.r['model'][before+':'+str(a)]={'from':before,'action':a,'target':'wrong','revision':1}
        vm.intervene({'operation':'checker_fault','common_mode':True});vm.act(vm.programs())
        self.assertEqual(vm.r['checker_repairs'],0);self.assertTrue(vm.r['history'][-1]['primary'])
        self.assertEqual(vm.r['model'][before+':'+str(vm.r['history'][-1]['action'])]['target'],'wrong')
    def test_no_renewal_destroys_executable_instances(self):
        vm=stable();ids=set(x for x in vm.r['programs'] if x);self.assertEqual(len(ids),vm.r['n'])
        vm.r['renew_checkers']=False
        for i,name in enumerate(vm.r['pools']['sequence'][:]):vm.replace('sequence',i,name)
        self.assertFalse(ids.intersection(vm.g));self.assertFalse(any(vm.r['programs']))
    def test_healthy_revalidation_acquires_complete_model(self):
        vm=stable()
        for _ in range(300):vm.act(vm.programs());vm.r['epoch']+=1
        result=inspect(vm.g);self.assertEqual(result['model_coverage'],1);self.assertEqual(result['model_accuracy'],1)
    def test_recognition_uses_observations_not_observer_core_labels(self):
        vm=stable();vm.r['recognition']='learned';vm.r['production']={'t1':[0]*20,'t3':[1]*20};vm.r['production_order']=['t1','t3']
        other=System(vm.g);other.r['core']=[7,11]
        for v in (vm,other):
            for p in v.r['operators']:v.g[p]['recognizes']=['t0']
            v.revise_recognition(v.programs())
        self.assertEqual([vm.g[p]['recognizes'] for p in vm.r['operators']],[other.g[p]['recognizes'] for p in other.r['operators']])
        self.assertEqual(vm.r['recognition_updates'],4)
    def test_inheritance_world_conservation(self):
        vm=stable();before=vm.inventory()
        for mode in ('both','template','sequence','none'):
            for b in (0,1,16,64):
                world=inherit(vm.g,54321,bottleneck=b,mode=mode)
                for g in world.values():audit(g)
                for pool in before:
                    self.assertEqual(before[pool],[a+b for a,b in zip(System(world['parent']).inventory()[pool],System(world['child']).inventory()[pool])])
                self.assertEqual(vm.r['initial_energy'],sum(g['root']['initial_energy'] for g in world.values()))
    def test_changed_inherited_code_breaks_zero_shot_model(self):
        vm=stable()
        for _ in range(300):vm.act(vm.programs());vm.r['epoch']+=1
        child=System(inherit(vm.g,54321,bottleneck=64)['child']);self.assertEqual(inspect(child.g)['model_accuracy'],1)
        for i,name in enumerate(child.r['pools']['sequence'][:]):child.replace('sequence',i,'p'+str(int(name[1:])^15))
        self.assertEqual(inspect(child.g)['model_accuracy'],0)


class RegressionTests(unittest.TestCase):
    def test_inherited_validation_age_and_witness_clock_across_generations(self):
        parent=stable()
        for _ in range(300): parent.act(parent.programs());parent.r['epoch']+=1
        before=copy.deepcopy(parent.g)
        child=System(inherit(parent.g,98765,bottleneck=64)['child'])
        for key,record in child.r['model'].items():
            old=parent.r['model'][key]
            self.assertEqual(-record['last_checked'],parent.r['epoch']-old['last_checked'])
            self.assertEqual(record['witness_clock'],old['witness_clock'])
            self.assertEqual(record['witness_epoch'],old['witness_epoch'])
        # Repeated visits to the same state must service both stale actions.
        state=child.g['environment']['at'];actions=[]
        for epoch in (0,4,8,12):
            child.g['environment']['at']=state;child.r['epoch']=epoch
            child.act(child.programs());actions.append(child.r['history'][-1]['action'])
        self.assertEqual(actions[0],actions[2]);self.assertEqual(actions[1],actions[3])
        self.assertNotEqual(actions[0],actions[1])
        grandchild=inherit(child.g,98766,bottleneck=32)['child']
        for key,record in grandchild['root']['model'].items():
            self.assertEqual(record['last_checked'],child.r['model'][key]['last_checked']-12)
            self.assertEqual(record['witness_clock'],child.r['model'][key]['witness_clock'])
        self.assertEqual(parent.g,before)

    def test_tied_decoder_accuracy_and_exact_word_probability(self):
        from tests.oracles.organization.campaign import heldout
        vm=stable();vm.r['feedback']=False
        for state in vm.g['environment']['states']:
            for action in (0,1):
                vm.r['model'][f'{state}:{action}']={'from':state,'action':action,'target':vm.g[state]['next']['0']}
        result=inspect(vm.g)
        self.assertEqual(result['code'],[None,None]);self.assertEqual(result['model_accuracy'],.5)
        words=heldout(vm.g)
        self.assertIsNone(words['passed']);self.assertEqual(words['expected_passed']/words['total'],1/64)
        # Ties need not imply error when both physical edges share a target.
        for state in vm.g['environment']['states']: vm.g[state]['next']['1']=vm.g[state]['next']['0']
        self.assertEqual(inspect(vm.g)['model_accuracy'],1)
        self.assertEqual(heldout(vm.g)['expected_passed'],512)

    def test_checkpoint_key_order_does_not_change_continuation(self):
        a=System(fresh(seed=5678));a.run(100)
        b=System(json.loads(json.dumps(a.g,sort_keys=True)))
        a.run(150);b.run(150);self.assertEqual(a.g,b.g)

    def test_public_construction_rejects_invalid_resource_and_rng_domains(self):
        for kw in ({'seed':0},{'seed':-1},{'seed':True},{'length':2},{'length':11},
                   {'rooms':2},{'n':0},{'energy':-1},{'symbol_factor':math.inf}):
            with self.assertRaises(ValueError):fresh(**kw)
        for kw in ({'seed':0},{'bottleneck':65},{'bottleneck':-1},{'mode':'unknown'},{'energy':-1}):
            with self.assertRaises(ValueError):inherit(stable().g,**({'seed':1}|kw))

if __name__=='__main__':unittest.main(verbosity=2)
