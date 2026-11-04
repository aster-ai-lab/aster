"""The benchmark must execute the selected ACP binary or fail explicitly."""

import subprocess
import sys
import tempfile
import unittest
import json
from pathlib import Path

from acp_engine import AcpEngine, EngineError

RUNNER = Path(__file__).with_name("run_quality.py")

PROTOCOL_FIXTURE = r'''
import json, sys
from pathlib import Path
mode = sys.argv[1]
count = 0
for line in sys.stdin:
    value = json.loads(line)
    with Path(__file__).with_suffix(".requests").open("a", encoding="utf-8") as log:
        log.write(line)
    if mode == "invalid-json":
        print("{broken", flush=True)
        break
    params = value["params"]
    if value["method"] == "initialize":
        result = {"protocolVersion": 1, "agentInfo": {"name": "transport-fixture"}}
    elif value["method"] == "session/new":
        result = {"sessionId": "fixture-session"}
    else:
        count += 1
        result = {"schema": params["schema"], "sessionId": params["sessionId"],
            "operationId": params["operationId"], "cwd": params["cwd"],
            "metadata": {"source_revision": 1, "results": [{"path": "evidence.rs", "start_line": 7,
                "end_line": 9, "digest_verified": True}],
                "fallback": None, "lexical_engine": "aster_vec_fts_v1",
                "semantic_status": {"phase": "ready"}}}
        if mode == "wrong-session":
            result["sessionId"] = "someone-else"
        if mode == "wrong-root":
            result["cwd"] = str(Path(params["cwd"]).parent)
        if mode == "invalid-hit":
            result["metadata"]["results"][0]["digest_verified"] = False
        if mode == "building" and count == 1:
            result["metadata"]["semantic_status"]["phase"] = "building"
            result["metadata"]["fallback"] = "building"
        if mode == "structural-building":
            result["metadata"]["fallback"] = "building"
            result["metadata"]["channels"] = [{"channel": "structural", "fallback": "building"}]
        if mode == "lexical-building" and count == 1:
            result["metadata"]["lexical_engine"] = "portable_bm25_v1"
        if mode == "unknown-lexical":
            result["metadata"]["lexical_engine"] = "unregistered"
        if mode == "missing-lexical":
            del result["metadata"]["lexical_engine"]
    print(json.dumps({"jsonrpc": "2.0", "id": value["id"], "result": result}), flush=True)
'''


class EngineCliTests(unittest.TestCase):
    def run_engine(self, engine):
        with tempfile.TemporaryDirectory(prefix="aster retrieval 引擎 ") as directory:
            return subprocess.run(
                [sys.executable, str(RUNNER), "--engine", str(engine), "--corpus", directory],
                capture_output=True, text=True, encoding="utf-8", timeout=45,
            )

    def test_missing_engine_fails_instead_of_reporting_standin_results(self):
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / "missing-engine"
            result = self.run_engine(missing)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("engine failed", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_non_acp_executable_is_not_reported_as_the_product(self):
        result = self.run_engine(sys.executable)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("engine failed", result.stderr)
        self.assertEqual(result.stdout, "")


class AcpTransportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ACP 引擎 path ")
        self.root = Path(self.temporary.name)
        self.fixture = self.root / "protocol_fixture.py"
        self.fixture.write_text(PROTOCOL_FIXTURE, encoding="utf-8")

    def tearDown(self):
        self.temporary.cleanup()

    def engine(self, mode="valid", semantic=False):
        return AcpEngine([sys.executable, str(self.fixture), mode], self.root, semantic)

    def test_real_subprocess_receives_query_and_returns_its_own_anchor(self):
        with self.engine() as engine:
            self.assertEqual(engine.rank("中文 exact query"), ["evidence.rs:7"])
            self.assertEqual(engine.agent_info["name"], "transport-fixture")
            self.assertEqual(len(engine.observations), 1)
            home = engine.home.name
        requests = [json.loads(line) for line in self.fixture.with_suffix(".requests").read_text().splitlines()]
        self.assertEqual([r["method"] for r in requests], ["initialize", "session/new", "_aster/workspace/search/v1"])
        self.assertEqual(requests[-1]["params"]["query"], "中文 exact query")
        self.assertIsNotNone(engine.process.poll())
        self.assertFalse(engine.reader.is_alive())
        self.assertFalse(Path(home).exists())

    def test_wrong_identity_and_unverified_hits_refuse(self):
        for mode in ("wrong-session", "wrong-root", "invalid-hit"):
            with self.subTest(mode=mode), self.engine(mode) as engine:
                with self.assertRaises(EngineError):
                    engine.rank("query")

    def test_partial_observation_is_retained_before_ready_result(self):
        with self.engine("building", semantic=True) as engine:
            self.assertEqual(engine.rank("query"), ["evidence.rs:7"])
            self.assertEqual(len(engine.observations), 2)
            self.assertEqual(engine.observations[0]["metadata"]["semantic_status"]["phase"], "building")
            self.assertEqual(engine.observations[1]["metadata"]["semantic_status"]["phase"], "ready")

    def test_malformed_output_stops_and_reaps_the_child(self):
        engine = self.engine("invalid-json")
        with self.assertRaisesRegex(EngineError, "invalid ACP output"):
            with engine:
                self.fail("the invalid engine was admitted")
        self.assertIsNotNone(engine.process.poll())
        self.assertFalse(engine.reader.is_alive())

    def test_optional_structural_channel_does_not_block_ready_semantic(self):
        with self.engine("structural-building", semantic=True) as engine:
            self.assertEqual(engine.rank("query"), ["evidence.rs:7"])
            self.assertEqual(len(engine.observations), 1)
            self.assertEqual(engine.observations[0]["metadata"]["channels"][0]["fallback"], "building")

    def test_native_lexical_readiness_is_required_in_both_profiles(self):
        for semantic in (False, True):
            with self.subTest(semantic=semantic), self.engine("lexical-building", semantic=semantic) as engine:
                self.assertEqual(engine.rank("query"), ["evidence.rs:7"])
                self.assertEqual(len(engine.observations), 2)
                self.assertEqual([o["metadata"]["lexical_engine"] for o in engine.observations],
                                 ["portable_bm25_v1", "aster_vec_fts_v1"])

    def test_unknown_or_missing_lexical_engine_is_refused(self):
        for mode in ("unknown-lexical", "missing-lexical"):
            with self.subTest(mode=mode), self.assertRaisesRegex(EngineError, "lexical engine"):
                with self.engine(mode) as engine:
                    engine.rank("query")


if __name__ == "__main__":
    unittest.main()
