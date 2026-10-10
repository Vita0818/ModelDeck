import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SOURCE = Path(__file__).resolve().parents[1] / 'run.py'
SPEC = importlib.util.spec_from_file_location('harness_run', SOURCE)
RUN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUN)


class CredentialResolutionTests(unittest.TestCase):
    def test_file_reference_becomes_the_actual_bearer_credential(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'key with spaces'
            path.write_text('fixture-file-token\n')
            credential = RUN.resolve_credential('{file:' + str(path) + '}', temporary, {})
            response = mock.MagicMock()
            response.__enter__.return_value.status = 200
            response.__enter__.return_value.headers = {}
            response.__enter__.return_value.read.return_value = b'{}'
            with mock.patch.object(RUN.urllib.request, 'urlopen', return_value=response) as transport:
                RUN.request('https://example.invalid/v1/models', credential)
            request = transport.call_args.args[0]
            self.assertEqual(request.get_header('Authorization'), 'Bearer fixture-file-token')
            self.assertNotIn('{file:', request.get_header('Authorization'))

    def test_relative_file_is_resolved_from_the_config_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'token').write_text('relative-token\n')
            self.assertEqual(RUN.resolve_credential('{file:token}', root, {}), 'relative-token')

    def test_environment_reference_uses_the_provided_environment(self):
        self.assertEqual(RUN.resolve_credential('{env:ARENA_TEST_KEY}', Path('/unused'),
            {'ARENA_TEST_KEY': 'fixture-env-token'}), 'fixture-env-token')

    def test_literal_credential_remains_supported(self):
        self.assertEqual(RUN.resolve_credential('literal-token', Path('/unused'), {}), 'literal-token')

    def test_missing_or_empty_references_fail_before_network(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'empty').write_text('\n')
            for reference in ['{env:MISSING}', '{file:missing}', '{file:empty}']:
                with self.subTest(reference=reference), self.assertRaises(ValueError):
                    RUN.resolve_credential(reference, root, {})

    def test_unresolved_reference_cannot_be_sent_as_a_bearer(self):
        with mock.patch.object(RUN.urllib.request, 'urlopen') as transport:
            with self.assertRaises(ValueError):
                RUN.request('https://example.invalid/v1/models', '{file:/never-read}')
            transport.assert_not_called()


if __name__ == '__main__':
    unittest.main()
