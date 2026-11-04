"""Check actual ACP phase attribution, cache counts and payload redaction."""

import argparse
import json
import math
import os
import shlex
import tempfile
from pathlib import Path

from acp_engine import AcpEngine, EngineError


SOURCE_SECRET = "SYNTHETIC_SOURCE_MUST_NOT_APPEAR_IN_PHASES"
QUERY_SECRET = "SYNTHETIC_QUERY_MUST_NOT_APPEAR_IN_PHASES"


def parse_phase(line):
    _, found, fields = line.partition("phase completed ")
    if not found:
        return None
    values = dict(token.split("=", 1) for token in shlex.split(fields))
    if "phase" not in values or "elapsed_ms" not in values:
        raise ValueError("phase sample omitted its identity or elapsed time")
    elapsed = float(values["elapsed_ms"])
    if not math.isfinite(elapsed) or elapsed < 0:
        raise ValueError("invalid phase elapsed time")
    values["elapsed_ms"] = elapsed
    return values


def check_profile(binary, semantic):
    with tempfile.TemporaryDirectory(prefix="ACP phase 中文 ") as directory:
        root = Path(directory)
        corpus = root / "workspace with spaces"
        corpus.mkdir()
        source = f"pub fn phase_probe() {{ /* {SOURCE_SECRET} */ }}\n".encode("utf-8")
        (corpus / "source.rs").write_bytes(source)
        evidence = os.environ.get("ASTER_RETRIEVAL_EVIDENCE_DIR")
        logs = (Path(evidence) / ("phase-c1-logs" if semantic else "phase-c0-logs")) if evidence else root / "logs"
        with AcpEngine([str(binary)], corpus, semantic, logs) as engine:
            sessions = [engine.session]
            assert engine.search("phase_probe " + QUERY_SECRET)
            second = engine.rpc("session/new", {"cwd": str(engine.corpus), "mcpServers": []})
            assert second["sessionId"] != engine.session
            engine.session = second["sessionId"]
            sessions.append(engine.session)
            assert engine.search("phase_probe " + QUERY_SECRET)
            try:
                engine.rpc("session/new", {"cwd": str(engine.corpus), "mcpServers": [],
                                          "_meta": {"sessionKind": "invalid-phase-fixture"}})
            except EngineError:
                pass
            else:
                raise AssertionError("the invalid session was admitted")
            identity = engine.identity()
        raw = b"".join(path.read_bytes() for path in sorted(logs.glob("aster-acp.*.log")))
        assert 0 < len(raw) <= 1024 * 1024, "missing/unbounded phase log"
        text = raw.decode("utf-8")
        for secret in (SOURCE_SECRET, QUERY_SECRET, "synthetic-not-a-credential"):
            assert secret not in text, "phase log exposed a sensitive payload"
        samples = [sample for line in text.splitlines() if (sample := parse_phase(line)) is not None]
        by_phase = {}
        for sample in samples:
            by_phase.setdefault(sample["phase"], []).append(sample)
        required = {"config_load", "stdio_ready", "initialize", "core_agent", "session_embedding",
                    "core_session_build", "core_session", "new_session", "workspace_search",
                    "catalog_admission", "lexical_query", "candidate_channels", "fusion_rerank",
                    "source_verification", "catalog_read", "catalog_build_queue", "catalog_build_compute"}
        assert required <= by_phase.keys(), f"missing actual phases: {required - by_phase.keys()}"
        for phase in ("config_load", "stdio_ready", "initialize"):
            assert len(by_phase[phase]) == 1, phase
        assert by_phase["config_load"][0]["success"] == "true"
        reuse = [r["reused"] for r in by_phase["core_agent"]]
        assert reuse[0] == "false" and reuse.count("false") == 1 and "true" in reuse
        for phase in ("session_embedding", "core_session_build", "core_session", "new_session"):
            assert [r["session_id"] for r in by_phase[phase]] == sessions, phase
        # Invalid admission has no completed-session sample; it is not a zero-ms session.
        assert len(by_phase["new_session"]) == 2
        order = [s["phase"] for s in samples]
        assert order.index("config_load") < order.index("stdio_ready") < order.index("initialize")
        assert order.index("core_agent") < order.index("core_session_build") < order.index("new_session")
        operations = [r["operation_id"] for r in identity["observations"]]
        assert [r["operation_id"] for r in by_phase["workspace_search"]] == operations
        assert len(by_phase["source_verification"]) == len(operations)
        for sample in by_phase["source_verification"]:
            assert int(sample["read_files"]) == 1
            assert int(sample["read_bytes"]) == len(source)
            assert int(sample["failed_reads"]) == 0
            assert int(sample["returned_hits"]) == 1
        query_phases = {"catalog_admission", "lexical_query", "candidate_channels", "fusion_rerank", "source_verification"}
        for line in text.splitlines():
            sample = parse_phase(line)
            if sample is not None and sample["phase"] in query_phases:
                assert any(operation in line for operation in operations), "query phase lost its request owner"
            if sample is not None and sample["phase"] in {"catalog_read", "catalog_build_queue", "catalog_build_compute"}:
                assert any(session in line for session in sessions), "background catalog lost its session owner"
        loads = by_phase.get("bundled_model_load", [])
        assert len(loads) == int(semantic), "model decode count disagrees with the profile"
        if semantic:
            assert loads[0]["success"] == "true"
            for observation in identity["observations"]:
                status = observation["metadata"]["semantic_status"]
                if status["phase"] == "ready":
                    assert status["indexedFiles"] == 1
                    assert status["coverageBps"] == 10000
            assert by_phase["embedding_request"]
        return {"profile": "C1" if semantic else "C0", "status": "PASS",
                "binary_sha256": identity["binary_sha256"], "sessions": sessions,
                "requests": len(operations), "source_bytes": len(source), "phase_samples": samples}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, type=Path)
    args = parser.parse_args()
    assert parse_phase("INFO target: phase completed phase=probe elapsed_ms=0")["elapsed_ms"] == 0
    try:
        parse_phase("INFO target: phase completed phase=missing")
    except ValueError:
        pass
    else:
        raise AssertionError("a missing duration was coerced to zero")
    reports = [check_profile(args.engine.resolve(), semantic) for semantic in (False, True)]
    print(json.dumps({"scope": "actual-process-phase-contract", "profiles": reports}, indent=1))


if __name__ == "__main__":
    main()
