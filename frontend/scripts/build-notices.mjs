// Preserve dependency notices in the binary distribution. This does not license
// project-authored code or replace notices belonging to separately shipped tools.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,readdirSync,statSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';

export function rustNotices(root) {
  const metadata=JSON.parse(execFileSync('cargo',['metadata','--locked','--offline',
    '--filter-platform','wasm32-unknown-unknown','--format-version','1'],
  {cwd:root,encoding:'utf8',maxBuffer:32*1024*1024}));
  const packages=new Map(metadata.packages.map(p=>[p.id,p]));
  const nodes=new Map(metadata.resolve.nodes.map(n=>[n.id,n]));
  const bridge=metadata.packages.find(p=>p.name==='wasm-bridge'&&!p.source);
  assert.ok(bridge,'missing workspace bridge');
  const visited=new Set(),pending=[bridge.id];
  while(pending.length) {
    const id=pending.pop();if(visited.has(id))continue;visited.add(id);
    for(const dependency of nodes.get(id).deps)
      if(dependency.dep_kinds.some(k=>k.kind===null))pending.push(dependency.pkg);
  }
  const dependencies=[...visited].map(id=>packages.get(id)).filter(p=>p.source)
    .sort((a,b)=>a.name.localeCompare(b.name)||a.version.localeCompare(b.version));
  const sections=['# Rust WebAssembly dependencies\n\nNormal dependencies of the locked WebAssembly build, including compile-time macro support. Project-authored packages are excluded.'];
  for(const p of dependencies) {
    const directory=dirname(p.manifest_path);
    const candidates=readdirSync(directory).filter(f=>/^(licen[cs]e|copying|notice)(?:[.-]|$)/i.test(f));
    if(p.license_file)candidates.push(p.license_file);
    const files=[...new Set(candidates)].filter(f=>statSync(join(directory,f)).isFile()).sort();
    assert.ok(files.length,`missing dependency notice: ${p.name} ${p.version}`);
    const texts=files.map(f=>`### ${f}\n\n${readFileSync(join(directory,f),'utf8').trim()}`);
    sections.push(`## ${p.name} ${p.version}\n\nDeclared licence: ${p.license??'see licence file'}.\n\n${texts.join('\n\n')}`);
  }
  return sections.join('\n\n')+'\n';
}

if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  const root=fileURLToPath(new URL('../../',import.meta.url));
  const output=join(root,'frontend/dist/third-party-notices.md');
  const javascript=readFileSync(output,'utf8');
  assert.ok(javascript.includes('react')&&javascript.includes('firebase'),'missing bundled JavaScript notices');
  assert.ok(!javascript.includes('# Rust WebAssembly dependencies'),'notices already assembled');
  writeFileSync(output,javascript+'\n'+rustNotices(root));
}
