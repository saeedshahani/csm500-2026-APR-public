import test from 'node:test';
import assert from 'node:assert/strict';
import {assertSameExecution} from './trace-comparison.mjs';

const record = digit => ({trace: {schema: 'reflective-trace-v1', implementation: `engine-0.1.0-source-sha256=${digit.repeat(64)}`, events: [{source: '1'}], initial: {next_identity: '4'}, settings: {seed: '42'}}, compiled: {implementation: 'domain-value'}});
test('semantic comparison checks both histories and retains original provenance', () => {
  const a = record('a'), b = record('b'), verified = [];
  assertSameExecution(a, b, [['trace']], trace => verified.push(trace.implementation));
  assert.equal(verified.length, 2);
  assert.notEqual(a.trace.implementation, b.trace.implementation);
});
test('event, allocator, setting and non-trace changes remain failures', () => {
  for (const mutate of [x => x.trace.events[0].source = '2', x => x.trace.initial.next_identity = '5', x => x.trace.settings.seed = '43', x => x.compiled.implementation = 'changed']) {
    const b = record('b'); mutate(b);
    assert.throws(() => assertSameExecution(record('a'), b, [['trace']], () => {}));
  }
});
test('malformed provenance, missing paths and replay failure cannot be normalised away', () => {
  const b = record('b'); b.trace.implementation = 'unknown';
  assert.throws(() => assertSameExecution(record('a'), b, [['trace']], () => {}));
  assert.throws(() => assertSameExecution(record('a'), record('b'), [['absent']], () => {}));
  assert.throws(() => assertSameExecution(record('a'), record('b'), [['trace']], () => { throw Error('invalid replay'); }));
});
