import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {exampleSettings,effectiveSettings,settingsReducer,engineConfig,importedSettings,datasetSettings,type ExampleProfile,type SettingsState} from '../src/run-settings.ts';

const manifest=JSON.parse(readFileSync(new URL('../../examples/manifest.json',import.meta.url),'utf8')) as {file:string;profile:ExampleProfile}[];
const profile=(file:string)=>exampleSettings(manifest.find(m=>m.file===file)!.profile);
const initial=():SettingsState=>({defaults:profile('independent/shared-read.graph'),overrides:{},label:'Initial'});
test('every gallery entry supplies a complete profile automatically, including lambda fixtures',()=>{
  let state=initial();
  for(const entry of manifest){
    const defaults=exampleSettings(entry.profile);
    state=settingsReducer(state,{type:'select',defaults,label:entry.file});
    assert.deepEqual(effectiveSettings(state),defaults);
    assert.equal(defaults.execution_profile,'reference-v1');
  }
  assert.equal(profile('behaviour/oscillating.graph').detect_cycles,true);
  assert.equal(profile('lambda/omega.graph').detect_cycles,false);
});
test('individual explicit overrides survive profile changes, including false, blank and equal values',()=>{
  let state=settingsReducer(initial(),{type:'edit',values:{max_steps:'75',seed:'',detect_cycles:false,observer:'value',execution_profile:'indexed-v2'}});
  state=settingsReducer(state,{type:'select',defaults:profile('behaviour/oscillating.graph'),label:'Oscillation'});
  assert.deepEqual(effectiveSettings(state),{...profile('behaviour/oscillating.graph'),max_steps:'75',seed:'',detect_cycles:false,observer:'value',execution_profile:'indexed-v2'});
  state=settingsReducer(state,{type:'select',defaults:profile('lambda/identity.graph'),label:'Lambda'});
  assert.equal(effectiveSettings(state).max_steps,'75');
  assert.equal(effectiveSettings(state).seed,'');
  const reset=settingsReducer(state,{type:'restore'});
  assert.deepEqual(effectiveSettings(reset),profile('lambda/identity.graph'));
  assert.deepEqual(state.overrides.max_steps,'75');
});
test('an untouched observer or recurrence flag follows each newly selected profile',()=>{
  let state=settingsReducer(initial(),{type:'edit',values:{seed:'9'}});
  state=settingsReducer(state,{type:'select',defaults:profile('behaviour/oscillating.graph'),label:'Oscillation'});
  assert.equal(effectiveSettings(state).detect_cycles,true);
  state=settingsReducer(state,{type:'select',defaults:profile('lambda/erasure.graph'),label:'Lambda'});
  assert.equal(effectiveSettings(state).detect_cycles,false);
  assert.equal(effectiveSettings(state).max_steps,'20');
  assert.equal(effectiveSettings(state).seed,'9');
});
test('imported settings remain explicit and round-trip arbitrary observer declarations',()=>{
  const observers={all:'observer-v1\nall\n',value:'observer-v1\nselect data\nlink 76616c7565\n'};
  for(const observer of ['all','value','observer-v1\nselect carrier\nkind Nat\n']){
    const settings={...profile('lambda/identity.graph'),seed:'',observer};
    const config=engineConfig(settings,observers);
    assert.equal(config.seed,null);
    assert.deepEqual(importedSettings(config,observers),settings);
    let state=settingsReducer(initial(),{type:'import',values:importedSettings(config,observers)});
    state=settingsReducer(state,{type:'select',defaults:profile('behaviour/oscillating.graph'),label:'Oscillation'});
    assert.deepEqual(effectiveSettings(state),settings);
  }
  assert.throws(()=>exampleSettings({...manifest[0].profile,observer:'unknown.observer'}),/Unknown example observer/);
});
test('dataset bounds cover processing, activation and the terminal query while respecting overrides',()=>{
  const base=profile('practical/work-items.graph');
  assert.equal(datasetSettings(base,'10','fixed').max_steps,'11');
  assert.equal(datasetSettings(base,'10','guarded_enable').max_steps,'12');
  assert.equal(datasetSettings(base,'0','fixed').max_steps,'1');
  assert.equal(datasetSettings(base,'1000','guarded_enable').max_steps,'1002');
  const state=settingsReducer(settingsReducer(initial(),{type:'edit',values:{max_steps:'2'}}),{type:'select',defaults:datasetSettings(base,'10','fixed'),label:'Dataset'});
  assert.equal(effectiveSettings(state).max_steps,'2');
});
