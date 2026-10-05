"""Independent analytical oracle, never imported by the Rust application."""
from math import isfinite,sqrt

def positive_equilibria(t,r,dg,dp):
    if not all(isfinite(x) and x>0 for x in (t,r,dg,dp)):
        raise ValueError('Proposition 3 requires finite strictly positive rates')
    delta=(r-2*dg)**2-8*r*dg*dp/t
    if r<=2*dg or delta<=0:
        raise ValueError('Proposition 3 requires r>2dg and strict positive discriminant')
    high=(r-2*dg+sqrt(delta))/(2*r)
    # Product of roots avoids catastrophic subtraction at a small lower root.
    low=(2*dg*dp/(r*t))/high
    return [(g,t*g/(t*g+dp)) for g in (low,high)]

def blocks(t,r,dg,dp,g,a):
    return [[-r*a*g/2,r*g*(1-g)/2],[t*(1-a),-t*g-dp]], [[0,-r*g/2],[t,-t*g-dp]]


def four_drift(state,t,r,dg,dp,mode='reciprocal'):
    """Ideal replacement dynamics; not the finite-resource noisy simulator."""
    g0,g1,a0,a1=state;g=g0+g1
    if mode=='reciprocal':b0,b1=r*g0*a1,r*g1*a0
    elif mode=='self':b0,b1=r*g0*a0,r*g1*a1
    else:raise ValueError('unknown recognition mode')
    total=b0+b1
    return [b0-(total+dg)*g0,b1-(total+dg)*g1,
            t*g0-(t*g+dp)*a0,t*g1-(t*g+dp)*a1]
