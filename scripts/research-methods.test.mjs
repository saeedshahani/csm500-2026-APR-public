// S6.I, methods cycle M01. Finite specification experiments, NOT an engine
// implementation, a reflective graph encoding, or a benchmark of other tools.
import assert from 'node:assert/strict';
import test from 'node:test';

const count = mask => mask.toString(2).replaceAll('0', '').length;
const key = s => JSON.stringify(s);

// Independent of any supplied ranking function: exhaust the finite transition
// graph, checking closure, outside-region deadlocks, and outside-region cycles.
function analyse(states, successors, legitimate) {
  const domain = new Set(states.map(key));
  const edges = new Map();
  let closure = true;
  let deadlocks = 0;
  let transitions = 0;
  for (const s of states) {
    const next = successors(s);
    transitions += next.length;
    for (const t of next) assert.ok(domain.has(key(t)), 'closed model domain');
    if (legitimate(s)) closure &&= next.every(legitimate);
    else if (!next.length) deadlocks++;
    edges.set(key(s), legitimate(s) ? [] : next.filter(t => !legitimate(t)).map(key));
  }
  const grey = new Set(), black = new Set();
  function visit(s) {
    if (grey.has(s)) return true;
    if (black.has(s)) return false;
    grey.add(s);
    for (const t of edges.get(s)) if (visit(t)) return true;
    grey.delete(s);
    black.add(s);
    return false;
  }
  const cycle = [...edges.keys()].some(visit);
  return {states: states.length, transitions, closure, deadlocks, cycle};
}

function workModel(n, allowance) {
  const states = [];
  for (let mask = 0; mask < 2 ** n; mask++)
    for (let policy = 0; policy < 2; policy++)
      for (let credit = 0; credit <= allowance; credit++)
        states.push({mask, policy, credit});
  const legitimate = s => s.mask === 0;
  const successors = s => {
    if (legitimate(s)) return [{...s, policy: 1 - s.policy}];
    const pending = Array.from({length: n}, (_, i) => i).filter(i => s.mask & (1 << i));
    const item = s.policy ? pending.at(-1) : pending[0];
    const next = [{...s, mask: s.mask & ~(1 << item), credit: allowance}];
    if (s.credit) next.push({...s, policy: 1 - s.policy, credit: s.credit - 1});
    return next;
  };
  return {states, legitimate, successors};
}

test('M01 bounded policy changes converge, with persistent legitimate activity', t => {
  let states = 0, transitions = 0;
  for (let n = 0; n <= 6; n++) for (let b = 0; b <= 3; b++) {
    const m = workModel(n, b);
    const result = analyse(m.states, m.successors, m.legitimate);
    assert.equal(result.closure, true);
    assert.equal(result.deadlocks, 0);
    assert.equal(result.cycle, false);
    for (const s of m.states) for (const next of m.successors(s)) {
      if (!m.legitimate(s))
        assert.ok((b + 1) * count(next.mask) + next.credit < (b + 1) * count(s.mask) + s.credit);
      else assert.notEqual(next.policy, s.policy); // not a normal form
    }
    states += result.states;
    transitions += result.transitions;
  }
  assert.equal(states, 2540);
  assert.equal(transitions, 3980);
  t.diagnostic(JSON.stringify({models: 28, states, transitions}));
});

test('M02 separately terminating policies can cycle when alternated', () => {
  const states = [0, 1];
  assert.equal(analyse(states, x => x === 0 ? [1] : [], () => false).cycle, false);
  assert.equal(analyse(states, x => x === 1 ? [0] : [], () => false).cycle, false);
  assert.equal(analyse(states, x => [1 - x], () => false).cycle, true);
});

test('M03 removing the policy-change allowance admits starvation outside legitimacy', () => {
  const m = workModel(1, 0);
  const unlimited = s => [...m.successors(s), {...s, policy: 1 - s.policy}];
  assert.equal(analyse(m.states, unlimited, m.legitimate).cycle, true);
  // The repair at either particular policy is repeatedly disabled by switching;
  // weak fairness for that occurrence does not rule out the switching execution.
  assert.equal(unlimited(m.states.find(s => s.mask === 1)).length, 2);
});

test('M04 continuing inputs break closure, and no-progress states break recovery', () => {
  const m = workModel(1, 0);
  const arrivals = s => [...m.successors(s), {...s, mask: 1}];
  assert.equal(analyse(m.states, arrivals, m.legitimate).closure, false);
  assert.equal(analyse(m.states, s => s.mask ? [] : m.successors(s), m.legitimate).deadlocks, 2);
});

test('M05 a rising success ratio is not a natural-valued repair measure', () => {
  let previous = 0;
  for (let good = 1; good <= 100; good++) {
    const unresolved = 1;
    const ratio = good / (good + unresolved);
    assert.ok(ratio > previous);
    assert.equal(unresolved, 1);
    previous = ratio;
  }
  // Algebraically good/(good+1) < (good+1)/(good+2) for every good >= 0;
  // the loop above is an illustration, not proof by 100 observations.
});

test('M06 multiple visibility blockers require multiplicity-aware retraction', t => {
  const actions = [['add', 'a', 'x'], ['add', 'b', 'x'], ['add', 'c', 'y'],
    ['remove', 'a'], ['remove', 'b'], ['remove', 'c']];
  let prefixes = 0;
  function explore(owners, counts, depth) {
    prefixes++;
    for (const target of ['x', 'y']) {
      const reference = [...owners.values()].filter(v => v === target).length;
      assert.equal(counts.get(target) ?? 0, reference);
    }
    if (!depth) return;
    for (const [op, id, target] of actions) {
      const next = new Map(owners), cached = new Map(counts);
      if (op === 'add') {
        if (!next.has(id)) { next.set(id, target); cached.set(target, (cached.get(target) ?? 0) + 1); }
      } else if (next.has(id)) {
        const old = next.get(id);
        next.delete(id); cached.set(old, cached.get(old) - 1);
      }
      explore(next, cached, depth - 1);
    }
  }
  explore(new Map(), new Map(), 5);
  assert.equal(prefixes, 9331);
  const owners = new Map([['a', 'x'], ['b', 'x']]);
  owners.delete('a');
  assert.equal([...owners.values()].includes('x'), true); // still hidden
  t.diagnostic(JSON.stringify({prefixes, maxDepth: 5, actions: 6}));
});

test('M07 absence predicates can have conjunctive or alternative explanations', () => {
  const clearA = ([, b]) => [false, b];
  const clearB = ([a]) => [a, false];
  const visible = ([a, b]) => !a && !b;
  const forbiddenPairAbsent = ([a, b]) => !(a && b);
  assert.equal(visible(clearA([true, true])), false);
  assert.equal(visible(clearB([true, true])), false);
  assert.equal(visible(clearB(clearA([true, true]))), true);
  assert.equal(forbiddenPairAbsent(clearA([true, true])), true);
  assert.equal(forbiddenPairAbsent(clearB([true, true])), true);
  // One fixed prerequisite edge cannot express "clear A OR clear B".
});

test('M08 raw relational joins must retain the engine injectivity condition', () => {
  const objects = [0, 1, 2];
  const cartesian = objects.flatMap(a => objects.map(b => [a, b]));
  assert.equal(cartesian.length, 9);
  assert.equal(cartesian.filter(([a, b]) => a !== b).length, 6);
});

test('M09 fixed-domain saturation and destructive normalisation have different outputs', () => {
  const start = new Set(['pending:1', 'pending:2']);
  const saturated = new Set([...start, 'done:1', 'done:2']);
  const replaced = new Set(['done:1', 'done:2']);
  assert.notDeepEqual(saturated, replaced);
  assert.deepEqual([...saturated].filter(x => x.startsWith('done:')), [...replaced]);
  // Only the selected projection agrees. Neither result implies equality of
  // carriers, intermediate choices, fresh identities or recorded events.
});

test('M10 degree-sensitive indices prune irrelevant edges, not the output itself', () => {
  for (const degree of [1, 10, 1000]) {
    const edges = Array.from({length: degree}, (_, i) => ({id: i, kind: i === degree - 1 ? 'wanted' : 'other'}));
    const reference = edges.filter(e => e.kind === 'wanted');
    const index = new Map();
    for (const edge of edges) index.set(edge.kind, [...(index.get(edge.kind) ?? []), edge]);
    assert.deepEqual(index.get('wanted'), reference);
    assert.equal(index.get('wanted').length, 1);
    // Construction still inspects degree records; this is not constant-time
    // end-to-end matching. If every edge is wanted, degree outputs remain.
    assert.equal(edges.map(e => ({...e, kind: 'wanted'})).filter(e => e.kind === 'wanted').length, degree);
  }
});
