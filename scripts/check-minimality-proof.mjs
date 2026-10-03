// Explicit tool path, or `lean` on PATH. Pinned proof checker; no download/install.
import {execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const lean=process.argv[2]??'lean', file='crates/engine/proofs/MinimalityBounds.lean';
const version=execFileSync(lean,['--version'],{encoding:'utf8'}).trim();
assert.match(version,/version 4\.19\.0(?:\s|,)/);
const source=readFileSync(file), text=source.toString();
assert.doesNotMatch(text,/\b(sorry|admit|axiom)\b/);
const output=execFileSync(lean,[file],{encoding:'utf8'});
for(const name of ['capacity','unary_boundary','binary_boundary','minimum_three_arcs','one_root_symmetry'])assert.match(output,new RegExp(`'MinimalityBounds\\.${name}' depends on axioms: \\[propext(?:, Quot.sound)?\\]`));
assert.doesNotMatch(output,/sorryAx|error:|warning:/);
console.log(JSON.stringify({version,source_sha256:createHash('sha256').update(source).digest('hex'),output},null,2));
