// Source collection must not depend on optional developer search tools.
import {readdirSync} from 'node:fs';

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
