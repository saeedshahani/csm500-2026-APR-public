// Controlled corruption generator and independent labels. No learner imports.
const names=['jan','feb','mar','apr','may','jun','jul','aug','sep','oct','nov','dec'];
const expanded=['january','february','march','april','may','june','july','august','september','october','november','december'];
export function corrupt(month,mask) {
  let value=mask&32?String(month+1).padStart(2,'0'):mask&16?expanded[month]:names[month];
  if(mask&8)value+='.';
  if(mask&4)value='month='+value;
  if(mask&2)value=value.toUpperCase();
  if(mask&1)value=' '+value+' ';
  return value;
}
export function corpus() {
  const tasks=[];
  for(let rotation=0;rotation<12;rotation++)for(let corruption=0;corruption<64;corruption++) {
    const positive=Array.from({length:12},(_,i)=>{const month=(i+rotation)%12;return {input:corrupt(month,corruption),expected:String(month+1).padStart(2,'0')};});
    tasks.push({id:`r${rotation}-c${corruption}`,rotation,corruption,training:positive.slice(0,6),validation:[...positive.slice(6,9),{input:`invalid-v-${rotation}`,expected:null}],test:[...positive.slice(9),{input:`invalid-t-${rotation}`,expected:null},{input:'13',expected:null},{input:'janitor',expected:null}]});
  }
  return tasks;
}
