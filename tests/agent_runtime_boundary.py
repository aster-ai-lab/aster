"""Closed sets for the Aster terminal and a3s-code-acp boundary.

Growth fails. Shrink prints that the baseline ceiling is unchanged.
The script has no switch that raises a ceiling.
"""

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASELINE_PATH = Path(__file__).resolve().parent / "agent_runtime_boundary.json"

ACP_CARGO = ROOT / "crates/code/acp/Cargo.toml"
ACP_LIB = ROOT / "crates/code/acp/src/lib.rs"
ASTER_CARGO = ROOT / "apps/aster/Cargo.toml"
ASTER_MAIN = ROOT / "apps/aster/src/main.rs"
CONFIG_LOADER = (
    ROOT
    / "crates/terminal/crates/codegen/a3s-code-config/src/loader.rs"
)
EVENT_LOOP = (
    ROOT
    / "crates/terminal/crates/codegen/a3s-code-pager/src/app/event_loop.rs"
)
PAGER_SRC = ROOT / "crates/terminal/crates/codegen/a3s-code-pager/src"
PAGER_ACP = PAGER_SRC / "acp/mod.rs"
PAGER_RUNTIME = PAGER_SRC / "agent_runtime.rs"
PAGER_SPAWN = PAGER_SRC / "acp/a3s_spawn.rs"
PAGER_LEADER = PAGER_SRC / "app/leader_cluster/mod.rs"
SPAWN_DEFINITION = "acp/spawn.rs"
SIDECAR_GATE = "pub async fn spawn_required_sidecar("

CONST_VALUE = re.compile(r'^pub const [A-Z0-9_]+: &str = "([^"]*)";$')
PATH_VALUE = re.compile(r'path = "([^"]*)"')


class BoundaryError(Exception):
    """A closed set grew or a required floor disappeared."""


def load_baseline(text):
    data = json.loads(text)
    required = (
        "acp_description",
        "acp_crate_doc",
        "acp_terminal_paths",
        "aster_pager_deps",
        "config_filenames",
        "main_allow",
        "main_identity_ceilings",
        "event_loop_inline_tests_max_lines",
        "acp_account_mods",
        "acp_default_features",
        "pager_shell_spawn_files",
        "pager_stdio_shell_needles",
        "pager_stdio_forbidden",
        "pager_interactive_sidecar",
        "pager_sidecar_entries",
        "pager_acp_stale_version_max",
    )
    missing = [key for key in required if key not in data]
    if missing:
        raise BoundaryError("baseline missing: " + ", ".join(missing))
    return data


def shrink_only(current, baseline, label):
    grown = sorted(set(current) - set(baseline))
    if grown:
        raise BoundaryError(label + " grew: " + ", ".join(grown))
    shrunk = sorted(set(baseline) - set(current))
    if shrunk:
        print(label + " shrank; baseline ceiling unchanged: " + ", ".join(shrunk))


def ceiling(current, limit, label):
    if current > limit:
        raise BoundaryError(f"{label} grew: {current} > {limit}")
    if current < limit:
        print(f"{label} shrank; baseline ceiling unchanged: {current} < {limit}")


def floor(current, required, label):
    missing = sorted(set(required) - set(current))
    if missing:
        raise BoundaryError(label + " missing: " + ", ".join(missing))


def package_description(text):
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("description = \""):
            if not stripped.endswith("\""):
                raise BoundaryError("description is not one quoted line")
            return stripped[len("description = \"") : -1]
    raise BoundaryError("description is absent")


def crate_doc(text, count):
    lines = text.splitlines()
    if len(lines) < count:
        raise BoundaryError("crate doc is shorter than the baseline")
    return lines[:count]


def table_keys(text, table_name):
    keys = []
    in_table = False
    header = "[" + table_name + "]"
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_table = stripped == header
            continue
        if not in_table or not stripped or stripped.startswith("#"):
            continue
        if "=" not in stripped:
            continue
        key = stripped.split("=", 1)[0].strip()
        if key:
            keys.append(key)
    return keys


def terminal_paths(text):
    paths = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("#"):
            continue
        for match in PATH_VALUE.finditer(stripped):
            path = match.group(1)
            if "terminal/" in path:
                paths.append(path)
    return paths


def const_strings(text):
    values = []
    for line in text.splitlines():
        match = CONST_VALUE.match(line.strip())
        if match:
            values.append(match.group(1))
    return values


def crate_allow_names(text):
    """Names in a leading crate allow. No allow attribute is an empty set."""
    lines = text.splitlines()
    index = 0
    while index < len(lines) and not lines[index].strip():
        index += 1
    body = None
    while index < len(lines):
        stripped = lines[index].strip()
        if not stripped.startswith("#!["):
            break
        if stripped.startswith("#![allow("):
            chunk = [stripped[len("#![allow(") :]]
            if ")]" not in stripped:
                index += 1
                while index < len(lines):
                    chunk.append(lines[index])
                    if ")]" in lines[index]:
                        break
                    index += 1
                else:
                    raise BoundaryError("crate allow attribute does not close")
            body = "\n".join(chunk)
            end = body.find(")]")
            if end < 0:
                raise BoundaryError("crate allow attribute does not close")
            body = body[:end]
            break
        if not stripped.endswith("]"):
            raise BoundaryError("crate attribute does not close")
        index += 1
    if body is None:
        return []
    names = []
    for line in body.splitlines():
        name = line.strip().strip(",").strip()
        if name:
            names.append(name)
    return names


def needle_counts(text, needles):
    return {needle: text.count(needle) for needle in needles}


def inline_test_lines(text):
    lines = text.splitlines()
    hits = [index for index, line in enumerate(lines) if line == "mod tests {"]
    if len(hits) != 1:
        raise BoundaryError(f"mod tests {{ appears {len(hits)} times")
    return len(lines) - hits[0]


def account_mods(text):
    names = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("pub mod "):
            rest = stripped[len("pub mod ") :]
        elif stripped.startswith("mod "):
            rest = stripped[len("mod ") :]
        else:
            continue
        if not rest.endswith(";"):
            continue
        name = rest[:-1].strip()
        if name == "cc_switch" or name.endswith("_account"):
            names.append(name)
    return names


def check_description(text, expected):
    found = package_description(text)
    if found != expected:
        raise BoundaryError("description mismatch: " + found)


def check_crate_doc(text, expected):
    found = crate_doc(text, len(expected))
    if found != list(expected):
        raise BoundaryError("crate doc mismatch")


def check_terminal_paths(text, baseline):
    shrink_only(terminal_paths(text), baseline, "acp terminal paths")


def check_pager_deps(text, required):
    floor(table_keys(text, "dependencies"), required, "aster pager deps")


def check_config_filenames(text, baseline):
    shrink_only(const_strings(text), baseline, "config filenames")


def check_main_allow(text, baseline):
    shrink_only(crate_allow_names(text), baseline, "main allow")


def check_identity_ceilings(text, ceilings):
    counts = needle_counts(text, ceilings)
    for needle, limit in ceilings.items():
        ceiling(counts[needle], limit, "main identity " + needle)


def check_inline_tests(text, limit):
    ceiling(inline_test_lines(text), limit, "event_loop inline tests")


def check_account_mods(text, baseline):
    shrink_only(account_mods(text), baseline, "acp account mods")


FEATURE_GATES = {
    "mod artifact_image;": '#[cfg(feature = "svg-png")]',
    "mod cc_switch;": '#[cfg(feature = "account-import")]',
    "mod cursor_account;": '#[cfg(feature = "account-import")]',
    "mod grok_account;": '#[cfg(feature = "account-import")]',
}


def default_features(text):
    in_features = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_features = stripped == "[features]"
            continue
        if in_features and stripped.startswith("default "):
            return re.findall(r'"([^"]*)"', stripped)
    raise BoundaryError("features default is absent")


def check_default_features(text, expected):
    found = default_features(text)
    if set(found) != set(expected):
        raise BoundaryError("acp default features changed: " + ", ".join(found))


def check_optional_dependency(text, name):
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(name + " ") or stripped.startswith(name + "="):
            if "optional = true" not in stripped:
                raise BoundaryError(name + " is not optional")
            return
    raise BoundaryError(name + " dependency is absent")


def check_feature_gates(text):
    lines = text.splitlines()
    for index, line in enumerate(lines):
        stripped = line.strip()
        expected = FEATURE_GATES.get(stripped)
        if expected is None:
            continue
        previous = lines[index - 1].strip() if index else ""
        if previous != expected:
            raise BoundaryError(stripped + " is not behind " + expected)


def shell_spawn_calls_in(relative, text):
    """True when a non-comment line calls spawn_grok_shell. The definition file does not count."""
    if relative == SPAWN_DEFINITION:
        return False
    for line in text.splitlines():
        stripped = line.strip()
        if (
            stripped.startswith("//")
            or stripped.startswith("///")
            or stripped.startswith("*")
        ):
            continue
        if "spawn_grok_shell(" in stripped:
            return True
    return False


def shell_spawn_call_files(root):
    found = []
    for path in sorted(root.rglob("*.rs")):
        relative = path.relative_to(root).as_posix()
        if shell_spawn_calls_in(relative, path.read_text(encoding="utf-8")):
            found.append(relative)
    return found


def check_shell_spawn_files(root, baseline):
    shrink_only(shell_spawn_call_files(root), baseline, "pager shell spawn")


def check_stdio_runtime(text, present, forbidden):
    problems = []
    missing = [needle for needle in present if needle not in text]
    if missing:
        problems.append("stdio sidecar missing: " + ", ".join(missing))
    found = [needle for needle in forbidden if needle in text]
    if found:
        problems.append("stdio shell present: " + ", ".join(found))
    if problems:
        raise BoundaryError("; ".join(problems))


def function_body(text, signature):
    """Slice one function. The file defines spawn_a3s_acp before the gate."""
    start = text.find(signature)
    if start < 0:
        raise BoundaryError("missing " + signature)
    rest = text[start:]
    next_item = rest.find("\npub ", len(signature))
    if next_item < 0:
        return rest
    return rest[:next_item]


def check_sidecar_gate(text, needles):
    body = function_body(text, SIDECAR_GATE)
    positions = []
    for needle in needles:
        index = body.find(needle)
        if index < 0:
            raise BoundaryError("sidecar gate missing: " + needle)
        positions.append(index)
    if positions != sorted(positions):
        raise BoundaryError("sidecar gate order changed")


def check_sidecar_entries(root, entries):
    if not isinstance(entries, dict) or not entries:
        raise BoundaryError("pager sidecar entries must be a non-empty object")
    for relative, needle in entries.items():
        path = root / relative
        if not path.is_file():
            raise BoundaryError("sidecar entry missing file: " + relative)
        text = path.read_text(encoding="utf-8")
        if needle not in text:
            raise BoundaryError(relative + " missing sidecar call")


def check_leader_in_process(text):
    if "MvpAgent" not in text:
        raise BoundaryError("leader cluster lost MvpAgent")
    if "spawn_required_sidecar(" in text or "spawn_a3s_acp(" in text:
        raise BoundaryError("leader cluster left the in-process agent")


def check_stale_core_version(text, limit):
    ceiling(text.count("9.0.0"), limit, "pager acp 9.0.0")


def check_repository(baseline):
    check_description(ACP_CARGO.read_text(encoding="utf-8"), baseline["acp_description"])
    check_crate_doc(ACP_LIB.read_text(encoding="utf-8"), baseline["acp_crate_doc"])
    check_terminal_paths(
        ACP_CARGO.read_text(encoding="utf-8"), baseline["acp_terminal_paths"]
    )
    check_pager_deps(ASTER_CARGO.read_text(encoding="utf-8"), baseline["aster_pager_deps"])
    check_config_filenames(
        CONFIG_LOADER.read_text(encoding="utf-8"), baseline["config_filenames"]
    )
    main = ASTER_MAIN.read_text(encoding="utf-8")
    check_main_allow(main, baseline["main_allow"])
    check_identity_ceilings(main, baseline["main_identity_ceilings"])
    check_inline_tests(
        EVENT_LOOP.read_text(encoding="utf-8"),
        baseline["event_loop_inline_tests_max_lines"],
    )
    lib = ACP_LIB.read_text(encoding="utf-8")
    cargo = ACP_CARGO.read_text(encoding="utf-8")
    check_account_mods(lib, baseline["acp_account_mods"])
    check_feature_gates(lib)
    check_default_features(cargo, baseline["acp_default_features"])
    for name in ("a3s-code-mermaid", "rusqlite", "quick-xml"):
        check_optional_dependency(cargo, name)
    check_shell_spawn_files(PAGER_SRC, baseline["pager_shell_spawn_files"])
    check_stdio_runtime(
        PAGER_RUNTIME.read_text(encoding="utf-8"),
        baseline["pager_stdio_shell_needles"],
        baseline["pager_stdio_forbidden"],
    )
    check_sidecar_gate(
        PAGER_SPAWN.read_text(encoding="utf-8"),
        baseline["pager_interactive_sidecar"],
    )
    check_sidecar_entries(PAGER_SRC, baseline["pager_sidecar_entries"])
    check_leader_in_process(PAGER_LEADER.read_text(encoding="utf-8"))
    acp_mod = PAGER_ACP.read_text(encoding="utf-8")
    check_stale_core_version(acp_mod, baseline["pager_acp_stale_version_max"])


class AgentRuntimeBoundaryTest(unittest.TestCase):
    def setUp(self):
        self.baseline = load_baseline(BASELINE_PATH.read_text(encoding="utf-8"))

    def test_repository_matches_the_closed_sets(self):
        check_repository(self.baseline)

    def test_description_growth_is_rejected(self):
        text = 'description = "ACP stdio agent bridging the forked grok-build pager"\n'
        with self.assertRaises(BoundaryError) as caught:
            check_description(text, self.baseline["acp_description"])
        self.assertIn("description mismatch", str(caught.exception))

    def test_crate_doc_growth_is_rejected(self):
        text = "\n".join(self.baseline["acp_crate_doc"]) + "\n"
        text = text.replace("9.1.4", "9.0.0", 1)
        with self.assertRaises(BoundaryError) as caught:
            check_crate_doc(text, self.baseline["acp_crate_doc"])
        self.assertIn("crate doc mismatch", str(caught.exception))

    def test_terminal_path_growth_is_rejected(self):
        text = (
            'a3s-code-mermaid = {path = "../../terminal/crates/codegen/a3s-code-mermaid"}\n'
            'a3s-code-pager = {path = "../../terminal/crates/codegen/a3s-code-pager"}\n'
        )
        with self.assertRaises(BoundaryError) as caught:
            check_terminal_paths(text, self.baseline["acp_terminal_paths"])
        self.assertIn("a3s-code-pager", str(caught.exception))

    def test_missing_pager_dep_is_rejected(self):
        text = "[dependencies]\na3s-code-pager = {path = \"x\"}\n"
        with self.assertRaises(BoundaryError) as caught:
            check_pager_deps(text, self.baseline["aster_pager_deps"])
        self.assertIn("a3s-code-shell", str(caught.exception))

    def test_config_filename_growth_is_rejected(self):
        text = 'pub const EXTRA_FILENAME: &str = "extra.toml";\n'
        with self.assertRaises(BoundaryError) as caught:
            check_config_filenames(text, self.baseline["config_filenames"])
        self.assertIn("extra.toml", str(caught.exception))

    def test_allow_growth_is_rejected(self):
        text = "#![allow(\n    dead_code,\n    unsafe_code\n)]\n"
        with self.assertRaises(BoundaryError) as caught:
            check_main_allow(text, self.baseline["main_allow"])
        self.assertIn("unsafe_code", str(caught.exception))

    def test_missing_allow_is_an_empty_set(self):
        text = "#![deny(clippy::indexing_slicing)]\nfn main() {}\n"
        self.assertEqual(crate_allow_names(text), [])

    def test_allow_after_deny_still_counts(self):
        text = (
            "#![deny(clippy::indexing_slicing)]\n"
            "#![allow(\n    dead_code,\n)]\n"
            "fn main() {}\n"
        )
        with self.assertRaises(BoundaryError) as caught:
            check_main_allow(text, [])
        self.assertIn("dead_code", str(caught.exception))

    def test_identity_needle_growth_is_rejected(self):
        text = "grok: grok:\n"
        ceilings = {"grok:": 1}
        with self.assertRaises(BoundaryError) as caught:
            check_identity_ceilings(text, ceilings)
        self.assertIn("grok:", str(caught.exception))

    def test_second_inline_test_module_is_rejected(self):
        text = "fn main() {}\nmod tests {\n}\nmod tests {\n}\n"
        with self.assertRaises(BoundaryError) as caught:
            check_inline_tests(text, 3)
        self.assertIn("appears 2 times", str(caught.exception))

    def test_inline_test_growth_is_rejected(self):
        text = "fn main() {}\nmod tests {\nfn added() {}\n}\n"
        with self.assertRaises(BoundaryError) as caught:
            check_inline_tests(text, 1)
        self.assertIn("event_loop inline tests grew", str(caught.exception))

    def test_account_mod_growth_is_rejected(self):
        text = "mod cc_switch;\nmod cursor_account;\nmod grok_account;\nmod extra_account;\n"
        with self.assertRaises(BoundaryError) as caught:
            check_account_mods(text, self.baseline["acp_account_mods"])
        self.assertIn("extra_account", str(caught.exception))

    def test_ungated_account_mod_is_rejected(self):
        text = "mod cc_switch;\n"
        with self.assertRaises(BoundaryError) as caught:
            check_feature_gates(text)
        self.assertIn("account-import", str(caught.exception))

    def test_shell_spawn_growth_is_rejected(self):
        files = {
            "acp/spawn.rs": "pub async fn spawn_grok_shell(\n",
            "headless.rs": "spawn_grok_shell(agent, cancel, memory).await\n",
            "extra.rs": "spawn_grok_shell(agent, cancel, memory).await\n",
        }
        found = [
            name
            for name, text in sorted(files.items())
            if shell_spawn_calls_in(name, text)
        ]
        with self.assertRaises(BoundaryError) as caught:
            shrink_only(found, ["headless.rs"], "pager shell spawn")
        self.assertIn("extra.rs", str(caught.exception))

    def test_spawn_definition_is_not_a_call_site(self):
        self.assertFalse(
            shell_spawn_calls_in("acp/spawn.rs", "pub async fn spawn_grok_shell(\n")
        )

    def test_stdio_shell_agent_is_rejected(self):
        text = "pub async fn run_stdio(&self) { run_stdio_agent().await }\n"
        with self.assertRaises(BoundaryError) as caught:
            check_stdio_runtime(
                text,
                self.baseline["pager_stdio_shell_needles"],
                self.baseline["pager_stdio_forbidden"],
            )
        self.assertIn("run_stdio_agent(", str(caught.exception))

    def test_stdio_sidecar_removal_is_rejected(self):
        text = "pub async fn run_stdio(&self) {}\n"
        with self.assertRaises(BoundaryError) as caught:
            check_stdio_runtime(
                text,
                self.baseline["pager_stdio_shell_needles"],
                self.baseline["pager_stdio_forbidden"],
            )
        self.assertIn("run_sidecar_on_stdio(", str(caught.exception))

    def test_sidecar_gate_reorder_is_rejected(self):
        text = (
            "pub async fn spawn_a3s_acp() {}\n"
            "pub async fn spawn_required_sidecar() {\n"
            "    spawn_a3s_acp().await\n"
            "    if !a3s_acp_enabled() {}\n"
            "}\n"
            "pub async fn later() {}\n"
        )
        with self.assertRaises(BoundaryError) as caught:
            check_sidecar_gate(text, self.baseline["pager_interactive_sidecar"])
        self.assertIn("order changed", str(caught.exception))

    def test_sidecar_entry_removal_is_rejected(self):
        import tempfile

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "headless.rs").write_text("spawn_grok_shell(\n", encoding="utf-8")
            with self.assertRaises(BoundaryError) as caught:
                check_sidecar_entries(root, {"headless.rs": "spawn_required_sidecar("})
            self.assertIn("headless.rs", str(caught.exception))

    def test_leader_sidecar_switch_is_rejected(self):
        text = "type Agent = MvpAgent;\nspawn_required_sidecar(\n"
        with self.assertRaises(BoundaryError) as caught:
            check_leader_in_process(text)
        self.assertIn("in-process", str(caught.exception))

    def test_stale_core_version_growth_is_rejected(self):
        with self.assertRaises(BoundaryError) as caught:
            check_stale_core_version("a3s-code-core 9.0.0\n", 0)
        self.assertIn("9.0.0", str(caught.exception))

    def test_core_dep_on_aster_is_allowed(self):
        text = (
            "[dependencies]\n"
            "a3s-code-core = {path = \"../../crates/code/core\"}\n"
            "a3s-code-pager = {path = \"x\"}\n"
            "a3s-code-pager-minimal = {path = \"x\"}\n"
            "a3s-code-shell = {path = \"x\"}\n"
        )
        check_pager_deps(text, self.baseline["aster_pager_deps"])


if __name__ == "__main__":
    unittest.main()
