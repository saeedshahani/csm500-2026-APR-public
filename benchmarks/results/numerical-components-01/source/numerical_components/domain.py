"""Finite substochastic Bellman modules; exact synthesis and separate certificate.
One scalar boundary, arbitrary finite state/action counts (resource caps explicit).
No source or target oracle is imported by the executor.
"""
from dataclasses import dataclass
from fractions import Fraction as Q
from itertools import product, combinations
import random

@dataclass(frozen=True)
class Action:
    weights: tuple  # retry transition probabilities, sum < 1
    reward: Q
    boundary: Q

class Meter:
    def __init__(self): self.ops=0; self.max_bits=0
    def charge(self, n=1, values=()):
        self.ops+=n
        for v in values:
            if isinstance(v,Q): self.max_bits=max(self.max_bits,v.numerator.bit_length(),v.denominator.bit_length())

def make_task(level, size, seed):
    rng=random.Random(seed*1000003+level*103+size)
    rows=[]
    for i in range(size):
        actions=[]
        for k in range(2):
            d=(level+2)*(k+1); q=1-Q(1,d)
            w=[Q(0)]*size
            w[(i+1)%size]+=q*Q(3,4); w[(i-1)%size]+=q*Q(1,4)
            s=Q(rng.randrange(2,7),8); r=Q(rng.randrange(9-int(s*8)),8)
            actions.append(Action(tuple(w),r/d,s/d))
        rows.append(tuple(actions))
    return tuple(rows)

def validate_task(task):
    n=len(task)
    return n>0 and all(row and all(len(a.weights)==n and all(w>=0 for w in a.weights)
        and sum(a.weights)<1 and a.reward>=0 and a.boundary>=0
        and sum(a.weights)+a.reward+a.boundary<=1 for a in row) for row in task)

def perturb(task,delta,high):
    return tuple(tuple(Action(a.weights,(1-delta)*a.reward+delta*(1-sum(a.weights))*high,
                              (1-delta)*a.boundary) for a in row) for row in task)

def action(a,x,p,meter=None):
    y=a.reward+a.boundary*p+sum((w*v for w,v in zip(a.weights,x)),Q(0))
    if meter: meter.charge(2*len(x)+3,(y,))
    return y

def bellman(task,x,p,meter=None):
    return tuple(max(action(a,x,p,meter) for a in row) for row in task)

def gaussian(matrix,rhs,meter=None):
    n=len(matrix); k=len(rhs[0]); a=[list(matrix[i])+list(rhs[i]) for i in range(n)]
    for col in range(n):
        pivot=next((r for r in range(col,n) if a[r][col]),None)
        if pivot is None: raise ValueError('singular policy')
        a[col],a[pivot]=a[pivot],a[col];d=a[col][col]
        a[col]=[v/d for v in a[col]]
        if meter: meter.charge(n+k,a[col])
        for r in range(n):
            if r!=col:
                f=a[r][col];a[r]=[u-f*v for u,v in zip(a[r],a[col])]
                if meter: meter.charge(2*(n+k),a[r])
    return tuple(tuple(row[n:]) for row in a)

def synthesize(task,meter=None):
    if not validate_task(task): raise ValueError('invalid task')
    count=1
    for row in task: count*=len(row)
    if len(task)>4 or count>16: raise ValueError('declared synthesis resource cap')
    n=len(task); lines=[set() for _ in task]
    for policy in product(*(range(len(row)) for row in task)):
        chosen=[task[i][policy[i]] for i in range(n)]
        mat=[[Q(i==j)-chosen[i].weights[j] for j in range(n)] for i in range(n)]
        sol=gaussian(mat,[(a.reward,a.boundary) for a in chosen],meter)
        for i in range(n): lines[i].add(sol[i])
    return tuple(tuple(sorted(row)) for row in lines)

def upper(lines,p,meter=None):
    values=[c+d*p for c,d in lines]
    if meter: meter.charge(2*len(lines),values)
    return max(values)

def certify(task,program,meter=None):
    """All-input proof certificate: piecewise affine residual and touching action.
    Independent of policy enumeration/Gaussian elimination. All input p in [0,1].
    """
    if not validate_task(task) or len(program)!=len(task) or any(not row for row in program):return False,0
    if any(len(line)!=2 or any(type(v) is not Q for v in line) for row in program for line in row): return False,0
    if any(c<0 or d<0 or c+d>1 for row in program for c,d in row): return False,0
    points={Q(0),Q(1)}
    for row in program:
        for (c,d),(e,f) in combinations(row,2):
            if d!=f:
                p=(e-c)/(d-f)
                if 0<p<1:points.add(p)
    points=sorted(points)
    vals={p:tuple(upper(row,p,meter) for row in program) for p in points}
    for l,r in zip(points,points[1:]):
        for i,row in enumerate(task):
            residuals=[(vals[l][i]-action(a,vals[l],l,meter),vals[r][i]-action(a,vals[r],r,meter)) for a in row]
            if any(a<0 or b<0 for a,b in residuals) or not any(a==0 and b==0 for a,b in residuals):return False,len(points)
    return True,len(points)

def boundary_lipschitz(task):
    return max(a.boundary/(1-sum(a.weights)) for row in task for a in row)
