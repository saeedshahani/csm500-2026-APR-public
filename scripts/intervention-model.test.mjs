import test from 'node:test';
import assert from 'node:assert/strict';

test('signed quotation formula agrees with independent incidence enumeration', () => {
  // Predicate interventions, NOT a claim that all valuations are valid/reachable
  // native carriers. m targets c; l/r target m. Quoting m makes it inoperative.
  for(let mask=0;mask<8;mask++) {
    const [m,l,r]=[0,1,2].map(i=>Boolean(mask&2**i));
    const memberships=[m&&['m','c'],l&&['l','m'],r&&['r','m']].filter(Boolean);
    const named=new Set(memberships.map(x=>x[1]));
    const operative=memberships.filter(([id])=>!named.has(id));
    const visible=!operative.some(([,target])=>target==='c');
    assert.equal(visible,!m||l||r);
  }
});

test('minimal positive supports alone do not describe a nonmonotone truth table', () => {
  // Native new-quotation/removal test has admitted masks 00 and 11; 01 is
  // blocked and 10 is infeasible. The only minimal support is empty, which
  // cannot be read as permission to retain arbitrary additional events.
  const admitted=[true,false,false,true];
  const minimal=admitted.flatMap((yes,s)=>yes&&!admitted.some((other,t)=>other&&t!==s&&(s&t)===t)?[s]:[]);
  assert.deepEqual(minimal,[0]);
  assert.notEqual(admitted[1],minimal.some(s=>(s&1)===s));
});

test('direct edges cannot preserve both conjunction and alternative support', () => {
  const conjunction=[[0,1]],alternative=[[0],[1]];
  assert.deepEqual([...new Set(conjunction.flat())],[...new Set(alternative.flat())]);
  const holds=(supports,retained)=>supports.some(s=>s.every(i=>retained.includes(i)));
  assert.equal(holds(conjunction,[0]),false);
  assert.equal(holds(alternative,[0]),true);
});

test('single-deletion tests miss redundant alternatives', () => {
  const enabled=mask=>mask!==0;
  assert.ok(enabled(3)&&enabled(2)&&enabled(1));
  assert.equal(enabled(0),false);
});

test('all 256 three-variable Boolean predicates retain exact signed cases', () => {
  for(let truth=0;truth<256;truth++) {
    const rows=Array.from({length:8},(_,mask)=>({present:[0,1,2].filter(i=>mask&2**i),absent:[0,1,2].filter(i=>!(mask&2**i)),value:Boolean(truth&2**mask)}));
    for(let mask=0;mask<8;mask++) {
      const matches=rows.filter(row=>row.present.every(i=>mask&2**i)&&row.absent.every(i=>!(mask&2**i)));
      assert.equal(matches.length,1);
      assert.equal(matches[0].value,Boolean(truth&2**mask));
    }
  }
});
