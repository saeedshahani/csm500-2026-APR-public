// Uses the Playwright CLI against an already opened local workbench session.
// Native measurements must have finished before running this diagnostic.
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,readdirSync,mkdirSync} from 'node:fs';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
const [cli,session,directory]=process.argv.slice(2);
if(!cli||!session||!directory||process.argv.length!==5)throw Error('node scripts/browser-scale-diagnostic.mjs PLAYWRIGHT_CLI SESSION NEW_OUTPUT_DIRECTORY');
const code=`async (page) => {
  const frames = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const now = () => page.evaluate(() => performance.now());
  const rows = [];
  for (const size of [10,100,1000]) for (let repetition=0;repetition<3;repetition++) {
    await page.getByLabel('Dataset size').selectOption(String(size));
    await page.getByRole('button',{name:'Preview dataset',exact:true}).click();
    await page.getByRole('button',{name:'Load dataset',exact:true}).waitFor({state:'visible'});
    const start = await now();
    await page.getByRole('button',{name:'Load dataset',exact:true}).click();
    await page.waitForFunction(() => document.querySelector('[data-testid="status"]').textContent==='Ready');
    await frames(); const load_ms = (await now())-start;
    const before = await page.getByTestId('domain-counts').textContent();
    const next = await now();
    await page.getByRole('button',{name:'Step',exact:true}).click();
    await page.waitForFunction(expected => document.querySelector('[data-testid="status"]').textContent===expected,size===1000?'ResourceLimit':'Yielded',{timeout:60000});
    await frames(); const step_ms = (await now())-next;
    const after = await page.getByTestId('domain-counts').textContent();
    const canvas_atoms = await page.locator('svg .atom').count();
    if (!before.includes(size+' pending') || !after.includes((size===1000?size:size-1)+' pending') ||
        !after.includes((size===1000?0:1)+' processed') || canvas_atoms>250) throw Error('browser domain/viewport oracle failed');
    rows.push({size,repetition,load_ms,step_ms,before,after,canvas_atoms,status:await page.getByTestId('status').textContent()});
  }
  return {rows,browser:await page.evaluate(()=>navigator.userAgent),viewport:page.viewportSize(),url:page.url()};
}`;
mkdirSync(directory,{recursive:false});
const stdout=execFileSync(cli,[`-s=${session}`,'run-code',code],{encoding:'utf8',timeout:180000,maxBuffer:2*1024*1024});
writeFileSync(resolve(directory,'cli-output.txt'),stdout,{flag:'wx'});
const match=stdout.match(/### Result\n([^\n]+)/);if(!match)throw Error('no diagnostic result; raw CLI output retained');
const result=JSON.parse(match[1]);
const sha=b=>createHash('sha256').update(b).digest('hex');
const assets=Object.fromEntries(readdirSync('frontend/dist/assets').sort().map(name=>[name,sha(readFileSync(resolve('frontend/dist/assets',name)))]));
writeFileSync(resolve(directory,'diagnostic.json'),JSON.stringify({schema:'browser-scale-diagnostic-v1',date:new Date().toISOString(),purpose:'Three local UI observations per size, not a controlled engine benchmark or latency guarantee',script_sha256:sha(readFileSync(new URL(import.meta.url))),assets,...result},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(result,null,2));
