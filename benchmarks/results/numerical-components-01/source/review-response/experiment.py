"""Acceptance campaign for the complete hybrid path; bounded deterministic inputs."""
import argparse
import json
import random
import sys
from pathlib import Path
from fractions import Fraction as Q
SOURCE = Path(__file__).resolve().parent
COMPONENT = SOURCE.parent/'numerical_components'
if not COMPONENT.is_dir():
    COMPONENT = SOURCE.parents[4]/'scripts/numerical_components'
sys.path.insert(0, str(COMPONENT))
from adapter import Component, checked_task, task_data, verify_packet
from test_adapter import TASK
from oracle import solve as exact_solution


def intervals(data, boundary):
    """Separate outward interval iteration, no adapter Bellman/linear solver calls."""
    scale = 1 << 64
    lo, hi = [0]*len(data), [scale]*len(data)
    for _ in range(400):
        nextlo, nexthi = [], []
        for row in data:
            lower = max(Q(a['reward'])*scale + Q(a['boundary'])*boundary*scale
                        + sum(Q(w)*v for w,v in zip(a['weights'], lo)) for a in row)
            upper = max(Q(a['reward'])*scale + Q(a['boundary'])*boundary*scale
                        + sum(Q(w)*v for w,v in zip(a['weights'], hi)) for a in row)
            nextlo.append(lower.numerator//lower.denominator)
            nexthi.append(-(-upper.numerator//upper.denominator))
        if (lo, hi) == (nextlo, nexthi): break
        lo, hi = nextlo, nexthi
    return [(Q(a,scale), Q(b,scale)) for a,b in zip(lo,hi)]


def generated(rng, n):
    task=[]
    for _ in range(n):
        row=[]
        for _ in range(2):
            weights=[rng.randrange(1,8) for _ in range(n)]
            mass=Q(rng.randrange(1,4),4)
            reward=Q(rng.randrange(9),8)
            boundary=Q(rng.randrange(9-int(reward*8)),8)
            row.append({'weights':[str(mass*w/sum(weights)) for w in weights],
                        'reward':str((1-mass)*reward), 'boundary':str((1-mass)*boundary)})
        task.append(row)
    return task


def run(out, cli):
    out=Path(out);out.mkdir(exist_ok=False)
    rng=random.Random(6100526)
    checks=0; cases=[]; native=[]
    for size in (1,2,3,4):
        for seed in range(12):
            task=generated(rng,size); p=Q(rng.randrange(17),16)
            enclosure=intervals(task,p)
            exact=exact_solution(checked_task(task),p)
            held_exact=exact_solution(checked_task(task),1-p)
            for mode in ('raw','compiled','direct'):
                c=Component(f'workflow-{size}-{seed}',task,p)
                r,cost=c.compute(mode,steps=64,bits=24)
                packet=c.assess(r,'1/1000',0)
                verify_packet(packet)
                rad=Q(packet['radius_now']); values=list(map(Q,r.data()['values']))
                for y,target,(lo,hi) in zip(values,exact,enclosure):
                    assert hi-lo < Q(1,10**12)
                    assert lo <= target <= hi
                    assert abs(y-target) <= rad
                    assert max(abs(y-lo),abs(y-hi)) <= rad+Q(1,10**12)
                    checks+=1
                # Held output at a genuinely different sampled environment.
                newp=1-p;c.observe(newp,1,1)
                held=c.assess(r,'1/1000',1);verify_packet(held)
                for y,target,(lo,hi) in zip(values,held_exact,intervals(task,newp)):
                    assert lo <= target <= hi
                    assert abs(y-target) <= Q(held['radius_now'])
                    assert max(abs(y-lo),abs(y-hi)) <= Q(held['radius_now'])+Q(1,10**12)
                    checks+=1
                cases.append({'size':size,'seed':seed,'mode':mode,'packet':packet,
                              'held':held,'cost':cost})
                if seed == 0:
                    native.append(c.publish(r,'1/1000',1,cli,out/f'native-{size}-{mode}'))
    # Strongly distinguish supported certification, evidence insufficiency and staleness.
    c=Component('end-to-end',TASK,'1/4')
    old,_=c.compute('raw',0)
    native.append(c.publish(old,'1/1000',0,cli,out/'native-uncertified'))
    exact,_=c.compute('compiled')
    native.append(c.publish(exact,'1/1000',0,cli,out/'native-certified'))
    native.append(c.publish(exact,'1/1000',33,cli,out/'native-expired'))
    c.revise(TASK,'3/4',34)
    native.append(c.publish(exact,'1/1000',34,cli,out/'native-revision-stale'))
    current,_=c.compute('direct')
    c.depart()
    native.append(c.publish(current,'1/1000',35,cli,out/'native-departed'))
    c.rejoin('1/4',36)
    native.append(c.publish(current,'1/1000',36,cli,out/'native-rejoined-stale'))
    current,_=c.compute('direct')
    native.append(c.publish(current,'1/1000',36,cli,out/'native-rejoined-current'))
    try:c.observe('3/4',1,37,'0')
    except ValueError:pass
    else:raise AssertionError('understated movement accepted')
    native.append(c.publish(current,'1/1000',37,cli,out/'native-monitor-stale'))
    # Values and tolerances grow in bit length, never as unary graph chains.
    for bits in (8,16,32,64,96,127):
        boundary=Q((1 << (bits-1))-1,1 << (bits-1))
        c=Component(f'precision-{bits}',TASK,boundary)
        receipt,_=c.compute('direct',bits=64)
        native.append(c.publish(receipt,Q(1,1 << (bits-1)),0,cli,out/f'native-bits-{bits}'))
    # Declared sequence completeness is indispensable: same samples cannot detect a hidden excursion.
    sampling_counterexample={'observed_samples':['1/4','1/4'],
        'possible_unobserved_path':['1/4','1','1/4'],
        'observed_variation':'0','actual_path_variation':'3/2',
        'conclusion':'monitor covers submitted samples, not unsensed changes'}
    report={'cases':cases,'numerical_cases':len(cases),'exact_bound_checks':checks, 'interval_enclosure_checks':checks,
            'native':native,'native_cases':len(native),'sampling_counterexample':sampling_counterexample,
            'status_counts':{status:sum(r['status']==status for r in native)
                             for status in ('certified','uncertified','stale')}}
    (out/'results.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps({k:v for k,v in report.items() if k not in ('cases','native')},indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--out',required=True);parser.add_argument('--cli',required=True)
    args=parser.parse_args();run(args.out,args.cli)
