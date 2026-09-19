// Reproducible artefact identity; manuscript and operational planning are excluded.
import {execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
const candidates=execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n');
const files=[...new Set(candidates)].filter(p=>
 /^(Cargo\.(toml|lock)|rust-toolchain\.toml)$/.test(p)||
 /^crates\/.*\.(rs|toml)$/.test(p)||/proptest-regressions\/.*\.txt$/.test(p)||
 /^examples\/.*\.(atoms|graph|observer|json|lambda|md)$/.test(p)||
 /^\.github\/workflows\/.*\.ya?ml$/.test(p)||
 /^(frontend|platform|scripts|benchmarks)\//.test(p)||
 /^docs\/(architecture|lambda-encoding|performance-study|trace-format|workbench|workbench-guide)\.md$/.test(p)
).sort();
const hash=createHash('sha256');
for(const p of files)hash.update(p).update('\0').update(readFileSync(p)).update('\0');
console.log(JSON.stringify({sha256:hash.digest('hex'),count:files.length,files},null,2));
