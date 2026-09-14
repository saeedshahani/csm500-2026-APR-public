import {test} from 'node:test';
import assert from 'node:assert/strict';
import {parseProject,readUtf8} from '../src/projects.ts';
const valid={schema:'reflective-project-v1',source:'program {} // Ελληνικά',draft:'draft 🐈',trace:null,execution:null,position:'0',presentation:{theme:'dark',view:'Practical'},config:{max_steps:'20',max_work:'1000000',max_candidates:'1000',seed:'42',detect_cycles:false,observer:'observer { all; }'}};
test('project format preserves Unicode and keeps presentation separate',()=>assert.deepEqual(parseProject(JSON.stringify(valid)),valid));
test('file input preserves Unicode and rejects invalid UTF-8',async()=>{assert.equal(await readUtf8(new File(['λ 🐈'],'source.graph')),'λ 🐈');await assert.rejects(readUtf8(new File([new Uint8Array([0xff])],'bad.graph')));});
test('project validation rejects unknown, malformed and over-budget parts',()=>{
 for(const change of [{extra:1},{schema:'v2'},{source:null},{position:'1'},{execution:{}},{presentation:{theme:'blue',view:'Practical'}},{config:{...valid.config,seed:'1e9'}},{config:{...valid.config,max_work:'1000001'}},{config:{...valid.config,extra:0}},{draft:'x'.repeat(4*1024*1024+1)},{trace:'{}'}])assert.throws(()=>parseProject(JSON.stringify({...valid,...change})));
});
