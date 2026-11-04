"""Quality benchmark: score the built engine (or a baseline) on frozen labels.

With --engine, the driver launches that ACP binary and calls its actual
workspace-search extension. The default mini-corpus stand-in only checks
the legacy harness. The frozen line labels and metric formulas stay fixed.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

from acp_engine import AcpEngine, EngineError
from metrics import mrr, ndcg_at_10, recall_at_k
from mini_corpus import FILES, QUERIES, write_corpus
from qualification import load_fixture, workspace_ranking, write_workspace


def obvious_ranker(query: str, corpus_dir: Path):
    """Hermetic stand-in engine: exact word overlap ranking (not the product)."""
    terms = [term.lower() for term in query.split()]
    scored = []
    for name, content in FILES.items():
        lines = content.splitlines()
        for index, line in enumerate(lines, start=1):
            lower = line.lower()
            overlap = sum(1 for term in terms if term in lower)
            if overlap:
                scored.append((overlap, f"{name}:{index}"))
    scored.sort(key=lambda pair: -pair[0])
    return [chunk for _, chunk in scored[:10]]


def ripgrep_baseline(query: str, corpus_dir: Path):
    """ripgrep: same queries, file:line keys; skipped loudly when absent."""
    try:
        out = subprocess.run(
            ["rg", "--no-heading", "-n", query, str(corpus_dir)],
            capture_output=True, text=True, timeout=10, check=False,
        )
    except FileNotFoundError:
        print("baseline=ripgrep SKIPPED (rg not on PATH)", file=sys.stderr)
        return None
    keys = []
    for line in out.stdout.splitlines():
        rel = line[len(str(corpus_dir)) + 1:].rsplit(":", 2)
        if len(rel) == 3:
            keys.append(f"{rel[0]}:{rel[1]}")
    return keys[:10]


def run_suite(name: str, rank_fn, corpus_dir: Path, queries=None):
    queries = QUERIES if queries is None else queries
    per_query = {}
    for query_id, spec in queries.items():
        ranking = rank_fn(spec["query"], corpus_dir)
        if ranking is None:
            return None
        per_query[query_id] = {"category": spec["category"], "ranking": ranking}
        if not spec["grades"]:
            per_query[query_id]["candidate_count"] = len(ranking)
            continue
        per_query[query_id]["ndcg@10"] = ndcg_at_10(spec["grades"], ranking)
        per_query[query_id]["mrr"] = mrr(spec["grades"], ranking)
        per_query[query_id]["recall@5"] = recall_at_k(spec["grades"], ranking, 5)
        per_query[query_id]["recall@10"] = recall_at_k(spec["grades"], ranking, 10)
    flat = {}
    positives = [q for q in per_query.values() if "ndcg@10" in q]
    for metric in ("ndcg@10", "mrr", "recall@5", "recall@10"):
        flat[metric] = round(
            sum(q[metric] for q in positives) / len(positives), 6
        )
    return {"suite": name, "per_query": per_query, "summary": flat}


def qualification_result(fixture, suite):
    queries = {q["id"]: q for q in fixture["queries"]}
    per_category = {}
    for category in sorted({q["category"] for q in suite["per_query"].values()}):
        samples = [q for q in suite["per_query"].values() if q["category"] == category and "ndcg@10" in q]
        per_category[category] = {
            metric: sum(q[metric] for q in samples) / len(samples)
            for metric in ("ndcg@10", "mrr", "recall@5", "recall@10")
        }
    symbols = [(query_id, q) for query_id, q in suite["per_query"].items() if q["category"] == "symbol"]
    gates = {
        "per_category_recall@10": all(q["recall@10"] >= 0.80 for q in per_category.values()),
        "overall_ndcg@10": suite["summary"]["ndcg@10"] >= 0.75,
        "exact_symbol_top1": all(q["ranking"] and q["ranking"][0] in queries[query_id]["grades"]
                                 for query_id, q in symbols),
    }
    pending = [{"id": q["id"], "owner": q["owner"], "status": "NOT_RUN"}
               for q in fixture["queries"] if q["id"] not in suite["per_query"]]
    passed = all(gates.values())
    return {
        "fixture_sha256": fixture["fixture_sha256"], "ranking_unit": fixture["ranking_unit"],
        "total_queries": len(queries), "executed_queries": len(suite["per_query"]),
        "workspace_quality_status": "PASS" if passed else "FAIL",
        "status": "FAIL" if not passed else "NOT_RUN" if pending else "PASS",
        "gates": gates, "per_category": per_category, "pending": pending,
        "attribution_failures": 0,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, help="path to the built aster-acp binary")
    parser.add_argument("--semantic", action="store_true", help="opt in to the bundled local semantic provider")
    parser.add_argument("--fixture", type=Path, help="frozen source-level qualification fixture (actual engine required)")
    parser.add_argument("--corpus", default="/tmp/aster-bench-corpus")
    parser.add_argument("--output", help="results json path (resume-friendly)")
    args = parser.parse_args()

    if (args.semantic or args.fixture) and not args.engine:
        parser.error("--semantic and --fixture require --engine")
    fixture = None
    corpus_dir = Path(args.corpus)
    try:
        if args.fixture:
            fixture = load_fixture(args.fixture)
            write_workspace(fixture, corpus_dir)
        else:
            corpus_dir.mkdir(parents=True, exist_ok=True)
            write_corpus(corpus_dir)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"fixture failed: {error}", file=sys.stderr)
        return 1

    suites = []
    identity = None
    if args.engine:
        engine = None
        try:
            with AcpEngine([str(args.engine.resolve())], corpus_dir, args.semantic) as engine:
                if fixture:
                    queries = {q["id"]: q for q in fixture["queries"] if q["owner"] == "workspace"}
                    rank = lambda query, _: workspace_ranking(fixture, engine.search(query))
                    suites.append(run_suite("aster-acp-hybrid", rank, corpus_dir, queries))
                else:
                    suites.append(run_suite("aster-acp-hybrid", engine.rank, corpus_dir))
                identity = engine.identity()
        except (EngineError, OSError, ValueError) as error:
            if engine is not None:
                failed = json.dumps({"execution_status": "FAIL", "error": str(error),
                                     "engine_identity": engine.identity(), "qualification": {"status": "FAIL"}}, indent=1)
                if args.output:
                    Path(args.output).write_text(failed, encoding="utf-8")
                print(failed)
            print(f"engine failed: {error}", file=sys.stderr)
            return 1
    else:
        obvious = run_suite("obvious-standin", obvious_ranker, corpus_dir)
        if obvious:
            suites.append(obvious)
    rg = None if fixture else run_suite("ripgrep", ripgrep_baseline, corpus_dir)
    if rg:
        suites.append(rg)

    results = {
        "engine": str(args.engine) if args.engine else "hermetic-standin",
        "engine_identity": identity,
        "corpus": str(corpus_dir),
        "queries": len(fixture["queries"]) if fixture else len(QUERIES),
        "suites": suites,
        "qualification": qualification_result(fixture, suites[0]) if fixture else {
            "status": "NOT_RUN", "reason": "Legacy snippet-anchor measurements are execution smoke; use --fixture for source-level qualification."
        },
    }
    text = json.dumps(results, indent=1)
    if args.output:
        Path(args.output).write_text(text, encoding="utf-8")
    print(text)

    symbol = next(
        (s for s in suites if s["suite"] == "obvious-standin"), None
    )
    if symbol and symbol["per_query"]["symbol_lookup"]["ndcg@10"] < 1.0:
        print("FLOOR VIOLATED: symbol query did not rank the exact hit first", file=sys.stderr)
        return 1
    if results["qualification"]["status"] == "FAIL":
        print("QUALIFICATION FAILED: frozen quality gates were not met", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
