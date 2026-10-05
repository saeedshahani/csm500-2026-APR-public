"""Exact differentiation checks the stated blocks against the full vector field."""
from fractions import Fraction as F
import itertools
import unittest
from tests.oracles.organization.theory import four_drift, blocks

class Dual:
    def __init__(self,x,dx=None):self.x=F(x);self.dx=dx if dx is not None else [F(0)]*4
    def __add__(self,o):
        if not isinstance(o,Dual):o=Dual(o)
        return Dual(self.x+o.x,[a+b for a,b in zip(self.dx,o.dx)])
    __radd__=__add__
    def __neg__(self):return Dual(-self.x,[-x for x in self.dx])
    def __sub__(self,o):return self+(-o if isinstance(o,Dual) else Dual(-o))
    def __rsub__(self,o):return Dual(o)+-self
    def __mul__(self,o):
        if not isinstance(o,Dual):o=Dual(o)
        return Dual(self.x*o.x,[a*o.x+self.x*b for a,b in zip(self.dx,o.dx)])
    __rmul__=__mul__

def multiply(a,b):return [[sum(x*y for x,y in zip(row,col)) for col in zip(*b)] for row in a]

class JacobianTests(unittest.TestCase):
    def test_exact_full_field_blocks_for_reciprocal_and_self_recognition(self):
        transform=[[1,1,0,0],[0,0,1,1],[1,-1,0,0],[0,0,1,-1]]
        inverse=[[F(1,2),0,F(1,2),0],[F(1,2),0,F(-1,2),0],
                 [0,F(1,2),0,F(1,2)],[0,F(1,2),0,F(-1,2)]]
        count=0
        for g,t,r,dp in itertools.product([F(1,10),F(1,3),F(2,3),F(9,10)],
                                           [F(5),F(20),F(100)],[F(40),F(100),F(400)],[F(1),F(5),F(10)]):
            a=t*g/(t*g+dp);dg=r*a*(1-g)/2
            for mode,sign in [('reciprocal',-1),('self',1)]:
                state=[Dual(x,[F(i==j) for j in range(4)]) for i,x in enumerate([g/2,g/2,a/2,a/2])]
                field=four_drift(state,t,r,dg,dp,mode)
                self.assertTrue(all(z.x==0 for z in field))
                transformed=multiply(multiply(transform,[z.dx for z in field]),inverse)
                total,imbalance=blocks(t,r,dg,dp,g,a);imbalance[0][1]=sign*r*g/2
                expected=[total[0]+[0,0],total[1]+[0,0],[0,0]+imbalance[0],[0,0]+imbalance[1]]
                self.assertEqual(transformed,expected);count+=1
        self.assertEqual(count,216)
