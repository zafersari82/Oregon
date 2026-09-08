"""Fail-closed tests for Mutation Evidence Manifest V1."""
from copy import deepcopy
from pathlib import Path
import tempfile
import unittest

from mutation_evidence import EvidenceError, load_manifest


EXPECTED = [('EA', 3), ('EE', 9), ('CS', 17), ('ER', 13), ('FS', 14), ('RJ', 12)]


def build_manifest(root: Path):
    authorities = []
    for prefix, count in EXPECTED:
        runner = root / 'scripts' / f'{prefix.lower()}_runner.py'
        runner.parent.mkdir(parents=True, exist_ok=True)
        runner.write_text('# test runner\n')
        authorities.append({
            'id': prefix,
            'display_name': prefix,
            'runner': str(runner.relative_to(root)),
            'runner_sha256': '0' * 64,
            'command': ['python3', str(runner.relative_to(root))],
            'expected_count': count,
            'mutations': [
                {
                    'id': f'{prefix}-{index:03d}',
                    'name': f'{prefix} mutation {index}',
                    'target': f'target/{prefix.lower()}',
                    'expected_broken_behavior': f'{prefix} broken {index}',
                    'killing_test': f'{prefix.lower()}_test_{index}',
                    'kill_classification': 'semantic-test-failure',
                }
                for index in range(1, count + 1)
            ],
        })
    return {
        'schema': 'oregon.mutation-evidence.manifest/v1',
        'manifest_version': 1,
        'public_claim_boundary': 'selected semantic mutations only; not formal proof',
        'authorities': authorities,
    }


class ManifestValidationTests(unittest.TestCase):
    def validate(self, mutate=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            document = build_manifest(root)
            if mutate is not None:
                mutate(document)
            path = root / 'manifest.json'
            import json
            path.write_text(json.dumps(document))
            return load_manifest(path, root)

    def test_valid_shape_has_six_authorities_and_68_mutations(self):
        _, authorities = self.validate()
        self.assertEqual([a.id for a in authorities], [p for p, _ in EXPECTED])
        self.assertEqual(sum(a.expected_count for a in authorities), 68)

    def test_duplicate_mutation_id_is_rejected(self):
        def mutate(document):
            document['authorities'][1]['mutations'][0]['id'] = 'EA-001'
        with self.assertRaises(EvidenceError):
            self.validate(mutate)

    def test_missing_mutation_is_rejected(self):
        def mutate(document):
            document['authorities'][-1]['mutations'].pop()
        with self.assertRaises(EvidenceError):
            self.validate(mutate)

    def test_extra_mutation_is_rejected(self):
        def mutate(document):
            extra = deepcopy(document['authorities'][0]['mutations'][-1])
            extra['id'] = 'EA-004'
            extra['name'] = 'unexpected extra mutation'
            document['authorities'][0]['mutations'].append(extra)
        with self.assertRaises(EvidenceError):
            self.validate(mutate)

    def test_five_authorities_are_rejected(self):
        def mutate(document):
            document['authorities'].pop()
        with self.assertRaises(EvidenceError):
            self.validate(mutate)

    def test_wrong_prefix_is_rejected(self):
        def mutate(document):
            document['authorities'][0]['mutations'][0]['id'] = 'EE-001'
        with self.assertRaises(EvidenceError):
            self.validate(mutate)

    def test_malformed_runner_digest_is_rejected(self):
        def mutate(document):
            document['authorities'][0]['runner_sha256'] = 'not-a-sha256'
        with self.assertRaises(EvidenceError):
            self.validate(mutate)

    def test_missing_runner_is_rejected(self):
        def mutate(document):
            document['authorities'][0]['runner'] = 'scripts/missing.py'
            document['authorities'][0]['command'] = ['python3', 'scripts/missing.py']
        with self.assertRaises(EvidenceError):
            self.validate(mutate)

    def test_command_must_bind_exact_runner(self):
        def mutate(document):
            document['authorities'][0]['command'] = ['python3', document['authorities'][1]['runner']]
        with self.assertRaises(EvidenceError):
            self.validate(mutate)


if __name__ == '__main__':
    unittest.main()
