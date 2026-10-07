// Source collection must not depend on optional developer search tools.
import {readdirSync} from 'node:fs';

// Rust PathBuf orders components, not the slash-separated display string.
// In particular, src/causal/provenance.rs precedes src/causal.rs.
export function compareRustPaths(left, right) {
  const a=left.split('/'), b=right.split('/');
  for(let i=0;i<Math.min(a.length,b.length);i++) {
    const order=Buffer.compare(Buffer.from(a[i]),Buffer.from(b[i]));
    if(order) return order;
  }
  return a.length-b.length;
}

export function sourceFiles(roots) {
  const files=[];
  function visit(directory) {
    for(const entry of readdirSync(directory,{withFileTypes:true})) {
      const path=`${directory}/${entry.name}`;
      if(entry.isDirectory()) visit(path);
      else if(entry.isFile()) files.push(path);
      else throw new Error(`Unsupported source entry: ${path}`);
    }
  }
  for(const root of roots) visit(root);
  return files.sort();
}
