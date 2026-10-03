// Structural section identity only: this is not a WASM semantic equivalence test.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
export function sections(bytes) {
  assert.equal(bytes.subarray(0,8).toString('hex'),'0061736d01000000');let at=8;
  const leb=()=>{let n=0,shift=0,b;do{assert.ok(at<bytes.length&&shift<=28);b=bytes[at++];n+=(b&127)*2**shift;shift+=7;}while(b&128);return n;};
  const result=[];while(at<bytes.length){const id=bytes[at++],length=leb(),start=at,end=at+length;assert.ok(end<=bytes.length);let name=null;
    if(id===0){const size=leb();assert.ok(at+size<=end);name=bytes.subarray(at,at+size).toString();}
    result.push({id,name,length,sha256:createHash('sha256').update(bytes.subarray(start,end)).digest('hex')});at=end;
  }return result;
}
