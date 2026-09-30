// Mathematical countermodels, not measurements of engine or hosted capacity.
import test from 'node:test';
import assert from 'node:assert/strict';
const next=[[3,2],[4,2],[5],[4],[5],[]];
const weight=s=>[2,1,0][s%3]+Number(s<3);
test('common credit strictly decreases, while violation count alone permits neutral body updates',()=>{
  for(let s=0;s<6;s++)for(const t of next[s])assert.ok(weight(t)<weight(s));
  assert.equal(0%3,3%3); // The body upgrade leaves pending status unchanged.
  // An unrestricted pending-body toggle would restore the lost credit.
  assert.ok(weight(0)>weight(3));
});
test('finite disturbances charge new work, not an invalid global monotonicity claim',()=>{
  let state=0,charged=weight(state),steps=0;
  for(const disturbance of [1,0,4,3,0]){
    if(next[state].length){state=next[state][0];steps++;}
    charged+=Math.max(0,weight(disturbance)-weight(state));state=disturbance;
  }
  while(next[state].length){state=next[state][0];steps++;}
  assert.equal(state,5);assert.ok(steps<=charged);
});
test('continuing arrivals can keep bounded backlog without global quiescence; overload grows',()=>{
  function workload(arrivals,capacity){let w=0,max=0;for(const a of arrivals){w=Math.max(0,w+a-capacity);max=Math.max(max,w);}return {w,max};}
  assert.deepEqual(workload(Array(1000).fill(3),3),{w:0,max:0});
  assert.deepEqual(workload(Array(1000).fill(3),2),{w:1000,max:1000});
  // Burst 6 every third slot: rate 2, capacity 2, backlog bounded by burst.
  assert.ok(workload(Array.from({length:1000},(_,i)=>i%3===0?6:0),2).max<=6);
});
test('bounded backlog and aggregate service do not imply per-item fairness',()=>{
  // Leave one old unit forever; service each new unit immediately.
  let old=1;
  for(let t=0;t<1000;t++){let fresh=1;fresh--;assert.equal(old+fresh,1);}
  assert.equal(old,1);
  // Any finite wait can precede a fair completion: fairness gives no fixed deadline.
  for(const delay of [1,10,1000]){const run=[...Array(delay).fill('administration'),'repair'];assert.equal(run.indexOf('repair'),delay);}
});
test('determinism selects one answer but does not prove agreement of the original relation',()=>{
  const allowed=['done','audited'];assert.equal(allowed[0],'done');assert.equal(new Set(allowed).size,2);
  // Equal output can hide distinct irreducible internal states.
  const terminal=[{output:'done',body:'A'},{output:'done',body:'B'}];
  assert.equal(new Set(terminal.map(s=>s.output)).size,1);assert.notDeepEqual(...terminal);
});
