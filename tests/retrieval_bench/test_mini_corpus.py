"""Corpus integrity: every graded chunk exists in the corpus at the graded line."""

import unittest
from pathlib import Path
from mini_corpus import FILES, QUERIES, write_corpus


class CorpusTests(unittest.TestCase):
    def test_every_graded_chunk_exists(self):
        root = Path("/tmp/aster-bench-corpus-selftest")
        root.mkdir(parents=True, exist_ok=True)
        write_corpus(root)
        for query_id, spec in QUERIES.items():
            for chunk, grade in spec["grades"].items():
                file_name, line = chunk.rsplit(":", 1)
                self.assertIn(file_name, FILES, f"{query_id} grades unknown file")
                lines = FILES[file_name].splitlines()
                position = int(line) - 1
                self.assertLess(position, len(lines), f"{chunk} out of range")
                self.assertGreaterEqual(grade, 0)
                self.assertLessEqual(grade, 3)

    def test_late_line_annotation_is_real(self):
        # The flicker answer sits past line 900 — the bounded-snippet property.
        lines = FILES["renderer.rs"].splitlines()
        self.assertGreaterEqual(len(lines), 900)
        self.assertIn("paint_widget_twice", lines[902])


if __name__ == "__main__":
    unittest.main()
