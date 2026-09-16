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
 const measured=rows.filter(r=>r.repetition>=0),rss=measured.map(r=>r.peak_rss_bytes).sort((a,b)=>a-b);
 assert.deepEqual(item.peak_rss_bytes,{n:rss.length,min:rss[0],median:rss[Math.floor(rss.length/2)],max:rss.at(-1)});
 assert.deepEqual(item.failures,[]);
 assert.deepEqual(item.outcomes,[...new Set(measured.map(r=>r.result.outcome))]);
 assert.deepEqual(item.details,measured[0].result);
 for(const row of measured){const {wall_ns:time,...details}=row.result;const {wall_ns:baseline,...expected}=item.details;assert.deepEqual(details,expected);assert.ok(time===null||time>=0);assert.ok(baseline===null||baseline>=0);}
}
for(const n of [10,100,1000]){const run=summary.find(r=>r.family==='practical'&&r.size===n&&r.phase==='run');assert.deepEqual(run.outcomes,[n===1000?'ResourceLimit':'NormalForm']);assert.equal(run.details.commits,n===1000?0:n);}
console.log('Verified 226 cells, 1,130 measured samples, 226 warm-ups, protocol digest, time/RSS summaries, structural counts and outcomes.');
