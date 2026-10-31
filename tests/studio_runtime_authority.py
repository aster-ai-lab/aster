"""Shrink-only authority boundaries for the Core/ACP/Studio single runtime."""

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STUDIO = ROOT / "apps/studio"
BASELINE = json.loads((Path(__file__).resolve().parent / "studio_runtime_authority.json").read_text())
RAW_CALL = re.compile(r"(?:\bclient|\.client)\s*\.\s*(?:request|notify)\s*\(")
PERMISSION_DEF = re.compile(r"\bfn\s+(?:permission_act|tobot_permission_act|is_observe_tool|is_mutation_signal|is_deck_tool)\b")
LEDGER = re.compile(r"\b(?:SessionExecution|TaskRun)\b|session-modes\.json")


def files(root, suffixes):
    return sorted(path for path in root.rglob("*") if path.is_file() and path.suffix in suffixes and "target" not in path.parts)


def production(path):
    return "test" not in path.name and "_e2e" not in path.name and "__tests__" not in path.parts and path.name != "fake_acp.rs"


def relative(path, root=STUDIO):
    return path.relative_to(root).as_posix()


def function_block(text, name):
    marker = f"fn {name}"
    start = text.find(marker)
    if start < 0:
        raise AssertionError(f"missing function {name}")
    brace = text.find("{", start + len(marker))
    if brace < 0:
        raise AssertionError(f"missing body {name}")
    depth = 0
    for index in range(brace, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[start : index + 1]
    raise AssertionError(f"unclosed function {name}")


def assert_files_shrunk(test, current, allowed, label):
    grown = sorted(set(current) - set(allowed))
    if grown:
        raise AssertionError(f"{label} grew: {', '.join(grown)}")
    shrunk = sorted(set(allowed) - set(current))
    if shrunk:
        print(f"{label} shrank; baseline ceiling unchanged: {', '.join(shrunk)}")


class StudioRuntimeAuthorityTest(unittest.TestCase):
    def test_raw_acp_calls_are_centralized(self):
        violations = []
        candidates = files(STUDIO / "src-tauri/src/agent_plane", {".rs"})
        candidates.append(STUDIO / "src-tauri/src/local_memory.rs")
        for path in candidates:
            if not production(path) or relative(path) in BASELINE["raw_request_allowed_files"]:
                continue
            if RAW_CALL.search(path.read_text(encoding="utf-8")):
                violations.append(relative(path))
        self.assertEqual([], violations, "raw ACP request/notify outside RuntimeClient/transport")

    def test_one_admitted_run_driver(self):
        driver = ROOT / BASELINE["shared_attempt_driver"]
        self.assertTrue(driver.is_file(), "shared admitted-Run driver is missing")
        definitions = []
        for path in files(ROOT / "crates/code/acp/src/agent", {".rs"}):
            if "async fn drive_attempt" in path.read_text(encoding="utf-8"):
                definitions.append(relative(path, ROOT))
        self.assertEqual([BASELINE["shared_attempt_driver"]], definitions)
        for entry in BASELINE["shared_attempt_callers"]:
            file_name, function = entry.rsplit(":", 1)
            text = (ROOT / file_name).read_text(encoding="utf-8")
            body = function_block(text, function)
            self.assertIn(".drive_attempt(", body, entry)
            self.assertNotIn("rx.recv()", body, f"independent receive loop in {entry}")
            self.assertNotIn("tokio::select!", body, f"independent cancel/deadline loop in {entry}")

    def test_plan_live_path_and_legacy_ceiling(self):
        stream = (STUDIO / "src-tauri/src/agent_plane/stream.rs").read_text(encoding="utf-8")
        self.assertIn('Some("plan")', stream)
        self.assertIn("data-agent-plan", stream)
        turn = (STUDIO / "src/renderer/pages/agents/taskConversation/turn.ts").read_text(encoding="utf-8")
        self.assertIn("agentPlanSteps", turn)
        legacy = [relative(path) for path in files(STUDIO / "src/renderer/pages/agents", {".ts", ".tsx"}) if production(path) and "legacyTaskPlanSteps" in path.read_text(encoding="utf-8")]
        assert_files_shrunk(self, legacy, BASELINE["plan_legacy_readers"], "Plan legacy readers")
        raw = [relative(path) for path in files(STUDIO / "src/renderer/pages/agents", {".ts", ".tsx"}) if production(path) and ("variant === 'TodoWrite'" in path.read_text(encoding="utf-8") or "variant: 'TodoWrite'" in path.read_text(encoding="utf-8"))]
        assert_files_shrunk(self, raw, BASELINE["plan_raw_variant_files"], "Plan raw variant readers")

    def test_permission_predecision_only_shrinks(self):
        current = [relative(path) for path in files(STUDIO / "src-tauri/src", {".rs"}) if production(path) and PERMISSION_DEF.search(path.read_text(encoding="utf-8"))]
        assert_files_shrunk(self, current, BASELINE["permission_predecision_allowed_files"], "permission pre-decision files")

    def test_alternate_execution_ledgers_only_shrink(self):
        current = [relative(path) for path in files(STUDIO / "src-tauri/src", {".rs"}) if production(path) and LEDGER.search(path.read_text(encoding="utf-8"))]
        assert_files_shrunk(self, current, BASELINE["alternate_ledger_allowed_files"], "alternate execution ledger files")

    def test_dynamic_catalog_replaces_static_execution_lists(self):
        capabilities = (STUDIO / "src/shared/ai/agentRuntimeCapabilities.ts").read_text(encoding="utf-8")
        aster = capabilities.split("'aster':", 1)[1]
        self.assertIn("permissionModes: []", aster)
        self.assertIn("slashCommands: []", aster)
        self.assertIn("builtinTools: () => []", aster)
        self.assertIn("current session's ACP catalog", aster)
        for path in files(STUDIO / "src/renderer/pages/agents", {".ts", ".tsx"}):
            if production(path):
                self.assertNotIn("ASTER_COMMANDS", path.read_text(encoding="utf-8"), relative(path))
        eligibility = (STUDIO / "src/renderer/pages/agents/agentModelEligibility.ts").read_text(encoding="utf-8")
        self.assertIn("isAsterModelInRuntimeCatalog", eligibility)
        self.assertIn("if (context.agentType === 'aster') return undefined", eligibility)
        client = (STUDIO / "src-tauri/src/agent_plane/runtime_client.rs").read_text(encoding="utf-8")
        self.assertIn("model_bindings", client)
        self.assertIn("session/tooling/v1", client)

    def test_runtime_client_owns_no_product_state(self):
        text = (STUDIO / "src-tauri/src/agent_plane/runtime_client.rs").read_text(encoding="utf-8")
        for symbol in BASELINE["runtime_client_forbidden_symbols"]:
            self.assertNotIn(symbol, text, f"RuntimeClient owns product state: {symbol}")


if __name__ == "__main__":
    unittest.main()
