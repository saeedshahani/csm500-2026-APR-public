import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {rustNotices} from '../frontend/scripts/build-notices.mjs';

test('binary distribution preserves locked JavaScript and WebAssembly dependency notices',()=>{
  const text=rustNotices(process.cwd());
  for(const name of ['serde','serde_json','wasm-bindgen','rand','num-rational'])
    assert.ok(text.includes(`## ${name} `),name);
  assert.match(text,/Permission is hereby granted/);
  assert.match(text,/Apache License/);
  assert.ok(!text.includes('## engine '));
  assert.ok(!text.includes('## criterion '));
  assert.ok(!text.includes('## anes '));
  const config=readFileSync('frontend/vite.config.ts','utf8');
  assert.ok(config.includes("license:{fileName:'third-party-notices.md'}"));
  assert.ok(JSON.parse(readFileSync('frontend/package.json')).scripts.build.endsWith('node scripts/build-notices.mjs'));
});
