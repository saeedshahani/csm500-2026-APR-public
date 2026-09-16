import type {SourceIssue} from './types';
export type LocatedIssue = {key:string;message:string;related:boolean;from:number;to:number;line:number;column:number};

// Parser offsets are UTF-8 bytes. CodeMirror uses UTF-16 with normalised newlines.
// Collect only requested boundaries, then scan once; do not allocate an entry per byte.
export function locateDiagnostics(source:string,issues:SourceIssue[]=[]):LocatedIssue[]{
 const pending=issues.flatMap((e,i)=>[
  {key:String(i),message:e.message,related:false,span:e.span},
  ...e.related.map((r,j)=>({key:`${i}:${j}`,message:r.message,related:true,span:r.span}))
 ]).map(e=>({...e,start:Number(e.span.start),end:Number(e.span.end)}))
  .filter(e=>/^\d+$/.test(e.span.start)&&/^\d+$/.test(e.span.end)&&Number.isSafeInteger(e.start)&&Number.isSafeInteger(e.end)&&e.start>=0&&e.start<=e.end);
 const wanted=new Set(pending.flatMap(e=>[e.start,e.end]));
 if(!wanted.size)return [];
 const boundaries=new Map<number,{offset:number;line:number;column:number}>();
 let bytes=0,offset=0,line=1,column=1,cr=false;
 const remember=()=>{if(wanted.has(bytes))boundaries.set(bytes,{offset,line,column});};remember();
 for(const ch of source){
  const code=ch.codePointAt(0)!;bytes+=code<=0x7f?1:code<=0x7ff?2:code<=0xffff?3:4;
  if(ch==='\r'){offset++;line++;column=1;}
  else if(ch==='\n'){if(!cr){offset++;line++;column=1;}}
  else{offset+=ch.length;column++;}
  cr=ch==='\r';remember();
 }
 return pending.flatMap(e=>{const a=boundaries.get(e.start),b=boundaries.get(e.end);return a&&b?[{key:e.key,message:e.message,related:e.related,from:a.offset,to:b.offset,line:a.line,column:a.column}]:[];});
}
