"""Independent exact oracle: Cramer's rule + Bellman optimality, no compiler calls."""
from fractions import Fraction as Q
from itertools import product,permutations

def determinant(a,meter=None):
    n=len(a); total=Q(0)
    for p in permutations(range(n)):
        inversions=sum(p[i]>p[j] for i in range(n) for j in range(i+1,n))
        term=Q(-1 if inversions%2 else 1)
        for i in range(n): term*=a[i][p[i]]
        total+=term
        if meter:meter.charge(n+1,(total,term))
    return total

def solve(task,parent,meter=None):
    n=len(task)
    for pi in product(*(range(len(row)) for row in task)):
        mat=[[Q(i==j)-task[i][pi[i]].weights[j] for j in range(n)] for i in range(n)]
        rhs=[task[i][pi[i]].reward+task[i][pi[i]].boundary*parent for i in range(n)]
        den=determinant(mat,meter)
        if not den:continue
        x=[]
        for j in range(n):
            replaced=[row[:] for row in mat]
            for i in range(n):replaced[i][j]=rhs[i]
            x.append(determinant(replaced,meter)/den)
        if meter:meter.charge(n*n+3*n+sum(len(row)*(2*n+3) for row in task),x)
        if all(x[i]==max(a.reward+a.boundary*parent+sum(a.weights[j]*x[j] for j in range(n)) for a in row) for i,row in enumerate(task)):
            return tuple(x)
    raise AssertionError('no exact optimal policy')

def solve_chain(tasks,boundary,meter=None):
    values=[];p=boundary
    for task in tasks:
        x=solve(task,p,meter);values.append(x);p=x[0]
    return values
