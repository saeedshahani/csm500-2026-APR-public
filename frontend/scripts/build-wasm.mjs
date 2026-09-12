import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
const root=fileURLToPath(new URL('../../',import.meta.url));
const bin=path.join(root,'target','tools','bin');
const executable=path.join(bin,process.platform==='win32'?'wasm-bindgen.exe':'wasm-bindgen');
let installed=false;
try{installed=execFileSync(executable,['--version'],{encoding:'utf8'}).trim()==='wasm-bindgen 0.2.100';}catch{/* Install the exact tool with its published dependency lockfile. */}
if(!installed)execFileSync('cargo',['install','wasm-bindgen-cli','--version','0.2.100','--locked','--root',path.join(root,'target','tools')],{cwd:root,stdio:'inherit'});
const target=process.argv[2];if(!['web','nodejs'].includes(target))throw new Error('Specify web or nodejs');
execFileSync(process.platform==='win32'?'wasm-pack.cmd':'wasm-pack',['build','../crates/wasm-bridge','--mode','no-install','--no-opt','--target',target,'--out-dir',`../../frontend/${target==='web'?'pkg':'pkg-node'}`,'--release','--','--locked'],{cwd:path.join(root,'frontend'),stdio:'inherit',env:{...process.env,PATH:bin+path.delimiter+process.env.PATH}});
