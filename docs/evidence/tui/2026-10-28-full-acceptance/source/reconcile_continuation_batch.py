"""Bind the original 57-entrypoint batches, preserving every failing evaluation."""
import hashlib
import json
from pathlib import Path
import re

CAMPAIGN = Path(__file__).resolve().parent.parent
ROOT = CAMPAIGN.parents[3]
DIGEST = "e1a37d311f896ceebb6a7a228a0f6a889f15313b"
platforms = []
inventories = []
for platform in ("windows", "linux"):
    audit = CAMPAIGN / platform / "continuation-20261030-pair"
    inventory = (audit / "all-57-tests.txt").read_text(encoding="utf-8-sig")
    names = re.findall(r"^(\S+): test$", inventory, re.M)
    assert len(names) == len(set(names)) == 57
    inventories.append(set(names))
    raw = (audit / "all-57-run.log").read_bytes()
    text = raw.decode("utf-8-sig")
    summaries = re.findall(
        r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;.*?finished in ([0-9.]+)s",
        text,
    )
    assert len(summaries) == 1, f"{platform}: original batch is not finished"
    state, passed, failed, ignored, seconds = summaries[0]
    assert int(passed) + int(failed) == 57 and int(ignored) == 0
    observed = re.findall(r"^test (\S+) \.\.\.", text, re.M)
    assert set(observed) == set(names), f"{platform}: entrypoint inventory differs"
    manifest_path = (audit / "manifest-path.txt").read_text().strip()
    if platform == "linux" and ROOT.drive:
        manifest_path = "D:/" + manifest_path.removeprefix("/mnt/d/")
    manifest = json.loads(Path(manifest_path).read_text())
    assert manifest["source_pair"].split(":")[1] == DIGEST
    reports = []
    for path in sorted((CAMPAIGN / platform / "continuation-20261030-all-57").glob("*.json")):
        contents = path.read_bytes()
        report = json.loads(contents)
        assert report["model"] == "kimi/k3.1-pro" and report["config_unchanged"]
        pair = next(item["detail"] for item in report["oracles"] if item.get("name") == "source-pair")
        assert pair == manifest["source_pair"]
        reports.append({
            "case": report["case"], "file": path.relative_to(CAMPAIGN).as_posix(),
            "sha256": hashlib.sha256(contents).hexdigest(),
            "assertions_finished": report["assertions_finished"], "panicked": report["panicked"],
        })
    assert reports
    if state == "ok":
        assert all(item["assertions_finished"] and not item["panicked"] for item in reports)
    platforms.append({
        "platform": platform, "source_pair": manifest["source_pair"], "binaries": manifest["files"],
        "entrypoints_passed": int(passed), "entrypoints_failed": int(failed),
        "seconds": float(seconds), "run_log_sha256": hashlib.sha256(raw).hexdigest(),
        "case_reports": reports,
    })
assert inventories[0] == inventories[1]
result = {
    "schema": "aster.tui.native-final-runs.v1", "model": "kimi/k3.1-pro",
    "source_pair_digest": DIGEST, "phase": "continuation-20261030-all-57",
    "all_entrypoints_passed": all(item["entrypoints_failed"] == 0 for item in platforms),
    "checkpoint_count": 153, "complete_153": False, "whole_pager_qualified": False,
    "supplemental_runs_do_not_replace_original_results": True, "platforms": platforms,
}
dest = CAMPAIGN / "RUNS-E1A57.json"
assert not dest.exists(), "retain the previous reconciliation"
dest.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8", newline="\n")
assert json.loads(dest.read_text()) == result
print(json.dumps({item["platform"]: {"passed": item["entrypoints_passed"],
                                     "failed": item["entrypoints_failed"]} for item in platforms}))
