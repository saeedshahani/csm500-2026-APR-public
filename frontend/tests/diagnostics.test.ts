import {test} from 'node:test';
import assert from 'node:assert/strict';
import {locateDiagnostics} from '../src/diagnostics.ts';
import type {SourceIssue} from '../src/types.ts';
const issue=(start:number,end:number):SourceIssue=>({kind:'MalformedInput',message:'example',span:{start:String(start),end:String(end)},related:[]});
test('UTF-8 spans become exact UTF-16 ranges across Unicode, tabs and mixed newlines',()=>{
 const source='λ🐈\r\n\t猫\rwrong\nend';
 const start=Buffer.byteLength(source.slice(0,source.indexOf('wrong')));
 const [range]=locateDiagnostics(source,[issue(start,start+5)]);
 assert.deepEqual({from:range.from,to:range.to,line:range.line,column:range.column},{from:7,to:12,line:3,column:1});
 assert.equal(source.replace(/\r\n?|\n/g,'\n').slice(range.from,range.to),'wrong');
 const [cat]=locateDiagnostics(source,[issue(2,6)]);assert.equal(cat.to-cat.from,2);assert.equal(cat.column,2);
});
test('all primary and related locations retain their own ranges, including EOF and multiline spans',()=>{
 const first=issue(0,4);first.related=[{message:'earlier declaration',span:{start:'5',end:'9'}}];
 const locations=locateDiagnostics('name\nname',[first,issue(9,9),issue(0,9)]);
 assert.deepEqual(locations.map(e=>[e.key,e.from,e.to,e.related]),[['0',0,4,false],['0:0',5,9,true],['1',9,9,false],['2',0,9,false]]);
 assert.equal(locations[2].line,2);assert.equal(locations[2].column,5);
});
test('invalid or non-character boundaries are not presented as precise source locations',()=>{
 assert.deepEqual(locateDiagnostics('🐈',[issue(1,4),issue(0,5),issue(4,0),issue(-1,0),issue(Number.MAX_SAFE_INTEGER+1,Number.MAX_SAFE_INTEGER+1)]),[]);
 assert.deepEqual(locateDiagnostics('unlocated error'),[]);
 assert.deepEqual(locateDiagnostics('x',[{...issue(0,1),span:{start:'',end:'1'}}]),[]);
});
