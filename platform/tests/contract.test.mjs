import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';

test('OpenAPI routes, operation identities and local schema references are complete', () => {
  const doc = JSON.parse(fs.readFileSync('platform/openapi.json'));
  assert.equal(doc.openapi, '3.1.0');
  const operations = Object.values(doc.paths).flatMap(p => Object.values(p));
  assert.equal(operations.length, 12);
  assert.equal(new Set(operations.map(o => o.operationId)).size, operations.length);
  const visit = value => {
    if (!value || typeof value !== 'object') return;
    if (value.$ref) {
      const target = value.$ref.slice(2).split('/').reduce((o, k) => o?.[k], doc);
      assert.ok(target, `Unresolved reference ${value.$ref}`);
    }
    for (const child of Object.values(value)) visit(child);
  };
  visit(doc);
  for (const [route, methods] of Object.entries(doc.paths)) for (const operation of Object.values(methods)) {
    for (const [, parameter] of route.matchAll(/\{(\w+)\}/g))
      assert.ok(operation.parameters.some(p => p.name === parameter && p.in === 'path' && p.required));
    assert.ok(operation.responses.default);
  }
});
