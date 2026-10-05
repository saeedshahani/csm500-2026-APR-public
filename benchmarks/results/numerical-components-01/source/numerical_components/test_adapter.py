import json
import copy
import unittest
from fractions import Fraction as Q
from dataclasses import replace
from adapter import (Component, Receipt, checked_task, task_data, fraction, digest,
                     residual_radius, solve, MAX_RECEIPTS, verify_packet)

TASK = [[{'weights': ['1/2', '1/4'], 'reward': '1/8', 'boundary': '1/8'},
         {'weights': ['1/4', '1/4'], 'reward': '1/8', 'boundary': '1/4'}],
        [{'weights': ['1/4', '1/4'], 'reward': '1/4', 'boundary': '1/8'},
         {'weights': ['1/8', '1/4'], 'reward': '1/8', 'boundary': '1/2'}]]


class AdapterTests(unittest.TestCase):
    def fresh(self):
        return Component('retry-workflow', TASK, '1/4')

    def test_residual_exact_target_and_arbitrary_candidate(self):
        task = checked_task([[{'weights': ['3/4'], 'reward': '1/8', 'boundary': '1/8'}]])
        for y in [Q(0), Q(1, 3), Q(5, 8), Q(1)]:
            r, _, _ = residual_radius(task, Q(1,4), (y,))
            self.assertLessEqual(abs(y-Q(5,8)), r)

    def test_three_modes_and_uncertified(self):
        c = self.fresh()
        r, _ = c.compute('raw', 0)
        self.assertEqual(c.assess(r, '1/1000', 0)['status'], 'uncertified')
        for mode in ('raw', 'compiled', 'direct'):
            r, _ = c.compute(mode, 128)
            self.assertEqual(c.assess(r, '1/1000', 0)['status'], 'certified')

    def test_receipt_every_field_tamper_and_rehash(self):
        c = self.fresh(); r, _ = c.compute('compiled')
        for field in r.data():
            d = r.data(); d[field] = 'forged'
            content = json.dumps(d, sort_keys=True, separators=(',', ':'))
            for key in (r.key, digest(d)):
                with self.assertRaises(ValueError):
                    c.assess(Receipt(content, key), '1/100', 0)

    def test_swapped_component_receipt(self):
        c = self.fresh(); r, _ = c.compute()
        other = Component('different-task', TASK, '3/4')
        other.compute()
        with self.assertRaises(ValueError): other.assess(r, '1/10', 0)

    def test_version_change_is_stale_then_recovers(self):
        c = self.fresh(); r, _ = c.compute('direct')
        c.revise(TASK, '3/4', 1)
        self.assertEqual(c.assess(r, '1/100', 1)['status'], 'stale')
        new, _ = c.compute('compiled')
        self.assertEqual(c.assess(new, '1/100', 1)['status'], 'certified')
        self.assertNotEqual(r.data()['revision'], new.data()['revision'])

    def test_depart_rejoin_fences_old_incarnation(self):
        c = self.fresh(); r, _ = c.compute('direct'); c.depart()
        self.assertEqual(c.assess(r, '1/100', 1)['status'], 'stale')
        with self.assertRaises(ValueError): c.compute()
        c.rejoin('1/4', 2)
        self.assertEqual(c.assess(r, '1/100', 2)['status'], 'stale')
        new, _ = c.compute('direct')
        self.assertEqual(new.data()['epoch'], 1)
        self.assertEqual(c.assess(new, '1/100', 2)['status'], 'certified')

    def test_expiry_and_clock_regression(self):
        c = self.fresh(); r, _ = c.compute('direct')
        self.assertEqual(c.assess(r, '1/100', 33)['status'], 'stale')
        with self.assertRaises(ValueError): c.assess(r, '1/100', 32)
        self.assertEqual(c.assess(r, '1/100', 34)['status'], 'stale')

    def test_understated_variation_latches(self):
        c = self.fresh(); r, _ = c.compute('direct')
        with self.assertRaises(ValueError): c.observe('3/4', 1, 1, '1/8')
        self.assertEqual(c.assess(r, '1/100', 1)['status'], 'stale')
        with self.assertRaises(ValueError): c.compute()

    def test_sequence_gap_replay_and_backward_clock(self):
        for seq, tick in [(0, 1), (2, 1), (1, 0)]:
            c = self.fresh(); c.tick = 1
            with self.assertRaises(ValueError): c.observe('1/4', seq, tick)
            self.assertIsNotNone(c.fault)

    def test_observed_movement_transports_existing_receipt(self):
        c = self.fresh(); r, _ = c.compute('direct')
        before = c.assess(r, '1/100', 0)
        c.observe('3/4', 1, 1)
        after = c.assess(r, '1/100', 1)
        self.assertEqual(after['status'], 'uncertified')
        self.assertGreater(Q(after['radius_now']), Q(before['radius_now']))
        self.assertEqual(c.variation, Q(1,2))

    def test_fixed_precision_floor(self):
        c = self.fresh(); r, _ = c.compute('compiled', bits=4)
        self.assertEqual(c.assess(r, '1/1000000', 0)['status'], 'uncertified')
        self.assertGreater(Q(r.data()['radius']), 0)

    def test_bounded_input_work_and_retention(self):
        c = self.fresh()
        for value in [float('nan'), True, '1/0', '1/'+str(1<<129), 1<<129]:
            with self.assertRaises(ValueError): fraction(value)
        for mode, steps, bits in [('raw', 4097, 24), ('raw', 1, 65), ('bad', 1, 24)]:
            with self.assertRaises(ValueError): c.compute(mode, steps, bits)
        for n in range(MAX_RECEIPTS):
            c.observe(str(Q(n,128)), n+1, n)
            c.compute('raw', 0)
        with self.assertRaises(ValueError): c.compute()

    def test_invalid_revision_transactional(self):
        c = self.fresh(); r, _ = c.compute('direct')
        with self.assertRaises(ValueError): c.revise([], '1/4', 0)
        self.assertEqual(c.assess(r, '1/100', 0)['status'], 'certified')

    def test_exported_certificate_recomputation_and_context(self):
        c = self.fresh(); r, _ = c.compute('direct')
        packet = c.assess(r, '1/100', 0)
        verify_packet(packet)
        mutations = [('radius_now', '0'), ('status', 'stale'), ('checked_at', -1)]
        for key, value in mutations:
            bad = copy.deepcopy(packet); bad[key] = value
            with self.assertRaises(ValueError): verify_packet(bad)
        bad = copy.deepcopy(packet); bad['receipt']['values'][0] = '0'
        bad['receipt_digest'] = digest(bad['receipt'])
        with self.assertRaises(ValueError): verify_packet(bad)
        bad = copy.deepcopy(packet); bad['current']['revision'] += 1
        with self.assertRaises(ValueError): verify_packet(bad)
        bad = copy.deepcopy(packet); bad['current']['boundary'] = '1'
        with self.assertRaises(ValueError): verify_packet(bad)

    def test_export_rejects_invalid_boundary_and_tolerance(self):
        c = self.fresh(); r, _ = c.compute('direct')
        packet = c.assess(r, '1/100', 0)
        for section, field, value in [(None, 'tolerance', '2'),
                ('current', 'boundary', '2'), (None, 'max_age', -1),
                ('receipt', 'boundary', '-1'), (None, 'observed_variation', '-1')]:
            bad = copy.deepcopy(packet)
            (bad if section is None else bad[section])[field] = value
            with self.assertRaises(ValueError): verify_packet(bad)

    def test_task_content_change_and_new_session(self):
        c = self.fresh(); r, _ = c.compute('compiled')
        replacement = copy.deepcopy(TASK)
        replacement[0][0]['reward'] = '1/16'
        c.revise(replacement, '1/4', 1)
        self.assertEqual(c.assess(r, '1/100', 1)['status'], 'stale')
        new, _ = c.compute('compiled')
        self.assertNotEqual(r.data()['task_digest'], new.data()['task_digest'])
        verify_packet(c.assess(new, '1/100', 1))
        restarted = self.fresh()
        with self.assertRaises(ValueError): restarted.assess(r, '1/100', 0)

    def test_constant_map_cycle_counterexample(self):
        endpoints = []; phases = []
        x = Q(0)
        for _ in range(100):
            x = Q(1); phases.append(x)
            x = Q(0); endpoints.append(x)
        self.assertTrue(all(v == 0 for v in endpoints))
        self.assertTrue(all(v == 1 for v in phases))
        # Both Lipschitz constants are zero; phase forcing must nevertheless be 1.
        self.assertEqual(max(phases), 1)

    def test_corrected_pair_and_persistent_forcing(self):
        for noise in (Q(0), Q(1,1000)):
            error = Q(1); previous = error
            for k in range(250):
                previous = error
                intermediate = Q(5,4)*error+noise
                error = Q(1,4)*intermediate+noise
                self.assertEqual(error, Q(5,16)*previous+Q(5,4)*noise)
            limit = Q(5,4)*noise/(1-Q(5,16))
            self.assertLess(error-limit, Q(1,10**30))
            self.assertLess(intermediate-(Q(5,4)*limit+noise), Q(1,10**30))


if __name__ == '__main__': unittest.main()
