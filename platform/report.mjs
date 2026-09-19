// Descriptive summaries of an already checked, hash-verified trace. No admission logic.
export function summarize(trace, digest) {
  const initial = trace.initial.atoms, final = new Map(Object.entries(initial));
  const altered = new Set();
  let consumed = 0, produced = 0;
  for (const event of trace.events) {
    for (const id of event.footprint.consumed) { final.delete(id); consumed++; }
    for (const [id, atom] of Object.entries(event.fresh)) { final.set(id, atom); produced++; }
    for (const id of event.footprint.altered) altered.add(id);
  }
  const count = values => {
    const kinds = {}, roles = {}; let atoms = 0, fields = 0;
    for (const atom of values) {
      atoms++; fields += atom.fields.length;
      const kind = typeof atom.kind === 'string' ? atom.kind : Object.keys(atom.kind)[0];
      kinds[kind] = (kinds[kind] ?? 0) + 1;
      const role = atom.kind.Record?.role;
      if (role) roles[role] = (roles[role] ?? 0) + 1;
    }
    return { atoms, fields, kinds, roles };
  };
  return { schema: 'reflective-run-summary-v1', trace_sha256: digest, outcome: trace.report?.outcome ?? null,
    commits: trace.events.length, initial: count(Object.values(initial)), final: count(final.values()),
    consumed_total: consumed, produced_total: produced, altered_rule_identities: [...altered],
    limitation: 'Carrier counts include quoted syntax and rule metadata. Totals are not graph equivalence, convergence or semantic usefulness. Identities are comparable only within this history.' };
}
