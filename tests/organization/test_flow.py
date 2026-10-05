import json
import os
from pathlib import Path
import tempfile
import unittest
from tests.oracles.organization.__main__ import demo


class FlowTests(unittest.TestCase):
    @unittest.skipUnless(os.environ.get('ORGANIZATION_NATIVE'), 'set ORGANIZATION_NATIVE to the built project CLI')
    def test_one_lineage_reaches_admitted_native_execution(self):
        with tempfile.TemporaryDirectory() as root:
            dest=Path(root)/'flow'
            report=demo(dest,os.environ['ORGANIZATION_NATIVE'])
            self.assertTrue(report['parent']['functional'])
            self.assertTrue(report['child']['functional'])
            self.assertTrue(report['same_code'])
            self.assertTrue(report['environmental_agreement'])
            self.assertTrue(report['separate_process_trace_verified'])
            self.assertEqual(report['native_steps'],8)
            self.assertEqual(report['child']['remaining_founder_programs'],0)
            self.assertEqual(report['child_heldout']['passed'],512)
            self.assertEqual(json.loads(json.dumps(report)),json.loads((dest/'report.json').read_text()))
            with self.assertRaises(FileExistsError):demo(dest,os.environ['ORGANIZATION_NATIVE'])
