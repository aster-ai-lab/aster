import csv
import io
import unittest
from pathlib import Path
import tempfile
import xml.etree.ElementTree as ET
import zipfile

from workspace_wiki_native import FIXTURE, fixture_files, verify_published_citations, digest


class WorkspaceWikiFixtureTests(unittest.TestCase):
    def test_citations_require_matching_current_page_and_source_generations(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            page = root / 'overview.md'
            page.write_text('[Evidence](_sources/source.md#L1-L2)')
            source = {'current': True, 'sourcePath': 'source.md', 'pageGeneration': 'sha256:' + digest(page), 'sourceGeneration': 'sha256:' + 'a' * 64}
            def owner(_command):
                return {'result': source}
            self.assertEqual(len(verify_published_citations([page], root, {'source.md': 'a' * 64}, owner)), 1)
            for field, value in [('current', False), ('sourcePath', '../foreign.md'), ('pageGeneration', 'changed'), ('sourceGeneration', 'changed')]:
                saved = source[field]
                source[field] = value
                with self.assertRaises(AssertionError):
                    verify_published_citations([page], root, {'source.md': 'a' * 64}, owner)
                source[field] = saved

    def test_original_code_and_cancel_sources_are_preserved(self):
        self.assertEqual(fixture_files('code'), {path: text.encode() for path, text in FIXTURE.items()})

    def test_document_and_mixed_sources_have_independent_reproducible_facts(self):
        documents = fixture_files('documents')
        self.assertFalse(any(path.endswith('.py') for path in documents))
        rows = list(csv.DictReader(io.StringIO(documents['data/requests.csv'].decode())))
        self.assertEqual(len(rows), 12)
        self.assertEqual(sum(row['status'] == 'closed' for row in rows), 9)
        with zipfile.ZipFile(io.BytesIO(documents['policy/service-policy.docx'])) as archive:
            text = ''.join(ET.fromstring(archive.read('word/document.xml')).itertext())
            self.assertIn('80 percent', text)
            self.assertIn('data/requests.csv', text)
        mixed = fixture_files('mixed')
        self.assertEqual(mixed.keys() - documents.keys(), {'analysis.py'})
        self.assertEqual({name: body for name, body in mixed.items() if name != 'analysis.py'}, documents)


if __name__ == '__main__':
    unittest.main()
