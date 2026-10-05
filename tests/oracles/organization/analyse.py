"""Historical descriptive analysis; no observations feed the Rust application."""
import collections,json,sys
from pathlib import Path
from .campaign import read,SEEDS

def main(source,dest):
    source=Path(source);rows=json.loads((source/'summary.json').read_text());groups=collections.defaultdict(list)
    for r in rows:groups[r['name'].rsplit('-',1)[0]].append(r)
    report={'experimental_unit':'seed lineage; paired conditions, reused length4 runs, timepoints and children are not independent','groups':{},'complete_lineages':{}}
    for name,items in groups.items():
        cases=[read(source/(i['name']+'.json.gz')) for i in items]
        report['groups'][name]={'met_criteria':sum(i['functional'] for i in items),'cases':len(items),
             'repairs':sum(i['new_checker_repairs']>0 for i in items),
             'energy_blocked_cases':sum(i['final']['blocked_energy']>0 for i in items),
             'symbol_blocked_cases':sum(i['final']['blocked_symbols']>0 for i in items),
             'harvest_delta':[i['final_graph']['root']['harvests']-i['initial']['root']['harvests'] for i in cases],
             'actions_delta':[i['final_graph']['root']['actions']-i['initial']['root']['actions'] for i in cases],
             'remaining_founder_programs':[i['remaining_founder_programs'] for i in items]}
    for b in (32,64):
        successes=[]
        for seed in SEEDS:
            p=next(x for x in rows if x['name']==f'acquisition-learned-{seed}')
            c=next(x for x in rows if x['name']==f'inheritance-both{b}-{seed}')
            if (p['functional'] and p['new_checker_repairs'] and p['new_recognition_updates'] and
                c['functional'] and c['same_code_as_parent'] and not c['remaining_founder_programs'] and
                not any(p['final']['original_instances'].values()) and not any(c['final']['original_instances'].values())):
                successes.append(seed)
        report['complete_lineages'][str(b)]={'seeds':successes,'of_all_parents':len(SEEDS),'of_functional_parents':sum(r['functional'] for r in rows if r['name'].startswith('acquisition-learned-'))}
    # Stored condition counts are implementation work, not independent samples.
    report['stored_conditions']=len(rows)
    report['population_attempts_executed']=sum(read(source/(r['name']+'.json.gz'))['final_graph']['root']['attempts']-read(source/(r['name']+'.json.gz'))['initial']['root']['attempts'] for r in rows)+850*128*len(SEEDS)
    Path(dest).write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':main(*sys.argv[1:])
