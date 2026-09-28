// Public-data adapter and result interpreter. The native engine performs the join.
import { shoppers } from '../scripts/import-study-data.mjs';
export const MONTH_HEADER = 'Administrative,Administrative_Duration,Informational,Informational_Duration,ProductRelated,ProductRelated_Duration,BounceRates,ExitRates,PageValues,SpecialDay,Month,OperatingSystems,Browser,Region,TrafficType,VisitorType,Weekend,Revenue';
const names = ['Jan','Feb','Mar','Apr','May','June','Jul','Aug','Sep','Oct','Nov','Dec'];
const q = JSON.stringify;
export function compileMonths(csv, policy = 'fixed') {
  if (typeof csv !== 'string' || Buffer.byteLength(csv) > 4*1024*1024) throw Error('CSV must be at most 4 MiB');
  if (!['fixed','guarded','disabled'].includes(policy)) throw Error('Unknown month policy');
  shoppers(csv, 0); // Strict release dialect, numeric/category ranges and all 18 fields.
  const rows = csv.trim().split(/\r?\n/).slice(1).map(line=>line.split(','));
  if (!rows.length || rows.length > 15000) throw Error('Expected 1–15000 complete session rows');
  let source = `program {
rule normalise {
 atom zero = Nat(); rank rk = zero; mode md = ModeObject; revision rev;
 ${policy === 'fixed' ? 'active enabled;' : ''}
 atom row = PatternVariable[Constructor] Free;
 atom raw = PatternVariable[Constructor] Free;
 atom canonical = PatternVariable[Constructor] Free;
 atom input = Record[ObjectLink]("source-month", row, raw);
 atom mapping = Record[ObjectLink]("normalizes-to", raw, canonical);
 atom output = Record[ObjectLink]("calendar-month", row, canonical);
 lhs { lr: row; lv: raw; lc: canonical; li: input; lm: mapping; }
 rhs { rr: row; rv: raw; rc: canonical; ro: output; rm: mapping; }
}
`;
  if (policy === 'guarded') source += `rule approve {
 atom z = Nat(); atom one = Nat(z); rank approve_rank = one;
 mode approve_mode = ModeMeta; revision approve_rev; active approve_active;
 atom self = PatternVariable[RuleHead] AppliedRule;
 atom own_z = Nat(); atom own_one = Nat(own_z);
 atom own_rank = Record[Rank](self, own_one);
 atom lower_rank = Record[Rank](self, own_z);
 atom own_active = Record[Active](self);
 atom own_rev = Record[Revision](self); atom next_rev = Record[Revision](self);
 atom target = PatternVariable[RuleHead] Free;
 atom target_z = Nat(); atom target_rank = Record[Rank](target, target_z);
 atom target_mode = Record[ModeObject](target);
 atom target_rev = Record[Revision](target); atom approved_rev = Record[Revision](target);
 atom approved = Record[Active](target);
 lhs { ls: self; lz: own_z; lo: own_one; alr: own_rank; la: own_active; alv: own_rev;
       lt: target; ltz: target_z; ltr: target_rank; ltm: target_mode; ltv: target_rev; }
 rhs { rs: self; rz: own_z; aro: own_one; arr: lower_rank; arv: next_rev;
       rt: target; rtz: target_z; rtr: target_rank; rtm: target_mode; rtv: approved_rev; ra: approved; }
}
`;
  names.forEach((name,i)=>{source += `atom raw_${i} = Constructor(${q('Month:'+name)});\natom canonical_${i} = Constructor(${q('MonthNumber:'+String(i+1).padStart(2,'0'))});\natom map_${i} = Record[ObjectLink]("normalizes-to", raw_${i}, canonical_${i});\n`;});
  rows.forEach((cells,i)=>{source += `atom row_${i} = Constructor(${q('Session:'+q([i+1,...cells]))});\natom month_${i} = Record[ObjectLink]("source-month", row_${i}, raw_${names.indexOf(cells[10])});\n`;});
  return {source:source+'}\n',rows:rows.length,policy};
}

export function monthResults(trace, originalCsv) {
  const atoms = new Map(Object.entries(trace.initial.atoms));
  for(const e of trace.events) { for(const id of e.footprint.consumed)atoms.delete(id);for(const [id,a] of Object.entries(e.fresh))atoms.set(id,a); }
  const rows=new Map();
  for(const [id,a] of atoms)if(a.kind.Constructor?.startsWith('Session:')) {
    const [row,...fields]=JSON.parse(a.kind.Constructor.slice(8));
    rows.set(id,{id:row,fields,source_month:fields[10],month_number:null,status:'unprocessed',links:0});
  }
  for(const a of atoms.values())if(a.kind.Record?.role==='ObjectLink'&&['source-month','calendar-month'].includes(a.kind.Record.label)&&rows.has(a.fields[0])) {
    const row=rows.get(a.fields[0]);row.links++;
    if(a.kind.Record.label==='calendar-month') { row.month_number=atoms.get(a.fields[1])?.kind.Constructor?.replace(/^MonthNumber:/,'')??null;row.status='normalised'; }
  }
  const values=[...rows.values()].sort((a,b)=>a.id-b.id);
  // Independent explicit table, not the compiler's ordered names or graph dictionary.
  const oracle={Jan:'01',Feb:'02',Mar:'03',Apr:'04',May:'05',June:'06',Jul:'07',Aug:'08',Sep:'09',Oct:'10',Nov:'11',Dec:'12'};
  const expected=originalCsv===undefined?new Map(Object.values(trace.initial.atoms).filter(a=>a.kind.Constructor?.startsWith('Session:')).map(a=>{const [id,...fields]=JSON.parse(a.kind.Constructor.slice(8));return [id,fields];})):new Map(originalCsv.trim().split(/\r?\n/).slice(1).map((line,i)=>[i+1,line.split(',')]));
  const errors=values.filter(r=>r.links!==1||r.status!=='normalised'||r.month_number!==oracle[r.source_month]||q(r.fields)!==q(expected.get(r.id))).map(r=>r.id);
  const complete=trace.report?.outcome==='NormalForm'&&values.length===expected.size&&new Set(values.map(r=>r.id)).size===expected.size&&errors.length===0;
  return {schema:'shopping-month-result-v1',complete,total:values.length,normalised:values.filter(r=>r.status==='normalised').length,invalid_total:errors.length,invalid_rows:errors.slice(0,100),rows:values};
}
export function monthCsv(result) {
  if(!result.complete)throw Error('No complete oracle-accepted month result');
  return MONTH_HEADER+',MonthNumber\n'+result.rows.map(r=>[...r.fields,r.month_number].join(',')).join('\n')+'\n';
}
