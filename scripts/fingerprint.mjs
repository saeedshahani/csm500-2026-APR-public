// Reproducible artefact identity; manuscript and operational planning are excluded.
import {execFileSync} from 'node:child_process';
import {readFileSync,existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
const candidates=execFileSync('git',['ls-files','-co','--exclude-standard'],{encoding:'utf8'}).trim().split('\n');
const files=[...new Set(candidates)].filter(p=>existsSync(p)&&(
 /^(Cargo\.(toml|lock)|rust-toolchain\.toml)$/.test(p)||
 /^crates\/.*\.(rs|toml)$/.test(p)||/proptest-regressions\/.*\.txt$/.test(p)||
 /^examples\/.*\.(atoms|graph|observer|json|lambda|md)$/.test(p)||
 /^\.github\/workflows\/.*\.ya?ml$/.test(p)||
 /^(frontend|platform|scripts|benchmarks)\//.test(p)||
 /^docs\/architecture\.md$/.test(p)||
 /^docs\/reference\/(lambda-encoding|trace-format|workbench)\.md$/.test(p)||
 /^docs\/research\/evaluation\.md$/.test(p)||
 /^docs\/guides\/workbench\.md$/.test(p)
)).sort();
const hash=createHash('sha256');
for(const p of files)hash.update(p).update('\0').update(readFileSync(p)).update('\0');
console.log(JSON.stringify({sha256:hash.digest('hex'),count:files.length,files},null,2));
