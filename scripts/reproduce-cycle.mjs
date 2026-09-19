// One-entry, offline, fresh-build semantic reproduction. Does not claim identical
// binaries, independent hardware, production deployment or repeatable timings.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {execFileSync,spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {gzipSync,gunzipSync} from 'node:zlib';
const sha=b=>createHash('sha256').update(b).digest('hex');
const args=process.argv.slice(2),verify=args[0]==='--verify';
if((verify?args.length!==2:args.length!==1))throw Error('node scripts/reproduce-cycle.mjs NEW_OUTPUT | --verify OUTPUT');
const output=path.resolve(args[verify?1:0]);
if(verify){
  const manifest=JSON.parse(fs.readFileSync(path.join(output,'manifest.json')));
  for(const [file,expected] of Object.entries(manifest.artifacts)){
    if(path.basename(file)!==file)throw Error('Invalid artifact path');
    if(sha(fs.readFileSync(path.join(output,file)))!==expected)throw Error('Changed artifact: '+file);
  }
  const bundle=JSON.parse(gunzipSync(fs.readFileSync(path.join(output,'sources.json.gz'))));
  for(const [file,content] of Object.entries(bundle))if(sha(Buffer.from(content,'base64'))!==manifest.sources[file])throw Error('Changed source: '+file);
  if(Object.keys(bundle).length!==Object.keys(manifest.sources).length)throw Error('Source inventory mismatch');
  console.log('Verified source bundle and retained outputs; rerun build/experiments separately.');
}else{
  fs.mkdirSync(output,{recursive:false});
  const clean=fs.mkdtempSync(path.join(os.tmpdir(),'reflective-reproduce-'));
  const tracked=execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n');
  const files=[...new Set(tracked)].filter(f=>fs.existsSync(f)&&(/^(Cargo\.(toml|lock)|rust-toolchain\.toml)$/.test(f)||/^crates\/.+\.(rs|toml|txt)$/.test(f)||/^examples\/.+\.(atoms|graph|lambda|observer|json|md)$/.test(f)||f==='scripts/reproduce-cycle.mjs')).sort();
  const bundle={},sources={},artifacts={};
  const save=(name,bytes)=>{fs.writeFileSync(path.join(output,name),bytes,{flag:'wx'});artifacts[name]=sha(bytes);};
  for(const file of files){
    if(path.isAbsolute(file)||file.split('/').includes('..'))throw Error('Unsafe source path');
    const bytes=fs.readFileSync(file);bundle[file]=bytes.toString('base64');sources[file]=sha(bytes);
    fs.mkdirSync(path.dirname(path.join(clean,file)),{recursive:true});fs.writeFileSync(path.join(clean,file),bytes,{flag:'wx'});
  }
  save('sources.json.gz',gzipSync(JSON.stringify(bundle)));
  const command=(cwd,cmd,argv,input)=>{
    const result=spawnSync(cmd,argv,{cwd,input,encoding:'utf8',timeout:300000,maxBuffer:32*1024*1024,
      env:{...process.env,CARGO_TARGET_DIR:path.join(cwd,'target')}});
    if(result.error||result.signal)throw result.error??Error(result.signal);
    return result;
  };
  const results=[];
  try{
    for(const [name,cwd] of [['original',process.cwd()],['clean',clean]]){
      const build=command(cwd,'cargo',['build','--release','--locked','--offline','-p','cli']);
      save(`${name}-build.log`,build.stdout+build.stderr);if(build.status!==0)throw Error(`${name} build failed`);
      const binary=path.join(cwd,'target/release/cli');
      const runs=[];
      for(const [fixture,steps,extra] of [['programs/arithmetic',1,[]],['programs/self-modify',3,['--strategy','seeded','--seed','42']],['behaviour/oscillating',4,['--detect-cycles','true']]]){
        const run=command(cwd,binary,['export',`examples/${fixture}.graph`,'--max-steps',String(steps),...extra]);
        if(![0,3,6].includes(run.status))throw Error(`Fixture failed: ${fixture}: ${run.stderr}`);
        const checked=command(cwd,binary,['trace-check','-'],run.stdout);if(checked.status!==0)throw Error('Replay failed');
        const key=fixture.replace('/','-');save(`${name}-${key}.json`,run.stdout);
        runs.push({fixture,steps,extra,exit:run.status,sha256:sha(run.stdout)});
      }
      const tests=command(cwd,'cargo',['test','--locked','--offline','-p','engine','--test','conditions']);
      save(`${name}-conditions.log`,tests.stdout+tests.stderr);if(tests.status!==0)throw Error('Condition oracle failed');
      results.push({name,binary_sha256:sha(fs.readFileSync(binary)),runs});
    }
    if(JSON.stringify(results[0].runs)!==JSON.stringify(results[1].runs))throw Error('Fresh build disagrees on exact traces');
    const manifest={schema:'reflective-cycle-reproduction-v1',created_at:new Date().toISOString(),
      commit:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),
      dirty:execFileSync('git',['status','--porcelain'],{encoding:'utf8'}).trim().split('\n').filter(Boolean),
      disposition:'Source bundle, not commit alone, identifies tested bytes. Unrelated dirty files may be listed but are not included unless in the source inventory.',
      environment:{os:os.type(),release:os.release(),arch:os.arch(),node:process.version,rust:execFileSync('rustc',['--version'],{encoding:'utf8'}).trim(),cargo:execFileSync('cargo',['--version'],{encoding:'utf8'}).trim()},
      policy:{observer:'complete carrier',limits:'CLI defaults except explicit arguments',comparison:'exact trace bytes; numeric tolerance zero',build:'separate source and target directories, shared toolchain/dependency cache; offline locked build',measurements:'No timing or memory repeatability claim',binary_reproducibility:'Digests recorded, not required equal'},sources,artifacts,results};
    fs.writeFileSync(path.join(output,'manifest.json'),JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
    console.log(JSON.stringify({output,exact_trace_agreement:true,source_files:files.length,binary_equal:results[0].binary_sha256===results[1].binary_sha256}));
  }catch(error){
    fs.writeFileSync(path.join(output,'failure.json'),JSON.stringify({schema:'reflective-reproduction-failure-v1',message:String(error),sources,artifacts,results},null,2)+'\n',{flag:'wx'});
    throw error;
  }finally{fs.rmSync(clean,{recursive:true,force:true});}
}
