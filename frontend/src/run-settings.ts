import type {PendingConfig} from './projects.ts';

export type RunSettings={max_steps:string;max_work:string;max_candidates:string;seed:string;detect_cycles:boolean;observer:string;execution_profile:NonNullable<PendingConfig['execution_profile']>};
export type ExampleProfile={max_steps:string;max_work:string;max_candidates:string;seed:string|null;detect_cycles:boolean;observer:string;execution_profile?:RunSettings['execution_profile']};
export type SettingsState={defaults:RunSettings;overrides:Partial<RunSettings>;label:string};
export type SettingsAction=
  |{type:'select';defaults:RunSettings;label:string}
  |{type:'edit';values:Partial<RunSettings>}
  |{type:'restore'}
  |{type:'import';values:RunSettings};

/** Every field is supplied by the selected profile unless explicitly overridden. */
export function settingsReducer(state:SettingsState,action:SettingsAction):SettingsState{
  switch(action.type){
    case 'select':return {...state,defaults:action.defaults,label:action.label};
    case 'edit':return {...state,overrides:{...state.overrides,...action.values}};
    case 'restore':return {...state,overrides:{}};
    case 'import':return {...state,overrides:{...action.values}};
  }
}
export function effectiveSettings(state:SettingsState):RunSettings{return {...state.defaults,...state.overrides};}
export function exampleSettings(profile:ExampleProfile):RunSettings{
  if(!['observers/all.observer','observers/value.observer'].includes(profile.observer))throw Error(`Unknown example observer: ${profile.observer}`);
  return {...profile,seed:profile.seed??'',observer:profile.observer==='observers/value.observer'?'value':'all',execution_profile:profile.execution_profile??'reference-v1'};
}
export function engineConfig(settings:RunSettings,observers:{all:string;value:string}):PendingConfig{
  return {...settings,seed:settings.seed.trim()?settings.seed:null,observer:settings.observer==='all'?observers.all:settings.observer==='value'?observers.value:settings.observer};
}
export function importedSettings(config:PendingConfig,observers:{all:string;value:string}):RunSettings{
  return {...config,seed:config.seed??'',execution_profile:config.execution_profile??'reference-v1',observer:config.observer===observers.all?'all':config.observer===observers.value?'value':config.observer};
}
export function datasetSettings(base:RunSettings,pending:string,policy:string):RunSettings{
  // Include a final applicability query after the expected commits.
  return {...base,max_steps:String(Number(pending)+(policy==='guarded_enable'?1:0)+1)};
}
