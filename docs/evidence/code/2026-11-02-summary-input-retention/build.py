from pathlib import Path
import datetime
import json
import os
import sys
import time

sys.path.insert(0, str(Path.cwd() / 'crates/code/acp/tests/long_horizon'))
from timing_runner import Recorder, summarize

root = Path.cwd()
out = Path(__file__).resolve().parent
label = sys.argv[1]
assert not (out / (label + "-build-status.json")).exists()
timing = Recorder(out / (label + '-build-phase-timing.jsonl'))
import atexit
atexit.register(timing.close)
timing.run_capture(["python3", str(out / "namespace.py"), label + "-before-build"], cwd=root, phase="source_snapshot_before").check_returncode()
commands = [{'name': 'pair-build', 'command': ['cargo', 'build', '--locked', '--offline', '--target-dir', 'target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5', '--config', 'profile.dev.package.sha2.opt-level=3', '-p', 'aster', '-p', 'aster-acp', '--bins']}, {'name': 'live-tests-build', 'command': ['cargo', 'test', '--locked', '--offline', '--target-dir', 'target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5', '--config', 'profile.test.package.sha2.opt-level=3', '-p', 'aster-acp', '--lib', '--test', 'deloop_real_llm', '--test', 'deloop_generalization_real_llm', '--test', 'long_horizon_real_llm', '--test', 'acp_conformance_stdio', '--test', 'shipped_tui_e2e', '--test', 'cursor_account_tui_e2e', '--no-run']}]
rows = []
for entry in commands:
    name, command = entry["name"], entry["command"]
    print("START " + name, flush=True)
    started = time.monotonic()
    filename = label + ("-pair-build.log" if name == "pair-build" else "-live-tests-build.log")
    result = timing.run_logged(command, cwd=root, env=dict(os.environ, CARGO_INCREMENTAL="0"), log=out / filename, phase=name.replace('-', '_'))
    row = {"name": name, "command": command, "exitCode": result.returncode, "seconds": time.monotonic() - started,
           "at": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    rows.append(row)
    (out / (label + "-build-status.json")).write_text(json.dumps(rows, indent=2) + "\n")
    print(json.dumps(row), flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
frozen = timing.run_capture(["python3", str(out / "freeze.py"), label, *sys.argv[2:3]], cwd=root, phase="freeze")
if frozen.returncode:
    print(frozen.stdout.decode("utf-8", errors="replace"), end="", flush=True)
    print(frozen.stderr.decode("utf-8", errors="replace"), end="", file=sys.stderr, flush=True)
frozen.check_returncode()
retention = timing.run_capture(
    [sys.executable, "scripts/aster_qualification_cleanup.py", "--execute"],
    cwd=root,
    phase="candidate_retention",
)
if retention.returncode:
    print(retention.stdout.decode("utf-8", errors="replace"), end="", flush=True)
    print(retention.stderr.decode("utf-8", errors="replace"), end="", file=sys.stderr, flush=True)
retention.check_returncode()
timing.close()
paths = [out / (label + '-build-phase-timing.jsonl'), out / (label + '-freeze-phase-timing.jsonl'), *(out / (label + suffix + '-phase-timing.jsonl') for suffix in ['-before-build', '-after-build'])]
(out / (label + '-build-phase-summary.json')).write_text(json.dumps(summarize(paths), indent=2) + '\n')
