import {test,expect,type Page} from '@playwright/test';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import path from 'node:path';
const root=path.resolve('..');
async function open(page:Page){await page.goto('/');await expect(page.getByTestId('status')).toHaveText('No program loaded');}
async function load(page:Page){await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');}
async function exported(page:Page,name='Export trace'){const wait=page.waitForEvent('download');await page.getByRole('button',{name,exact:true}).click();return readFileSync((await(await wait).path())!,'utf8');}
function cli(args:string[],input?:string){return execFileSync('cargo',['run','--quiet','--locked','-p','cli','--',...args],{cwd:root,input,encoding:'utf8',maxBuffer:64*1024*1024});}

test('precise Unicode errors, related declarations and EOF navigation preserve the loaded execution',async({page})=>{
 await open(page);await load(page);const trace=await exported(page),editor=page.getByRole('textbox',{name:'Program source'});
 await editor.fill('program {\n atom a = Constructor("λ 🐈"); atom a = Constructor("猫");\n}');
 await page.getByRole('button',{name:'Validate draft'}).click();
 await expect(page.locator('.source-error')).toHaveText('a');await expect(page.locator('.source-related')).toHaveText('a');
 await page.getByLabel('Source diagnostics').getByRole('button').first().click();await expect(editor).toBeFocused();expect(await page.evaluate(()=>getSelection()?.toString())).toBe('a');
 await page.getByRole('button',{name:/Go to line.*related/}).click();expect(await page.evaluate(()=>getSelection()?.toString())).toBe('a');
 expect(await exported(page)).toBe(trace);await page.screenshot({path:'../output/playwright/s5-source-ranges.png',fullPage:true});
 await editor.fill('program { atom x = Nat(missing); atom y = Nat(other); }');await page.getByRole('button',{name:'Load program',exact:true}).click();
 await expect(page.locator('.source-error')).toHaveText(['missing','other']);expect(await exported(page)).toBe(trace);
 await page.setViewportSize({width:390,height:844});await page.getByLabel('Theme').selectOption('dark');expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
 await page.screenshot({path:'../output/playwright/s5-errors-dark-narrow.png',fullPage:true});
 await editor.fill('program { atom x = Constructor("🐈");');await page.getByRole('button',{name:'Validate draft'}).click();
 await expect(page.getByRole('img',{name:'Diagnostic at insertion point'})).toBeVisible();await page.getByLabel('Source diagnostics').getByRole('button').first().click();await page.keyboard.type(' }');
 await page.getByRole('button',{name:'Validate draft'}).click();await expect(page.getByLabel('Source diagnostics')).toHaveCount(0);expect(await exported(page)).toBe(trace);
 await page.getByRole('button',{name:'Reset',exact:true}).click();expect(await exported(page)).toBe(trace);await expect(editor).toContainText('🐈');
 await load(page);await expect(page.locator('.status-strip')).toContainText('1 atoms');
});

test('lambda history integrates rule sides, complete carrier, causal witnesses and CLI round trips',async({page})=>{
 await open(page);await page.getByLabel('Example',{exact:true}).selectOption({label:'lambda/repetition'});await page.getByRole('button',{name:'Apply example settings'}).click();await load(page);
 await page.getByRole('button',{name:'Rules',exact:true}).click();await expect(page.getByLabel('Rule definitions').locator('article')).toHaveCount(11);
 for(const label of ['L: matches','R: produces/preserves','K: preserved'])await expect(page.getByLabel('Rule definitions').getByText(label,{exact:true})).toHaveCount(11);
 await page.getByRole('button',{name:'Check applicability and local conflicts'}).click();await expect(page.getByLabel('Rule definitions')).toContainText('Applicable');
 await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('NormalForm');await expect(page.getByTestId('lambda-term')).toHaveText('(λ.0)');
 const bytes=await exported(page),trace=JSON.parse(bytes);expect(trace.events).toHaveLength(7);
 expect(JSON.parse(cli(['export','examples/lambda/repetition.graph','--max-steps','20','--strategy','seeded','--seed','42']))).toEqual(trace);
 expect(cli(['trace-check','-'],bytes)).toContain('Verified 7 events');
 await page.getByRole('button',{name:'Internal',exact:true}).click();await expect(page.locator('svg .atom')).toHaveCount(250);await expect(page.locator('.view-context').first()).toContainText('Showing 250 of');
 const atom=trace.events[6].footprint.consumed[0];await page.locator('.change-summary').getByRole('button',{name:new RegExp(`#${atom}$`)}).first().click();await expect(page.locator('.inspector')).toContainText('removed from state 7');
 await page.getByRole('button',{name:'History',exact:true}).click();await page.getByRole('button',{name:'Inspect event 2',exact:true}).click();await expect(page.getByLabel('Inspect state',{exact:true})).toHaveValue('3');
 await expect(page.locator('.change-summary')).toContainText(`exact revision #${trace.events[2].redex.revision}`);
 await page.getByRole('button',{name:'Inspect ancestors / descendants'}).click();await expect(page.getByLabel('History analysis')).toContainText('Ancestors:');await expect(page.getByLabel('History analysis')).toContainText('Descendants:');
 const witness=page.locator('.edge-list').getByRole('button',{name:/atom #/}).first();await expect(witness).toBeVisible();await witness.click();await expect(page.locator('.inspector')).toContainText('Kind / label');
 await page.getByRole('button',{name:'First',exact:true}).click();await page.getByRole('button',{name:'Analyse alternatives at state 0',exact:true}).click();await expect(page.getByLabel('History analysis')).toContainText('Hypothetical alternatives (not executed events)');await expect(page.getByLabel('History analysis')).toContainText('1 fully enabled occurrences; 0 checked pairs');
 expect(await exported(page)).toBe(bytes);await page.getByLabel('Import trace',{exact:true}).setInputFiles({name:'lambda.trace.json',mimeType:'application/json',buffer:Buffer.from(bytes)});await expect(page.getByTestId('lambda-term')).toHaveText('(λ.0)');
 await page.screenshot({path:'../output/playwright/s5-lambda-history.png',fullPage:true});
});

test('measured hundred-item workload completes in the browser and retains all history and identities',async({page})=>{
 test.setTimeout(240_000);await open(page);await page.locator('summary').filter({hasText:/^Run settings/}).click();await page.getByLabel('Absolute step bound').fill('101');await page.getByLabel('Seed',{exact:true}).fill('');await page.getByLabel('Commits per batch').selectOption('10');
 await page.getByText('Work-item datasets',{exact:true}).click();await page.getByLabel('Dataset size').selectOption('100');await page.getByRole('button',{name:'Preview dataset',exact:true}).click();await expect(page.locator('.dataset-preview')).toContainText('100 items');await page.getByRole('button',{name:'Load dataset',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');
 await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('NormalForm',{timeout:180_000});await expect(page.getByTestId('domain-counts')).toContainText('0 pending · 100 processed');
 const bytes=await exported(page),trace=JSON.parse(bytes);expect(trace.events).toHaveLength(100);expect(trace.settings.plan.seed).toBeNull();expect(cli(['trace-check','-'],bytes)).toContain('Verified 100 events');
 await page.getByLabel('Inspect state',{exact:true}).fill('50');await expect(page.getByTestId('domain-counts')).toContainText('50 pending · 50 processed');
 await page.getByRole('button',{name:'First',exact:true}).click();await expect(page.getByTestId('domain-counts')).toContainText('100 pending · 0 processed');
 await page.getByRole('button',{name:'Latest',exact:true}).click();await expect(page.getByTestId('domain-counts')).toContainText('0 pending · 100 processed');expect(await exported(page)).toBe(bytes);
 await page.getByRole('button',{name:'Next items',exact:true}).click();await expect(page.getByLabel('Work items',{exact:true})).toContainText('Page 2 / 4');await page.screenshot({path:'../output/playwright/s5-hundred-items.png',fullPage:true});
});

test('reflective activation links the source, new exact revision and causal witness without mutating history',async({page})=>{
 await open(page);await page.getByLabel('Example',{exact:true}).selectOption({label:'practical/policy-enable'});await page.getByRole('button',{name:'Apply example settings'}).click();await load(page);
 await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('NormalForm');const bytes=await exported(page),trace=JSON.parse(bytes);
 await page.getByRole('button',{name:'History',exact:true}).click();await page.getByRole('button',{name:'Inspect event 1',exact:true}).click();await expect(page.getByLabel('Inspect state',{exact:true})).toHaveValue('2');
 await page.locator('.change-summary p').getByRole('button').first().click();await expect(page.locator('.inspector')).toContainText('RuleHead');
 await page.locator('.change-summary p').getByRole('button').nth(1).click();await expect(page.locator('.inspector')).toContainText('Revision');
 await expect(page.locator('.edge-list')).toContainText('(exact executable revision)');await page.locator('.edge-list').getByRole('button',{name:`ExactRevision: atom #${trace.events[1].redex.revision}`,exact:true}).click();
 await expect(page.getByLabel('Inspect state',{exact:true})).toHaveValue('1');await expect(page.locator('.inspector')).toContainText('Revision');await expect(page.locator('.change-summary')).toContainText('altered: process');expect(await exported(page)).toBe(bytes);
 await page.screenshot({path:'../output/playwright/s5-exact-revision.png',fullPage:true});
});
