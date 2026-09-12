export type Atom = {kind: string | {Constructor?: string; Record?:{role:string;label:string|null}; PatternVariable?:{admits:unknown[];binding:string}};fields:string[]};
export type EventRecord = {id:string;redex:{state:string;source:string;revision:string;binding:[string,string][]};post_state:string;footprint:Record<string,string[]>;fresh:Record<string,Atom>};
export type EngineState = {
  graph:{next_identity:string;atoms:Record<string,Atom>};
  rules:{head:string;revision:string;rank:string;active:boolean;meta:boolean}[];quoted:Record<string,string>;
  events:EventRecord[];dependencies:{before:string;after:string;witnesses:{kind:string;atom:string}[]}[];
  relation:string;limitation:string;position:string;endpoint:string;status:string;can_advance:boolean;
  report:null|{outcome:string;payload:string|null;scheduler:string;recurrence:null|{first:string;repeated:string}};
};
export type Reply = {ok:boolean;error:null|{kind:string;message:string;line:string|null;column:string|null};state:EngineState|null;trace:string|null};
export type Command = {command:string;[key:string]:unknown};
export type Request = {generation:number;id:number;payload:Command};
export type Response = {generation:number;id:number;reply:Reply};
