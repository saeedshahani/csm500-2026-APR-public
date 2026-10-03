// Conventional oracle built by list rotation, independently of the policy model.
export const months=['01','02','03','04','05','06','07','08','09','10','11','12'];
export const contexts=['calendar','fiscal','noncalendar'];
export function meaningTasks() {
  const tasks=[];
  for(let rotation=0;rotation<12;rotation++)for(let start=0;start<12;start++) {
    const sequence=[...months.slice(rotation),...months.slice(0,rotation)],fiscal=[...months.slice(start),...months.slice(0,start)];
    const oracle=token=>({calendar:token,fiscal:fiscal[months.indexOf(token)],noncalendar:null});
    const expand=tokens=>tokens.flatMap(token=>contexts.map(context=>({token,context,expected:oracle(token)[context]})));
    tasks.push({id:`r${rotation}-s${start}`,rotation,start,training:expand(sequence.slice(0,6)),validation:expand(sequence.slice(6,9)),test:expand(sequence.slice(9))});
  }
  return tasks;
}
