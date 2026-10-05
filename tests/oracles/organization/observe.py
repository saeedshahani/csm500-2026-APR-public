"""Read-only historical measurement oracle; not production code."""
from collections import Counter


def inspect(graph):
    r = graph['root']; env = graph['environment']; n = r['n']
    weights = [[r['basal'], r['basal']] for _ in range(2)]
    active = 0
    for name in r['operators']:
        node = graph[name]; active += node['count']
        if r['feedback']:
            weights[node['input_symbol']][node['output_symbol']] += node['count']
    q = [[x / sum(row) for x in row] for row in weights]
    fidelity = max((q[0][0] + q[1][1]) / 2, (q[0][1] + q[1][0]) / 2)
    code = [None if row[0] == row[1] else int(row[1] > row[0]) for row in weights]
    distributions = [[.5, .5] if c is None else [float(c == 0), float(c == 1)] for c in code]
    checked = correct = 0
    for record in r['model'].values():
        checked += 1
        correct += sum(probability for action, probability in enumerate(distributions[record['action']])
                       if record['target'] == graph[record['from']]['next'][str(action)])
    window = r['history'][-100:]
    predictions = [w for w in window if w['predicted'] is not None]
    program_roles = Counter(graph[p]['role'] for p in r['programs'] if p is not None)
    return {'epoch': r['epoch'], 'energy': r['energy'], 'source': r['source'],
            'fidelity': fidelity, 'code': code, 'action_distributions': distributions,
            'decoder_deterministic': None not in code, 'active_fraction': active / n,
            'core_fractions': [graph[f't{i}']['count'] / n for i in r['core']],
            'model_records': checked, 'model_accuracy': correct / checked if checked else 0,
            'model_coverage': checked / (2 * len(env['states'])),
            'recent_predictions': len(predictions),
            'recent_correct': sum(w['predicted'] == w['observed'] for w in predictions),
            'recent_harvests': sum('collected' in w for w in window),
            'original_instances': {pool: sum(age == 0 for age in ages) for pool, ages in r['ages'].items()},
            'program_roles': dict(program_roles),
            **{key: r[key] for key in ('recognition_updates', 'model_updates', 'detected_errors',
                                      'checker_disagreements', 'checker_repairs', 'checked',
                                      'harvests', 'blocked_energy', 'blocked_symbols', 'actions')},
            'program_creations': r['created']['program'], 'program_removals': r['removed']['program']}


def audit(graph):
    r = graph['root']
    assert min(r['energy'], r['source'], r['spent_energy']) >= 0
    assert r['energy'] + r['source'] + r['spent_energy'] == r['initial_energy']
    for pool, positions in r['pools'].items():
        counts = Counter(x for x in positions if x is not None)
        assert len(positions) == r['n']
        total = r['reserves'][pool][:]
        assert min(total) >= 0
        for key, node in graph.items():
            if node.get('tag') == pool:
                assert node['count'] == counts[key], (pool, key)
                for a in node['sequence']:
                    total[a] += node['count']
        assert total == r['initial_symbols'][pool]
    assert len(set(p for p in r['programs'] if p is not None)) == sum(p is not None for p in r['programs'])
    for i, p in enumerate(r['programs']):
        if p is not None:
            assert graph[p]['operator'] == r['pools']['sequence'][i]
    return True


def functional(row):
    return (row['fidelity'] >= .9 and row['active_fraction'] >= .35
            and min(row['core_fractions']) >= .10 and row['model_accuracy'] >= .95
            and row['model_coverage'] >= .75 and row['recent_harvests'] >= 10
            and row['program_roles'].get(0, 0) > 0 and row['program_roles'].get(1, 0) > 0)
