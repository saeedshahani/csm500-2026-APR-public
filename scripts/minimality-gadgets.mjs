// Complete finite labelled search; no symmetry pruning or acyclicity assumption.
import assert from 'node:assert/strict';
export function permutations(a) {if(!a.length)return [[]];return a.flatMap((x,i)=>permutations(a.filter((_,j)=>j!==i)).map(p=>[x,...p]));}
export function inspect(rows,i) {
  const n=i+2, reached=new Set([0]), todo=[0];
  while(todo.length){const x=todo.pop();for(const y of rows[x]??[])if(!reached.has(y)){reached.add(y);todo.push(y);}}
  if(reached.size!==n)return null;
  const edges=rows.reduce((s,r)=>s+r.length,0);
  const swappable=permutations(Array.from({length:i-1},(_,j)=>j+1)).some(p=>{
    const map=[0,...p,i+1,i];
    return rows.every((r,x)=>r.every(y=>(rows[map[x]]??[]).includes(map[y])));
  });
  return {edges,ordered:!swappable};
}
export function enumerate(i,independent=false) {
  const n=i+2, result={internal:i,total:0,reachable:0,ordered:0,min_edges:null,witness:null};
  function accept(rows) {
    result.total++;const found=inspect(rows,i);if(!found)return;
    result.reachable++;if(!found.ordered)return;result.ordered++;
    if(result.min_edges===null||found.edges<result.min_edges){result.min_edges=found.edges;result.witness=rows;}
  }
  if(independent) {
    assert.ok(i<=3);
    // Independent representation: every adjacency bitmask, reject row capacity >2.
    for(let bits=0;bits<2**(i*n);bits++){
      const rows=Array.from({length:i},(_,x)=>Array.from({length:n},(_,y)=>y).filter(y=>Math.floor(bits/2**(x*n+y))%2));
      if(rows.some(r=>r.length>2))continue;
      accept(rows);
    }
  } else {
    const choices=[[],...Array.from({length:n},(_,j)=>[j])];
    for(let a=0;a<n;a++)for(let b=a+1;b<n;b++)choices.push([a,b]);
    const visit=rows=>{if(rows.length===i){accept(rows);return;}for(const row of choices)visit([...rows,row]);};
    visit([]);
  }
  return result;
}
export function verifyWitness(row) {
  if(row.witness===null){assert.equal(row.internal,1);return;}
  const {witness:g,internal:i}=row;
  assert.ok(g.length===i && g.every(r=>r.length<=2 && new Set(r).size===r.length));
  // An independent full permutation check (including terminals) for the certificate.
  for(const p of permutations(Array.from({length:i+2},(_,j)=>j))) {
    if(p[0]!==0||p[i]!==i+1||p[i+1]!==i)continue;
    const rel=new Set(g.flatMap((r,x)=>r.map(y=>`${x},${y}`)));
    const mapped=new Set(g.flatMap((r,x)=>r.map(y=>`${p[x]},${p[y]}`)));
    assert.ok([...rel].some(e=>!mapped.has(e)),'certificate has a boundary swap');
  }
  assert.equal(g.reduce((s,r)=>s+r.length,0),row.min_edges);
}
