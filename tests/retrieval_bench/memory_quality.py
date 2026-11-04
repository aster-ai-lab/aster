"""Grade actual Core ActiveRecall output with the existing frozen metrics."""

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

from metrics import mrr, ndcg_at_10, recall_at_k
from qualification import load_fixture


RRF = "aster.code.memory.hybrid.rrf-k60.v1"
LINEAR = "aster.code.memory.hybrid.linear-unit.v1"
BUNDLED_PROVIDER = {
    "dimension": 256, "model": "potion-code-16M-v2", "normalization": "unit",
    "provider": "light",
    "revision": "sha256:2320c68f3da5ca9eb8ed56b2354d1cc85d47849be77aa4876ab569e352bade3e",
}


def evaluate(fixture, raw):
    if (raw["schema_version"] != 1 or raw["owner"] != "core-v2-active-recall"
            or raw["fixture_sha256"] != fixture["fixture_sha256"]):
        raise ValueError("memory owner/fixture identity mismatch")
    documents = [d for d in fixture["documents"] if d["owner"] == "memory"]
    if raw["documents"] != documents:
        raise ValueError("memory source records differ from the frozen fixture")
    by_id = {d["id"]: d for d in documents}
    if raw["provider"] != BUNDLED_PROVIDER:
        raise ValueError("bundled provider differs from the fixed model generation")
    queries = {q["id"]: q for q in fixture["queries"] if q["owner"] == "memory"}
    limit = fixture["output_limits"]["host_hits"]
    if raw["recall_policy"] != {"max_results": limit, "min_lexical_score": 0.0}:
        raise ValueError("memory recall limits differ")
    profiles = {}
    for profile in raw["profiles"]:
        binding = profile["binding"]
        name = binding["fusionProfile"]
        if name not in (RRF, LINEAR) or name in profiles:
            raise ValueError("unknown or duplicate memory profile")
        expected_policy = {"candidateLimit": 10, "minScore": 0.0}
        if name == LINEAR:
            expected_policy["linearFusion"] = True
        if binding["policy"] != expected_policy or binding["embedding"] != raw["provider"]:
            raise ValueError("memory policy/model mismatch")
        if set(profile["queries"]) != set(queries):
            raise ValueError("memory query coverage differs")
        per_query = {}
        for query_id, observed in profile["queries"].items():
            query = queries[query_id]
            if observed["query"] != query["query"]:
                raise ValueError("memory query text differs")
            hits = observed["hits"]
            if len(hits) > limit or len(json.dumps(hits, ensure_ascii=False).encode("utf-8")) > fixture["output_limits"]["host_bytes"]:
                raise ValueError("memory output exceeds frozen limits")
            ranking = []
            previous_score = math.inf
            for hit in hits:
                document = by_id.get(hit["nodeId"])
                if (document is None or hit["nodeId"] in ranking
                        or type(hit["nodeRevision"]) is not int or hit["nodeRevision"] != 1
                        or hit["content"] != document["content"]
                        or hashlib.sha256(hit["content"].encode("utf-8")).hexdigest() != document["sha256"]):
                    raise ValueError("memory source attribution/revision mismatch")
                score = hit["score"]
                if type(score) not in (int, float) or not math.isfinite(score) or not 0 <= score <= 1 or score > previous_score:
                    raise ValueError("invalid memory score/order")
                previous_score = score
                ranking.append(hit["nodeId"])
            per_query[query_id] = {
                "ranking": ranking, "ndcg@10": ndcg_at_10(query["grades"], ranking),
                "mrr": mrr(query["grades"], ranking),
                "recall@5": recall_at_k(query["grades"], ranking, 5),
                "recall@10": recall_at_k(query["grades"], ranking, 10),
            }
        summary = {metric: sum(q[metric] for q in per_query.values()) / len(per_query)
                   for metric in ("ndcg@10", "mrr", "recall@5", "recall@10")}
        gates = {"memory_recall@10": summary["recall@10"] >= 0.80,
                 "memory_ndcg@10": summary["ndcg@10"] >= 0.75}
        profiles[name] = {"per_query": per_query, "summary": summary, "gates": gates,
                          "status": "PASS" if all(gates.values()) else "FAIL"}
    if set(profiles) != {RRF, LINEAR}:
        raise ValueError("both fixed memory profiles are required")
    return {"schema_version": 1, "owner": raw["owner"], "fixture_sha256": raw["fixture_sha256"],
            "primary_profile": LINEAR, "profiles": profiles,
            "executed_memory_queries_per_profile": len(queries),
            "memory_quality_status": profiles[LINEAR]["status"],
            "full_60_query_status": "NOT_RUN", "attribution_failures": 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--raw", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = evaluate(load_fixture(args.fixture), json.loads(args.raw.read_text(encoding="utf-8")))
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"memory qualification refused: {error}", file=sys.stderr)
        return 1
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"memory_quality_status": result["memory_quality_status"],
                      "profiles": {name: profile["summary"] for name, profile in result["profiles"].items()}}))
    return 0 if result["memory_quality_status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
