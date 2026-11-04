"""Metric/attribution controls only; synthetic rankings are not product evidence."""

import copy
import unittest
from pathlib import Path

from memory_quality import BUNDLED_PROVIDER, LINEAR, RRF, evaluate
from qualification import load_fixture


FIXTURE = Path(__file__).resolve().parents[2] / "crates/code/acp/tests/fixtures/retrieval_qualification_v1.json"


class MemoryQualityTests(unittest.TestCase):
    def setUp(self):
        self.fixture = load_fixture(FIXTURE)
        documents = [d for d in self.fixture["documents"] if d["owner"] == "memory"]
        by_id = {d["id"]: d for d in documents}
        queries = {}
        for query in self.fixture["queries"]:
            if query["owner"] == "memory":
                queries[query["id"]] = {"query": query["query"], "hits": [
                    {"nodeId": source_id, "nodeRevision": 1, "score": 1.0,
                     "content": by_id[source_id]["content"]}
                    for source_id in query["grades"]]}
        self.raw = {
            "schema_version": 1, "owner": "core-v2-active-recall",
            "fixture_sha256": self.fixture["fixture_sha256"],
            "documents": documents, "provider": copy.deepcopy(BUNDLED_PROVIDER),
            "recall_policy": {"max_results": 12, "min_lexical_score": 0.0},
            "profiles": [{"binding": {"fusionProfile": name, "embedding": copy.deepcopy(BUNDLED_PROVIDER),
                          "policy": {"candidateLimit": 10, "minScore": 0.0, **extra}},
                          "queries": copy.deepcopy(queries)}
                         for name, extra in [(RRF, {}), (LINEAR, {"linearFusion": True})]],
        }

    def test_primary_profile_is_fixed_and_partial_run_stays_partial(self):
        report = evaluate(self.fixture, self.raw)
        self.assertEqual(report["primary_profile"], LINEAR)
        self.assertEqual(report["memory_quality_status"], "PASS")
        self.assertEqual(report["full_60_query_status"], "NOT_RUN")
        self.assertEqual(report["executed_memory_queries_per_profile"], 10)
        self.assertEqual(report["profiles"][LINEAR]["summary"]["ndcg@10"], 1.0)
        for query in self.raw["profiles"][1]["queries"].values():
            query["hits"] = []
        report = evaluate(self.fixture, self.raw)
        self.assertEqual(report["memory_quality_status"], "FAIL")
        self.assertEqual(report["profiles"][RRF]["status"], "PASS")

    def test_corrupt_identity_coverage_policy_and_source_are_refused(self):
        for corruption in ("fixture", "owner", "model", "documents", "coverage", "query", "policy",
                           "profile", "revision", "boolean-revision", "content", "foreign", "duplicate", "score", "order", "limit"):
            raw = copy.deepcopy(self.raw)
            profile = raw["profiles"][1]
            observed = next(iter(profile["queries"].values()))
            hit = observed["hits"][0]
            if corruption == "fixture": raw["fixture_sha256"] = "changed"
            elif corruption == "owner": raw["owner"] = "workspace"
            elif corruption == "model": raw["provider"]["revision"] = "changed"
            elif corruption == "documents": raw["documents"][0]["content"] += "changed"
            elif corruption == "coverage": profile["queries"].pop(next(iter(profile["queries"])))
            elif corruption == "query": observed["query"] += "changed"
            elif corruption == "policy": profile["binding"]["policy"]["minScore"] = 0.5
            elif corruption == "profile": profile["binding"]["fusionProfile"] = RRF
            elif corruption == "revision": hit["nodeRevision"] = 2
            elif corruption == "boolean-revision": hit["nodeRevision"] = True
            elif corruption == "content": hit["content"] += "changed"
            elif corruption == "foreign": hit["nodeId"] = "outside"
            elif corruption == "duplicate": observed["hits"].append(copy.deepcopy(hit))
            elif corruption == "score": hit["score"] = float("nan")
            elif corruption == "order":
                hit["score"] = 0.0
                document = next(d for d in raw["documents"] if d["id"] != hit["nodeId"])
                observed["hits"].append({"nodeId": document["id"], "nodeRevision": 1,
                                         "content": document["content"], "score": 0.5})
            else: raw["recall_policy"]["max_results"] = 100
            with self.subTest(corruption=corruption), self.assertRaises(ValueError):
                evaluate(self.fixture, raw)


if __name__ == "__main__":
    unittest.main()
