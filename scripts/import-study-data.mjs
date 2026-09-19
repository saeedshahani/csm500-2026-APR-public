// Restricted, versioned importers for the two declared S6.J releases.
// A source row is not a rewrite event. No network access or engine execution.
import {readFileSync, writeFileSync, mkdirSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
const hash = b => createHash('sha256').update(b).digest('hex');
const quote = s => JSON.stringify(s);
function checkLimit(limit) {
  if (limit !== Infinity && (!Number.isSafeInteger(limit) || limit < 0)) throw Error('subset limit must be a nonnegative integer');
}
export function structural(text, limit = Infinity) {
  checkLimit(limit);
  if (!/^%%?MatrixMarket matrix coordinate pattern symmetric\s*$/i.test(text.split(/\r?\n/)[0])) throw Error('unsupported matrix format');
  const lines = text.trim().split(/\r?\n/).filter(l => l.trim() && !l.startsWith('%'));
  const [n, m, count] = lines.shift().trim().split(/\s+/).map(Number);
  if (!Number.isSafeInteger(n) || n < 1 || n !== m || count !== lines.length) throw Error('invalid square matrix dimensions/count');
  const seen = new Set(); const edges = []; let duplicates = 0;
  for (const line of lines) {
    const fields = line.trim().split(/\s+/);
    if (fields.length !== 2) throw Error('expected unweighted coordinate pair');
    const [a, b] = fields.map(Number);
    if (![a,b].every(v => Number.isSafeInteger(v) && v >= 1 && v <= n)) throw Error('missing/out-of-range endpoint');
    const pair = [Math.min(a,b), Math.max(a,b)]; const key = pair.join(':');
    if (seen.has(key)) { duplicates++; continue; } seen.add(key); edges.push(pair);
  }
  edges.sort((a,b) => a[0]-b[0] || a[1]-b[1]);
  const selected = edges.slice(0, limit);
  // Preserve declared isolated vertices only for a complete import. Subsets are
  // edge-prefix induced-endpoint projections, not induced subgraphs.
  const vertices = limit >= edges.length ? Array.from({length:n},(_,i)=>i+1) : [...new Set(selected.flat())].sort((a,b)=>a-b);
  const source = ['program {', ...vertices.map(v=>`atom v${v} = Constructor("vertex:${v}");`),
    ...selected.map(([a,b],i)=>`atom e${i} = Record[ObjectLink]("undirected", v${a}, v${b});`), '}'].join('\n')+'\n';
  return {source, counts:{source_vertices:n, source_rows:count, duplicate_relations:duplicates, selected_vertices:vertices.length,
    selected_relations:selected.length, carrier_atoms:vertices.length+selected.length, carrier_fields:2*selected.length, rule_atoms:0},
    recipe:'canonical undirected pairs, deduplicate, numeric lexicographic edge prefix; one record per undirected relation'};
}
const headers = 'Administrative,Administrative_Duration,Informational,Informational_Duration,ProductRelated,ProductRelated_Duration,BounceRates,ExitRates,PageValues,SpecialDay,Month,OperatingSystems,Browser,Region,TrafficType,VisitorType,Weekend,Revenue'.split(',');
const months = new Set(['Jan','Feb','Mar','Apr','May','June','Jul','Aug','Sep','Oct','Nov','Dec']);
export function shoppers(text, limit = Infinity) {
  checkLimit(limit);
  const lines = text.trim().split(/\r?\n/);
  if (lines.shift() !== headers.join(',')) throw Error('unexpected schema/header');
  const rows = lines.map((line, index) => {
    // This release is a simple unquoted CSV dialect. Fail rather than silently
    // misparse another release containing quoted commas/newlines.
    if (line.includes('"')) throw Error('quoted CSV is outside this import profile');
    const cells = line.split(',');
    if (cells.length !== 18 || cells.some(v => !v)) throw Error('missing value/column');
    for (const i of [0,1,2,3,4,5,6,7,8,9,11,12,13,14]) {
      if (!/^\d+(\.\d+)?([Ee][+-]?\d+)?$/.test(cells[i]) || !Number.isFinite(Number(cells[i]))) throw Error('invalid nonnegative numeric field');
    }
    if ([0,2,4,11,12,13,14].some(i => !Number.isSafeInteger(Number(cells[i]))) ||
        [6,7,9].some(i => Number(cells[i]) > 1)) throw Error('invalid count/category code or rate range');
    if (!months.has(cells[10]) || !['Returning_Visitor','New_Visitor','Other'].includes(cells[15]) ||
      ![cells[16],cells[17]].every(v=>['TRUE','FALSE'].includes(v))) throw Error('invalid category');
    return {id:index+1, month:cells[10], visitor:cells[15], revenue:cells[17]};
  });
  const selected = rows.slice(0, limit);
  const source = ['program {', ...selected.flatMap(r => [
    `atom s${r.id} = Constructor("session:${r.id}");`,
    ...['month','visitor','revenue'].flatMap(k => [
      `atom ${k}${r.id} = Constructor(${quote(r[k])});`,
      `atom r${k}${r.id} = Record[ObjectLink]("${k}", s${r.id}, ${k}${r.id});`]),
  ]), '}'].join('\n')+'\n';
  return {source, counts:{source_rows:rows.length, selected_sessions:selected.length, projected_attributes:3,
    carrier_atoms:7*selected.length, carrier_fields:6*selected.length, rule_atoms:0},
    recipe:'source-order row prefix, row number identity, project Month/VisitorType/Revenue; identical rows remain distinct sessions'};
}
export function importArchives(input, output) {
  mkdirSync(output, {recursive:false});
  const manifest = {schema:'study-import-v1', imported_at:new Date().toISOString(), importer_sha256:hash(readFileSync(new URL(import.meta.url))), datasets:[]};
  for (const [name, archive, member, importer, url, licence, attribution] of [
    ['grqc','ca-GrQc.zip','ca-GrQc.mtx',structural,'https://nrvis.com/download/data/ca/ca-GrQc.zip','Network Repository Attribution-ShareAlike policy (version unspecified)', 'J. Leskovec, J. Kleinberg, C. Faloutsos; repository R. A. Rossi and N. K. Ahmed (AAAI 2015)'],
    ['shoppers','shoppers.zip','online_shoppers_intention.csv',shoppers,'https://archive.ics.uci.edu/static/public/468/online%2Bshoppers%2Bpurchasing%2Bintention%2Bdataset.zip','CC-BY-4.0','C. Sakar and Y. Kastro (2018), DOI 10.24432/C5F88Q'],
  ]) {
    const bytes = readFileSync(resolve(input, archive));
    const expected = name === 'grqc' ? '8e27388f036f9c1ba679f8993e5b8ec10abc4b2f7b5810a8eb596529db0d4fdd' : '2972e6184d3ad7beaaa831d9fc2b059dc3ee29df69d1ec593c466a5cd8485d14';
    if (hash(bytes) !== expected) throw Error(`changed upstream release: ${archive}; review terms and mapping before accepting new bytes`);
    const raw = execFileSync('unzip',['-p',resolve(input,archive),member],{maxBuffer:64*1024*1024});
    const data = {name,url,licence,attribution,archive_sha256:hash(bytes),member,member_sha256:hash(raw),subsets:[]};
    for (const limit of [1000,10000,Infinity]) {
      const value = importer(raw.toString('utf8'),limit); const file = `${name}-${Number.isFinite(limit)?limit:'full'}.graph`;
      writeFileSync(resolve(output,file),value.source,{flag:'wx'});
      data.subsets.push({file,sha256:hash(value.source),...value,source:undefined});
    }
    manifest.datasets.push(data);
  }
  writeFileSync(resolve(output,'manifest.json'),JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
  return manifest;
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if(process.argv.length!==4)throw Error('node scripts/import-study-data.mjs ARCHIVE_DIRECTORY NEW_OUTPUT_DIRECTORY');
  console.log(JSON.stringify(importArchives(process.argv[2],process.argv[3]),null,2));
}
