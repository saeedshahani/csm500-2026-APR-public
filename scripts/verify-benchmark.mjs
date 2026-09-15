// Validate retained evidence without rerunning or reinterpreting measurements.
import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const root=process.argv[2]??'benchmarks/results/s5-native';
const read=name=>JSON.parse(readFileSync(`${root}/${name}`,'utf8'));
const env=read('environment.json'),matrix=read('matrix.json'),raw=read('raw.json'),summary=read('summary.json');
assert.equal(createHash('sha256').update(readFileSync(`${root}/protocol.md`)).digest('hex'),env.protocol_sha256);
assert.equal(matrix.length,226);assert.equal(summary.length,226);assert.equal(raw.length,226*6);
for(const [family,size,phase] of matrix){
 const rows=raw.filter(r=>r.family===family&&r.size===size&&r.phase===phase);
 assert.equal(rows.length,6);assert.deepEqual(rows.map(r=>r.repetition),[-1,0,1,2,3,4]);
 const item=summary.find(r=>r.family===family&&r.size===size&&r.phase===phase);assert.ok(item);
 for(const row of rows){assert.equal(row.exit_code,0);assert.equal(row.censored,null);assert.ok(row.peak_rss_bytes>0);assert.equal(row.result.family,family);assert.equal(row.result.size,size);assert.equal(row.result.phase,phase);}
 const values=rows.filter(r=>r.repetition>=0).map(r=>r.result.wall_ns).filter(n=>typeof n==='number').sort((a,b)=>a-b);
 assert.deepEqual(item.wall_ns,values.length?{n:values.length,min:values[0],median:values[Math.floor(values.length/2)],max:values.at(-1)}:null);
}
for(const n of [10,100,1000]){const run=summary.find(r=>r.family==='practical'&&r.size===n&&r.phase==='run');assert.deepEqual(run.outcomes,[n===1000?'ResourceLimit':'NormalForm']);assert.equal(run.details.commits,n===1000?0:n);}
console.log('Verified 226 cells, 1,130 measured samples, 226 warm-ups, protocol digest and practical outcomes.');
