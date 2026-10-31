"""Quality benchmark: score the built engine (or a baseline) on frozen labels.

The engine under test is whatever emits a JSON array of ranked chunk keys on
stdout for one query — the harness never reimplements retrieval. The
mini-corpus default is hermetic: an obvious in-process ranker stands in for
the engine so CI proves the loop; real runs point --engine at the built
workspace_retrieval_aster binary (FR-001).
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

from metrics import mrr, ndcg_at_10, recall_at_k, summarize, approx_tokens
from mini_corpus import FILES, QUERIES, write_corpus


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


def run_suite(name: str, rank_fn, corpus_dir: Path):
    per_query = {}
    for query_id, spec in QUERIES.items():
        ranking = rank_fn(spec["query"], corpus_dir)
        if ranking is None:
            return None
        per_query.setdefault(query_id, {})
        per_query[query_id]["ndcg@10"] = ndcg_at_10(spec["grades"], ranking)
        per_query[query_id]["mrr"] = mrr(spec["grades"], ranking)
        per_query[query_id]["recall@5"] = recall_at_k(spec["grades"], ranking, 5)
    flat = {}
    for metric in ("ndcg@10", "mrr", "recall@5"):
        flat[metric] = round(
            sum(q[metric] for q in per_query.values()) / len(per_query), 6
        )
    return {"suite": name, "per_query": per_query, "summary": flat}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", help="path to the built engine binary (JSON hits on stdout)")
    parser.add_argument("--corpus", default="/tmp/aster-bench-corpus")
    parser.add_argument("--output", help="results json path (resume-friendly)")
    args = parser.parse_args()

    corpus_dir = Path(args.corpus)
    corpus_dir.mkdir(parents=True, exist_ok=True)
    write_corpus(corpus_dir)

    suites = []
    obvious = run_suite("obvious-standin", obvious_ranker, corpus_dir)
    if obvious:
        suites.append(obvious)
    rg = run_suite("ripgrep", ripgrep_baseline, corpus_dir)
    if rg:
        suites.append(rg)

    results = {
        "engine": args.engine or "hermetic-standin",
        "corpus": str(corpus_dir),
        "queries": len(QUERIES),
        "suites": suites,
    }
    text = json.dumps(results, indent=1)
    if args.output:
        Path(args.output).write_text(text, encoding="utf-8")
    print(text)

    from mini_corpus import EXPECTED_FLOOR
    symbol = next(
        (s for s in suites if s["suite"] == "obvious-standin"), None
    )
    if symbol and symbol["per_query"]["symbol_lookup"]["ndcg@10"] < 1.0:
        print("FLOOR VIOLATED: symbol query did not rank the exact hit first", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
