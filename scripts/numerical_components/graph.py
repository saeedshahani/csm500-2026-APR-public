"""Ordinary graph-rule source emission; matching/validation remain native."""

def rule(name,atoms,lhs,rhs):
    import re
    names=set(atoms)
    rename=lambda text: re.sub(r'"[^"]*"|\b[A-Za-z][A-Za-z0-9_]*\b',lambda m:name+'_'+m[0] if m[0] in names else m[0],text)
    declarations='\n'.join(f'atom {name}_{n} = {rename(v)};' for n,v in atoms.items())
    ls=' '.join(f'{name}_lhs{k}: {name}_{n};' for k,n in enumerate(lhs));rs=' '.join(f'{name}_rhs{k}: {name}_{n};' for k,n in enumerate(rhs))
    return f'''rule {name} {{
atom {name}_zero = Nat(); rank {name}_rank = {name}_zero;
mode {name}_mode = ModeObject; revision {name}_version; active {name}_enabled;
{declarations}
lhs {{ {ls} }} rhs {{ {rs} }}
}}'''
