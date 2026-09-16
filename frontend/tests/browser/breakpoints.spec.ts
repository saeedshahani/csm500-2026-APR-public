import {test,expect,type Page} from '@playwright/test';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import path from 'node:path';
async function exported(page:Page){const wait=page.waitForEvent('download');await page.getByRole('button',{name:'Export trace',exact:true}).click();return readFileSync((await(await wait).path())!,'utf8');}
async function load(page:Page,example:string){await page.goto('/');await expect(page.getByTestId('status')).toHaveText('No program loaded');await page.getByLabel('Example',{exact:true}).selectOption({label:example});await page.getByRole('button',{name:'Apply example settings'}).click();await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Ready');}
async function enable(page:Page,name=/Break on rule/){const checkbox=page.getByRole('checkbox',{name});if(!await checkbox.isVisible())await page.locator('summary').filter({hasText:/^Rule breakpoints/}).click();await checkbox.click();await expect(checkbox).toBeChecked();}

test('seeded breakpoints preserve selected continuation through step, history, recovery and import',async({page})=>{
 await load(page,'practical/work-items');await page.getByText('Rule breakpoints (0)',{exact:true}).click();
 const initial=await exported(page);await enable(page,/Break on rule process/);expect(await exported(page)).toBe(initial);
 await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Breakpoint');expect(await exported(page)).toBe(initial);
 await expect(page.getByRole('status').filter({hasText:'Paused at state'})).toContainText('Paused at state 0');
 await page.getByRole('button',{name:'Step',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Yielded');expect(JSON.parse(await exported(page)).events).toHaveLength(1);
 await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Breakpoint');expect(JSON.parse(await exported(page)).events).toHaveLength(1);
 await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Breakpoint');const prefix=await exported(page);expect(JSON.parse(prefix).events).toHaveLength(2);
 await page.getByRole('button',{name:'First',exact:true}).click();await expect(page.getByRole('button',{name:'Step',exact:true})).toBeDisabled();expect(await exported(page)).toBe(prefix);
 await page.getByRole('button',{name:'Latest',exact:true}).click();const pause=await page.getByRole('status').filter({hasText:'Paused at state'}).textContent();
 await page.getByRole('textbox',{name:'Program source'}).fill('broken draft preserved');await page.getByRole('button',{name:'Validate draft'}).click();await page.getByRole('button',{name:'Recover engine'}).click();await expect(page.getByRole('alert')).toHaveCount(0);
 await expect(page.getByTestId('status')).toHaveText('Breakpoint');expect(await page.getByRole('status').filter({hasText:'Paused at state'}).textContent()).toBe(pause);expect(await exported(page)).toBe(prefix);
 await expect(page.getByRole('checkbox',{name:/Break on rule/})).toBeChecked();await expect(page.getByRole('textbox',{name:'Program source'})).toContainText('broken draft preserved');
 await page.setViewportSize({width:390,height:844});await page.getByLabel('Theme').selectOption('dark');expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
 await page.screenshot({path:'../output/playwright/s5-breakpoint-dark.png',fullPage:true});
 await page.getByLabel('Import trace',{exact:true}).setInputFiles({name:'prefix.json',mimeType:'application/json',buffer:Buffer.from(prefix)});await expect(page.getByTestId('status')).toHaveText('Paused');await expect(page.getByRole('checkbox',{name:/Break on rule/,includeHidden:true})).not.toBeChecked();
 await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('NormalForm');
 const complete=await exported(page),doc=JSON.parse(complete);
 const expected=execFileSync('cargo',['run','--quiet','--locked','-p','cli','--','export','examples/practical/work-items.graph','--max-steps',doc.settings.plan.max_steps,'--strategy','seeded','--seed',doc.settings.plan.seed],{cwd:path.resolve('..'),encoding:'utf8'});
 expect(doc).toEqual(JSON.parse(expected));
 await enable(page);await page.getByRole('button',{name:'Reset',exact:true}).click();await expect(page.getByRole('checkbox',{name:/Break on rule/,includeHidden:true})).not.toBeChecked();await expect(page.getByTestId('status')).toHaveText('Ready');
});

test('a breakpoint follows activation and shows the replacement executable revision',async({page})=>{
 await load(page,'practical/policy-enable');const initial=JSON.parse(await exported(page));
 await page.getByText('Rule breakpoints (0)',{exact:true}).click();await enable(page,/Break on rule process/);
 await page.locator('summary').filter({hasText:/^Run settings/}).click();await page.getByLabel('Commits per batch').selectOption('10');
 await page.getByRole('button',{name:'Run',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Breakpoint');
 const paused=JSON.parse(await exported(page));expect(paused.events).toHaveLength(1);const context=page.getByRole('status').filter({hasText:'Paused at state'});
 await expect(context).toContainText('Paused at state 1');const revision=await context.getByRole('button').nth(1).textContent();
 expect(initial.initial.atoms[revision!.slice(1)]).toBeUndefined();await context.getByRole('button').nth(1).click();await expect(page.locator('.inspector')).toContainText('Revision');
 await page.getByRole('button',{name:'Step',exact:true}).click();await expect(page.getByTestId('status')).toHaveText('Yielded');const stepped=JSON.parse(await exported(page));expect(stepped.events[1].redex.revision).toBe(revision!.slice(1));
 await page.getByRole('button',{name:'Load program',exact:true}).click();await expect(page.getByRole('checkbox',{name:/Break on rule process/,includeHidden:true})).not.toBeChecked();
});
