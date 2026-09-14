export type Atom = {kind: string | {Constructor?: string; Record?:{role:string;label:string|null}; PatternVariable?:{admits:unknown[];binding:string}};fields:string[]};
export type EventRecord = {id:string;redex:{state:string;source:string;revision:string;binding:[string,string][]};post_state:string;footprint:Record<string,string[]>;fresh:Record<string,Atom>};
export type EngineState = {
  graph:{next_identity:string;atoms:Record<string,Atom>};
  rules:{head:string;revision:string;rank:string;active:boolean;meta:boolean;lhs?:string[];rhs?:string[];preserved?:string[];representation?:string[]}[];quoted:Record<string,string>;
  events:EventRecord[];dependencies:{before:string;after:string;witnesses:{kind:string;atom:string}[]}[];
  relation:string;limitation:string;position:string;endpoint:string;status:string;can_advance:boolean;
  report:null|{outcome:string;payload:string|null;scheduler:string;recurrence:null|{first:string;repeated:string}};
  previous?:{next_identity:string;atoms:Record<string,Atom>}|null;
  names?:Record<string,string>;
  observer?:string;
  settings?:{plan?:{max_steps:string;seed:string|null;detect_cycles:boolean}|null;search?:{max_work:string;max_candidates:string};[key:string]:unknown};
};
export type Analysis = {state:string;ancestors:string[];descendants:string[];local:null|{alternatives:EventRecord['redex'][];pairs:{first:string;second:string;second_after_first:string|null;first_after_second:string|null;commutes:boolean|null;corresponding_events:boolean|null}[]}};
export type Reply = {ok:boolean;error:null|{kind:string;message:string;line:string|null;column:string|null};state:EngineState|null;trace:string|null;analysis?:Analysis|null;dataset?:import('./Datasets').Preview|null;inspection?:{total:string;atoms:[string,Atom][]}|null};
export type Command = {command:string;[key:string]:unknown};
export type Request = {generation:number;id:number;payload:Command};
export type Response = {generation:number;id:number;reply:Reply};
