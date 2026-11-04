"""Qualification input and attribution checks, independent of search ranking."""

import copy
import hashlib
import json
import tempfile
import unittest
from collections import Counter
from pathlib import Path

from qualification import load_fixture, workspace_ranking, write_workspace
from run_quality import qualification_result, run_suite


FIXTURE = Path(__file__).resolve().parents[2] / "crates/code/acp/tests/fixtures/retrieval_qualification_v1.json"
FROZEN_SHA = "9dd4e0b0d6d17a91aba31932e346c9bf72cee69e8a0e65cf242f5cfe154d8b19"


class QualificationTests(unittest.TestCase):
    def setUp(self):
        self.fixture = load_fixture(FIXTURE)

    def test_frozen_source_and_query_counts(self):
        self.assertEqual(self.fixture["fixture_sha256"], FROZEN_SHA)
        self.assertEqual(len(self.fixture["documents"]), 56)
        self.assertEqual(len(self.fixture["queries"]), 60)
        self.assertEqual(Counter(q["category"] for q in self.fixture["queries"]),
                         dict.fromkeys(("symbol", "behavior", "architecture", "cjk", "memory", "sources"), 10))
        self.assertEqual(sum(q["negative"] for q in self.fixture["queries"]), 4)

    def test_bad_labels_and_source_bytes_are_rejected(self):
        for corruption in ("digest", "span", "owner", "path", "category", "duplicate", "negative"):
            fixture = copy.deepcopy(self.fixture)
            if corruption == "digest":
                fixture["documents"][0]["content"] += "changed\n"
            elif corruption == "span":
                fixture["queries"][0]["evidence"][0]["start_line"] = 100
            elif corruption == "owner":
                fixture["queries"][0]["owner"] = "memory"
            elif corruption == "path":
                fixture["documents"][0]["path"] = "../outside.rs"
            elif corruption == "category":
                fixture["queries"][0]["category"] = "unregistered"
            elif corruption == "duplicate":
                fixture["queries"][1]["id"] = fixture["queries"][0]["id"]
            else:
                fixture["queries"][0]["negative"] = True
            with self.subTest(corruption=corruption), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "invalid.json"
                path.write_text(json.dumps(fixture), encoding="utf-8")
                with self.assertRaises(ValueError):
                    load_fixture(path)

    def test_corpus_contains_only_actual_workspace_sources_and_preserves_bytes(self):
        with tempfile.TemporaryDirectory(prefix="qualification 中文 ") as directory:
            root = Path(directory)
            write_workspace(self.fixture, root)
            paths = {str(p.relative_to(root).as_posix()) for p in root.rglob("*") if p.is_file()}
            expected = {d["path"] for d in self.fixture["documents"] if d["owner"] == "workspace"}
            self.assertEqual(paths, expected)
            self.assertEqual(len(paths), 40)
            for document in self.fixture["documents"]:
                if document["owner"] == "workspace":
                    self.assertEqual(hashlib.sha256((root / document["path"]).read_bytes()).hexdigest(), document["sha256"])
            with self.assertRaisesRegex(ValueError, "empty directory"):
                write_workspace(self.fixture, root)

    def test_normalization_deduplicates_returned_sources_without_gold_lookup(self):
        document = self.fixture["documents"][0]
        hit = {"path": document["path"], "start_line": 2, "end_line": 2,
               "content_digest": "sha256:" + document["sha256"],
               "digest_verified": True, "snippet_truncated": False,
               "text": "\n     2 | pub fn purge_expired_sessions() {}"}
        self.fixture["queries"] = []
        self.assertEqual(workspace_ranking(self.fixture, [hit, hit]), [document["id"]])
        cropped = dict(hit, snippet_truncated=True, text="\n     2 | ...purge_expired_sessions() {}")
        self.assertEqual(workspace_ranking(self.fixture, [cropped]), [document["id"]])
        for field, value in (("content_digest", "sha256:stale"), ("end_line", 90),
                             ("text", "\n     2 | invented source")):
            with self.subTest(field=field), self.assertRaises(ValueError):
                workspace_ranking(self.fixture, [dict(hit, **{field: value})])

    def test_quality_gates_do_not_promote_a_partial_run_to_full_qualification(self):
        queries = {q["id"]: q for q in self.fixture["queries"] if q["owner"] == "workspace"}
        # Perfect rankings are a metric control, never a product result.
        control = {q["query"]: list(q["grades"]) for q in queries.values()}
        suite = run_suite("metric-control", lambda query, _: control[query], None, queries)
        report = qualification_result(self.fixture, suite)
        self.assertEqual(report["executed_queries"], 40)
        self.assertEqual(len(report["pending"]), 20)
        self.assertEqual(report["workspace_quality_status"], "PASS")
        self.assertEqual(report["status"], "NOT_RUN")
        self.assertEqual(suite["summary"]["ndcg@10"], 1.0)
        control[queries[next(iter(queries))]["query"]] = []
        suite = run_suite("metric-control", lambda query, _: control[query], None, queries)
        report = qualification_result(self.fixture, suite)
        self.assertFalse(report["gates"]["exact_symbol_top1"])
        self.assertEqual(report["workspace_quality_status"], "FAIL")
        self.assertEqual(report["status"], "FAIL")

    def test_negative_queries_do_not_inflate_relevance_means(self):
        queries = {
            "positive": {"category": "sources", "query": "answer", "grades": {"source": 3}},
            "negative": {"category": "sources", "query": "absent", "grades": {}},
        }
        suite = run_suite("metric-control", lambda query, _: ["source"] if query == "answer" else [], None, queries)
        self.assertEqual(suite["summary"]["ndcg@10"], 1.0)
        self.assertNotIn("ndcg@10", suite["per_query"]["negative"])
        self.assertEqual(suite["per_query"]["negative"]["candidate_count"], 0)


if __name__ == "__main__":
    unittest.main()
