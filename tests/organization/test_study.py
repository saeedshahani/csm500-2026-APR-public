import json
from pathlib import Path
import tempfile
import unittest

from tests.oracles.organization.study import compare_outputs


class ReproductionTests(unittest.TestCase):
    def test_exact_artifact_inventory_is_required(self):
        with tempfile.TemporaryDirectory() as directory:
            reference = Path(directory) / 'reference'
            run = Path(directory) / 'run'
            reference.mkdir()
            run.mkdir()
            (reference / 'manifest.json').write_text(json.dumps({'analysis.json': 'unused'}))
            (reference / 'analysis.json').write_text('{}')
            with self.assertRaisesRegex(ValueError, 'artifact set mismatch'):
                compare_outputs(reference, run)
            (run / 'analysis.json').write_text('{}')
            self.assertEqual(compare_outputs(reference, run), 1)
            (run / 'unexpected.json').write_text('{}')
            with self.assertRaisesRegex(ValueError, 'unexpected'):
                compare_outputs(reference, run)
            (run / 'unexpected.json').unlink()
            (run / 'analysis.json').write_text('{"changed":true}')
            with self.assertRaisesRegex(ValueError, 'reproduction mismatch'):
                compare_outputs(reference, run)

    def test_empty_reconstruction_cannot_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'manifest.json').write_text('{}')
            with self.assertRaisesRegex(ValueError, 'artifact set mismatch'):
                compare_outputs(root, root / 'missing')
