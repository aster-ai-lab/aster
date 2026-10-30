"""Build, freeze and run an explicit feature inventory on one native platform."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser()
parser.add_argument("platform", choices=("windows", "linux"))
parser.add_argument("phase")
parser.add_argument("--config", type=Path, required=True)
parser.add_argument("--expected-tests", type=int, required=True)
parser.add_argument("tests", nargs="+")
args = parser.parse_args()
root = Path.cwd().resolve()
campaign = Path(__file__).resolve().parent.parent
audit = campaign / args.platform / args.phase
audit.mkdir(parents=True, exist_ok=True)
suffix = ".exe" if args.platform == "windows" else ""
compiled = root / ".tmp" / (args.phase + "-tests" + suffix)
with (audit / "test-build.log").open("wb") as log:
    code = subprocess.run(["cargo", "rustc", "--locked", "-p", "a3s-code-acp", "--test",
                           "aster_tui_live_e2e", "--", "--emit=link=" + str(compiled)],
                          stdout=log, stderr=subprocess.STDOUT).returncode
if code:
    raise SystemExit(code)
with (audit / "build.jsonl").open("wb") as out, (audit / "build.log").open("wb") as err:
    code = subprocess.run(["cargo", "build", "--locked", "-p", "aster", "-p", "a3s-code-acp",
                           "--bins", "--message-format=json"], stdout=out, stderr=err).returncode
if code:
    raise SystemExit(code)
for name, command in [
    ("freeze.log", [sys.executable, str(campaign / "source/continuation_native_pair.py"), args.platform, "--phase", args.phase]),
    ("test-freeze.log", [sys.executable, str(campaign / "source/freeze_native_test.py"), args.platform,
                         args.phase, str(compiled), "--expected-tests", str(args.expected_tests)]),
]:
    with (audit / name).open("wb") as log:
        code = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT).returncode
    if code:
        raise SystemExit(code)
pair = json.loads(Path((audit / "manifest-path.txt").read_text().strip()).read_text())
harness = json.loads((audit / "test-binary.json").read_text())
assert set(args.tests).issubset(harness["entrypoints"])
env = os.environ.copy()
env.update({"A3S_CONFIG_FILE": str(args.config.resolve()),
            "A3S_CODE_TUI_BIN": str(Path(pair["folder"]) / ("aster" + suffix)),
            "A3S_CODE_ACP_BIN": str(Path(pair["folder"]) / ("a3s-code-acp" + suffix)),
            "A3S_CODE_ACP_TRACE_PROXY": str(root / ".tmp" / ("acp-trace-proxy.exe" if suffix else "acp-trace-proxy-linux")),
            "A3S_TUI_LIVE_EVIDENCE_DIR": str(audit / "reports")})
results = []
for test in args.tests:
    log = audit / (test.rpartition("::")[-1] + ".log")
    with log.open("wb") as stream:
        code = subprocess.run([harness["binary"], test, "--ignored", "--exact", "--nocapture", "--test-threads=1"],
                              env=env, stdout=stream, stderr=subprocess.STDOUT).returncode
    results.append({"test": test, "exit_code": code, "log": log.relative_to(campaign).as_posix()})
    (audit / "feature-results.json").write_text(json.dumps({"source_pair": pair["source_pair"], "tests": results}, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(results[-1]), flush=True)
raise SystemExit(int(any(item["exit_code"] for item in results)))
