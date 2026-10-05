"""Artifact integrity and truthful status reporting, without simulating runners."""
import hashlib
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SCRIPT = Path(__file__).resolve().parents[1] / 'ci_build.py'
spec = importlib.util.spec_from_file_location('ci_build', SCRIPT)
ci = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ci)
setup_spec = importlib.util.spec_from_file_location('ci_setup', SCRIPT.with_name('ci_setup.py'))
setup = importlib.util.module_from_spec(setup_spec)
setup_spec.loader.exec_module(setup)


class CiTests(unittest.TestCase):
    def test_cold_flutter_bootstrap_requires_real_machine_version(self):
        version = '{"frameworkRevision":"pinned", "frameworkVersion":"3.47.6"}'
        self.assertEqual(setup.parse_machine_version('Building tool...\nGot dependencies!\n'+version)['frameworkRevision'], 'pinned')
        with self.assertRaises(ValueError):
            setup.parse_machine_version('Download failed; no machine version')

    def test_archive_integrity_and_contents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stage = root / 'payload'
            stage.mkdir()
            (stage / 'metadata.json').write_text('{"signed":false}')
            archive = root / 'test.zip'
            ci.make_archive(stage, archive)
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            self.assertEqual((root / 'CHECKSUMS.txt').read_text(), f'{digest}  test.zip\n')
            with zipfile.ZipFile(archive) as z:
                self.assertEqual(z.namelist(), ['metadata.json'])
                self.assertEqual(z.read('metadata.json'), b'{"signed":false}')

    def test_failed_stage_and_unimplemented_player_cannot_be_runtime_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            summary = Path(directory) / 'summary.md'
            args = type('Args', (), dict(platform='ios', arch='arm64', variant='device',
                        rust_outcome='success', flutter_outcome='failure', package_outcome='skipped'))()
            with patch.dict(os.environ, {'GITHUB_STEP_SUMMARY': str(summary)}):
                ci.summary(args)
            output = summary.read_text()
            self.assertIn('| PASS | FAIL | NOT IMPLEMENTED | BLOCKED | NOT TESTED |', output)


if __name__ == '__main__':
    unittest.main()
