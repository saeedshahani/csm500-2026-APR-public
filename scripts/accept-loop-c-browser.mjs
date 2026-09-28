// Open the full completed calendar job in a Playwright CLI session first.
// Use a fresh snapshot to confirm the visible Calendar results controls.
import fs from 'node:fs';import path from 'node:path';import {execFileSync} from 'node:child_process';import assert from 'node:assert/strict';
import {hash} from '../platform/store.mjs';
const [cli,session,destination]=process.argv.slice(2);if(!cli||!session||!destination||fs.existsSync(destination))throw Error('Usage: node scripts/accept-loop-c-browser.mjs CLI_JS SESSION NEW_RESULT.json');
const code=`async page => {
 await page.setViewportSize({width:1280,height:900});
 const region=page.getByRole('region',{name:'Calendar results',exact:true});
 const now=()=>page.evaluate(()=>performance.now());
 const frames=()=>page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 if(!(await region.getByRole('status').textContent()).includes('12,330 of 12,330'))throw Error('Wrong result count');
 const navigation_ms=[],response_body_ms=[],render_after_body_ms=[],response_bytes=[];
 for(const offset of [25,50,75]){
  const response=page.waitForResponse(r=>r.url().includes('/month-results?')&&r.url().includes('offset='+offset));
  const t=await now();await region.getByRole('button',{name:'Next rows',exact:true}).click();
  const reply=await response,body=await reply.body(),received=await now(),parsed=JSON.parse(body.toString());
  if(parsed.items.length!==25||parsed.total!==12330||!parsed.complete||parsed.items[0].id!==offset+1)throw Error('Page oracle');
  await page.waitForFunction(n=>document.querySelector('[aria-label="Calendar results"] tbody tr td')?.textContent===String(n),offset+1);await frames();const rendered=await now();
  navigation_ms.push(rendered-t);response_body_ms.push(received-t);render_after_body_ms.push(rendered-received);response_bytes.push(body.length);
 }
 const downloaded=page.waitForEvent('download');await region.getByRole('button',{name:'Download normalised CSV',exact:true}).click();const download=await downloaded;await download.saveAs('output/playwright/loop-c-final.csv');
 await page.getByRole('combobox',{name:'Theme',exact:true}).selectOption({label:'Light'});await region.evaluate(el=>el.scrollIntoView({block:'start'}));await frames();await page.screenshot({path:'output/playwright/loop-c-final-light.png'});
 await page.getByRole('combobox',{name:'Theme',exact:true}).selectOption({label:'Dark'});await page.setViewportSize({width:390,height:844});await region.evaluate(el=>el.scrollIntoView({block:'start'}));await frames();const narrow_overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth);await page.screenshot({path:'output/playwright/loop-c-final-dark.png'});
 return {schema:'loop-c-browser-v1',total:12330,rows_rendered:await region.locator('tbody tr').count(),navigation_ms,response_body_ms,render_after_body_ms,response_bytes,narrow_overflow,browser:await page.evaluate(()=>navigator.userAgent),measured_viewport:{width:1280,height:900},narrow_viewport:page.viewportSize()};
}`;
const output=execFileSync(process.execPath,[cli,`-s=${session}`,'run-code',code],{encoding:'utf8',timeout:60000,maxBuffer:2*1024*1024});
const match=output.match(/### Result\n([^\n]+)/);if(!match)throw Error(output);
const result=JSON.parse(match[1]);result.csv_sha256=hash(fs.readFileSync('output/playwright/loop-c-final.csv'));
assert.equal(result.csv_sha256,'9d122207618b5f05204c9a12f16104213c0ab384df69eee2b7b424739f6901a7');
result.created_at=new Date().toISOString();result.script_sha256=hash(fs.readFileSync(new URL(import.meta.url)));
result.assets=Object.fromEntries(fs.readdirSync('frontend/dist/assets').sort().map(n=>[n,hash(fs.readFileSync(path.join('frontend/dist/assets',n)))]));
fs.writeFileSync(destination,JSON.stringify(result,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify(result,null,2));
