import {test,expect} from '@playwright/test';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import path from 'node:path';
const root=path.resolve('..');
test('production WASM: load, step, inspect, export, import and replay',async({page})=>{
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));await page.goto('/');await expect(page.getByTestId('status')).toHaveText('No program loaded');
  await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');
  await expect(page.locator('svg .atom')).toHaveCount(20);await page.getByRole('button',{name:'Step',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Yielded');
  await page.getByLabel('Select atom',{exact:true}).selectOption('0');await expect(page.locator('.inspector')).toContainText('revision');
  const download=page.waitForEvent('download');await page.getByRole('button',{name:'Export trace'}).click();const saved=await download;const file=await saved.path();expect(file).not.toBeNull();
  const trace=JSON.parse(readFileSync(file!,'utf8'));expect(trace.events).toHaveLength(1);expect(trace.settings.plan.seed).toBe('42');expect(trace.relation).toBe('positive_resource_order');
  const check=execFileSync('cargo',['run','--quiet','--locked','-p','cli','--','trace-check',file!],{cwd:root,encoding:'utf8'});expect(check).toContain('Verified 1 events');
  await page.getByRole('button',{name:'Reset',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');
  await page.getByLabel('Import trace',{exact:true}).setInputFiles(file!);await expect(page.getByTestId('status')).toHaveText('Paused');
  await page.getByLabel('Inspect state',{exact:true}).selectOption('0');await expect(page.getByRole('button',{name:'Step',exact:true})).toBeDisabled();await page.getByLabel('Inspect state',{exact:true}).selectOption('1');
  await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('NormalForm');await expect(page.getByLabel('Recorded history')).toContainText('No direct resource edges');expect(errors).toEqual([]);
  await page.screenshot({path:'../output/playwright/workbench.png',fullPage:true});
});
test('failed loads retain source and graph; malicious labels render as text; empty reset',async({page})=>{
  await page.goto('/');await expect(page.getByTestId('status')).toHaveText('No program loaded');await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');
  const editor=page.getByRole('textbox',{name:'Program source'});await editor.fill('program { broken }');await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByRole('alert')).toContainText('line');await expect(page.locator('svg .atom')).toHaveCount(20);await expect(editor).toContainText('broken');
  await editor.fill('program { atom a = Constructor("<img src=x onerror=alert(1)>"); }');await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');await expect(page.locator('img')).toHaveCount(0);await page.getByLabel('Select atom',{exact:true}).selectOption('0');await expect(page.locator('.inspector')).toContainText('<img src=x');
  await editor.fill('program {}');await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.locator('.empty-graph')).toBeVisible();await page.getByRole('button',{name:'Step',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('NormalForm');for(let i=0;i<3;i++){await page.getByRole('button',{name:'Reset',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');}
});
test('CLI trace enters browser with metadata preserved; corrupt import is atomic',async({page})=>{
  const bytes=execFileSync('cargo',['run','--quiet','--locked','-p','cli','--','export','examples/behaviour/stable.graph','--max-steps','1'],{cwd:root});
  await page.goto('/');await expect(page.getByTestId('status')).toHaveText('No program loaded');await page.getByLabel('Import trace',{exact:true}).setInputFiles({name:'cli.json',mimeType:'application/json',buffer:bytes});await expect(page.getByTestId('status')).toHaveText('NormalForm');
  await page.getByLabel('Import trace',{exact:true}).setInputFiles({name:'bad.json',mimeType:'application/json',buffer:Buffer.from('{broken')});await expect(page.getByRole('alert')).toBeVisible();await expect(page.getByTestId('status')).toHaveText('NormalForm');
  const event=page.waitForEvent('download');await page.getByRole('button',{name:'Export trace'}).click();const artifact=JSON.parse(readFileSync((await (await event).path())!,'utf8'));expect(artifact).toEqual(JSON.parse(bytes.toString()));
});
test('costly query can be stopped; recovery and subsequent replacement stay usable',async({page})=>{
  await page.goto('/');await expect(page.getByTestId('status')).toHaveText('No program loaded');
  const vars=Array.from({length:18},(_,i)=>`atom x${i} = PatternVariable[Constructor] Free;`).join('\n');const sides=Array.from({length:18},(_,i)=>`l${i}: x${i};`).join(' ');const rhs=Array.from({length:18},(_,i)=>`r${i}: x${i};`).join(' ');const data=Array.from({length:20},(_,i)=>`atom a${i} = Constructor("A");`).join('\n');
  await page.getByRole('textbox',{name:'Program source'}).fill(`program {rule costly {atom z = Nat();rank rank0=z;mode mode0=ModeObject;revision v;active act;${vars} lhs {${sides}} rhs {${rhs}}} ${data}}`);
  await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Working');
  await page.getByRole('button',{name:'Stop',exact:true}).click();await expect(page.getByRole('alert')).toContainText('unfinished batch was discarded');await expect(page.getByTestId('status')).toHaveText('Paused');await expect(page.getByLabel('Inspect state',{exact:true})).toHaveValue('0');
  await page.getByLabel('Example',{exact:true}).selectOption({label:'behaviour/stable'});await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('NormalForm');
});
for(const observer of ['all','value'])test(`browser batch sizes match uninterrupted native recurrence: ${observer}`,async({page})=>{
  let native:Buffer;try{native=execFileSync('cargo',['run','--quiet','--locked','-p','cli','--','export','examples/behaviour/oscillating.graph','--max-steps','6','--seed','42','--strategy','seeded','--detect-cycles','true','--observer',`examples/observers/${observer}.observer`],{cwd:root});}catch(e){native=(e as {stdout:Buffer}).stdout;}
  const expected=JSON.parse(native.toString());
  await page.goto('/');await expect(page.getByTestId('status')).toHaveText('No program loaded');await page.getByLabel('Example',{exact:true}).selectOption({label:'behaviour/oscillating'});await page.locator('summary').click();await page.getByLabel('Absolute step bound').fill('6');await page.getByLabel('Detect recurrence').check();await page.getByLabel('Observer',{exact:true}).selectOption(observer);
  for(const batch of ['1','2','5']){await page.getByLabel('Commits per batch').selectOption(batch);await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('CycleDetected');const download=page.waitForEvent('download');await page.getByRole('button',{name:'Export trace'}).click();const trace=JSON.parse(readFileSync((await (await download).path())!,'utf8'));expect(trace).toEqual(expected);}
});
