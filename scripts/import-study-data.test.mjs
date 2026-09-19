import test from 'node:test';
import assert from 'node:assert/strict';
import {structural, shoppers} from './import-study-data.mjs';
const matrix = text => '%MatrixMarket matrix coordinate pattern symmetric\n' + text;
test('undirected input preserves loops and isolated vertices, deduplicates reversals',()=>{
  const result=structural(matrix('3 3 3\n1 2\n2 1\n2 2\n'));
  assert.equal(result.counts.duplicate_relations,1); assert.equal(result.counts.carrier_atoms,5);
  assert.equal(result.counts.carrier_fields,4); assert.match(result.source,/v3/);
  assert.equal(structural(matrix('3 3 2\n2 2\n1 2\n'),1).counts.selected_vertices,2);
});
test('invalid endpoints, dimension/count and weights fail closed',()=>{
  for(const source of ['2 3 0','2 2 1\n1 3','2 2 0\n1 2','2 2 1\n1 2 7'])assert.throws(()=>structural(matrix(source)));
  assert.throws(()=>structural('2 2 1\n1 2'));
  assert.throws(()=>structural(matrix('2 2 0').replace('symmetric','general')));
  for(const limit of [-1,0.5,NaN])assert.throws(()=>structural(matrix('2 2 0'),limit));
});
const header='Administrative,Administrative_Duration,Informational,Informational_Duration,ProductRelated,ProductRelated_Duration,BounceRates,ExitRates,PageValues,SpecialDay,Month,OperatingSystems,Browser,Region,TrafficType,VisitorType,Weekend,Revenue';
const row='0,0,0,0,1,0,0.2,0.2,0,0,Feb,1,1,1,1,Returning_Visitor,FALSE,FALSE';
test('session duplicates remain separate observations; deterministic projection counts',()=>{
  const result=shoppers(`${header}\r\n${row}\r\n${row}\r\n`);
  assert.equal(result.counts.carrier_atoms,14); assert.equal(result.counts.carrier_fields,12);
  assert.match(result.source,/session:2/); assert.equal(shoppers(`${header}\n${row}`,0).counts.selected_sessions,0);
});
test('missing values, categories, quotes, numeric fields and schema fail closed',()=>{
  for(const input of [row.replace('Feb',''),row.replace('Feb','MISSING'),row.replace('0.2','-1'),row.replace('0.2','1.2'),row.replace(/^0,/, '0.5,'),row.replace('FALSE','null'),`"${row}"`]) assert.throws(()=>shoppers(`${header}\n${input}`));
  for(const limit of [-1,0.5,NaN])assert.throws(()=>shoppers(`${header}\n${row}`,limit));
  assert.throws(()=>shoppers(`wrong\n${row}`));
});
