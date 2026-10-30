"""Verify the retained native background-ownership run and terminal gates."""
import hashlib
import json
from pathlib import Path


CAMPAIGN = Path(__file__).resolve().parent.parent
ROOT = CAMPAIGN.parents[3]
DIGEST = "e1a37d311f896ceebb6a7a228a0f6a889f15313b"
source = json.loads((CAMPAIGN / "source/continuation-20261030-source.json").read_text())
patch = (CAMPAIGN / "source" / source["patch"]).read_bytes()
assert len(patch) == source["bytes"]
assert hashlib.sha256(patch).hexdigest() == source["sha256"]
for file in source["files"]:
    raw = (ROOT / file["path"]).read_bytes()
    assert len(raw) == file["bytes"] and hashlib.sha256(raw).hexdigest() == file["sha256"]

platforms = []
for platform, unit_count in (("windows", 39), ("linux", 75)):
    relative = f"{platform}/continuation-20261030-owner/background-session-read-only-owner.json"
    raw = (CAMPAIGN / relative).read_bytes()
    report = json.loads(raw)
    assert report["assertions_finished"] and not report["panicked"]
    assert report["model"] == "kimi/k3.1-pro" and report["config_unchanged"]
    oracles = {item["name"]: item for item in report["oracles"] if "name" in item}
    required = {
        "background-permission-owner", "background-permission-card-owner",
        "background-tool-owner", "background-response-owner",
    }
    assert required.issubset(oracles)
    pair = oracles["source-pair"]["detail"]
    assert pair.split(":")[1] == DIGEST
    audit = CAMPAIGN / platform / "continuation-20261030-pair"
    manifest_path = (audit / "manifest-path.txt").read_text().strip()
    if platform == "linux" and ROOT.drive:
        manifest_path = "D:/" + manifest_path.removeprefix("/mnt/d/")
    manifest_path = Path(manifest_path)
    manifest = json.loads(manifest_path.read_text())
    assert manifest["source_pair"] == pair
    assert len(manifest["files"]) == 2
    gates = CAMPAIGN / platform / "continuation-20261030-gates"
    units = (gates / "tty-tests.log").read_text(encoding="utf-8-sig")
    assert f"test result: ok. {unit_count} passed; 0 failed" in units
    assert "test result: ok. 2 passed; 0 failed" in units
    clippy = (gates / "tty-clippy-fixed.log").read_text(encoding="utf-8-sig")
    assert "Finished `dev`" in clippy and "error:" not in clippy
    product_clippy = (gates / "product-clippy.log").read_text(encoding="utf-8-sig")
    assert "a3s-common" in product_clippy
    assert "clippy::derivable_impls" in product_clippy
    assert "clippy::unnecessary_sort_by" in product_clippy
    platforms.append({
        "platform": platform, "source_pair": pair,
        "pair_manifest": manifest_path.relative_to(ROOT).as_posix(),
        "binaries": manifest["files"],
        "report": {"file": relative, "sha256": hashlib.sha256(raw).hexdigest()},
        "required_oracles": sorted(required), "tty_unit_tests": unit_count,
        "tty_doctests": 2, "tty_clippy": "pass", "product_clippy": "fail",
    })

result = {
    "schema": "aster.tui.continuation-ownership.v1",
    "model": "kimi/k3.1-pro", "source_pair_digest": DIGEST,
    "complete_for_F04_07": True, "complete_for_full_acceptance": False,
    "source_patch_sha256": source["sha256"], "platforms": platforms,
    "product_clippy_blockers": ["a3s-common: derivable_impls", "a3s-common: unnecessary_sort_by"],
}
dest = CAMPAIGN / "source/continuation-20261030-verification.json"
dest.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8", newline="\n")
assert json.loads(dest.read_text()) == result
print(json.dumps({"complete_for_F04_07": True, "complete_for_full_acceptance": False,
                  "platforms": len(platforms), "source_files_verified": len(source["files"])}))
