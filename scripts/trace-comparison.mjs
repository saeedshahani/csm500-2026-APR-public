// Historical semantic conformance: retain build provenance, compare all execution data.
import assert from 'node:assert/strict';

export function assertSameExecution(actual, expected, tracePaths, verify) {
  assert.equal(typeof verify, 'function', 'both histories require checked replay');
  const copies = [structuredClone(actual), structuredClone(expected)];
  for (const path of tracePaths) {
    assert.ok(Array.isArray(path) && path.length > 0);
    for (const copy of copies) {
      const trace = path.reduce((value, key) => value?.[key], copy);
      assert.equal(trace?.schema, 'reflective-trace-v1');
      assert.match(trace.implementation, /^engine-[^-]+-source-sha256=[0-9a-f]{64}$/);
      verify(trace); // Verify the original provenance-bearing value before projection.
      delete trace.implementation;
    }
  }
  assert.deepEqual(...copies);
}
