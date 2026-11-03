"""Reconcile the original inventory using the real native AIX builder.

Run from the repository root with an already-built aster-use executable.
This produces evidence and planning data, never publication or execution claims.
"""

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdk", type=Path, required=True)
    args = parser.parse_args()
    sdk = args.sdk.resolve(strict=True)
    evidence = Path("docs/evidence/cli/2026-11-03-convergence")
    contracts = Path("specs/031-cli-mise-replacement/contracts")
    previous = Path("docs/evidence/cli/2026-11-01-mise-replacement")
    original = json.loads((previous / "plugins-source-build-35.json").read_text())
    dependencies = json.loads((previous / "desktop-plugin-inventory.json").read_text())
    assert len(original["results"]) == 35 and len(dependencies["entries"]) == 14
    expected_sources = {Path(x["source"]) for x in original["results"]}
    assert expected_sources == {p.parent for p in Path("apps/use-registry/packages").glob("*/aster-use-extension.acl")}
    archives = Path(tempfile.mkdtemp(prefix="original-aix-", dir=".tmp/mise-convergence-implementation"))
    captures = []
    entries = []
    catalogue = [json.loads(p.read_text()) for p in Path("plugins/registry/catalog").glob("**/*.json")]
    for item in original["results"]:
        source = Path(item["source"])
        archive = archives / (source.name + ".aix")
        command = [str(sdk), "registry", "package", "build", str(source.resolve()), "--output", str(archive.resolve()), "--json"]
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", timeout=30)
        captures.append({"source": str(source), "command": command, "exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
        if result.returncode:
            write(evidence / ("original-aix-build-failed-" + archives.name + ".json"), {"sdkSha256": sha(sdk), "results": captures})
            raise RuntimeError(f"Native AIX build failed: {source}")
        package = json.loads(result.stdout)["data"]["aixPackage"]
        assert package["packageId"] == item["package"]["packageId"]
        files = [{"path": str(p.relative_to(source)), "sha256": sha(p)} for p in sorted(source.rglob("*")) if p.is_file()]
        entries.append({
            "ordinal": len(entries) + 1, "kind": "application", "originalId": package["route"],
            "packageId": package["packageId"], "source": str(source), "sourceFiles": files,
            "nativePackage": package, "archiveSha256": sha(archive), "archiveBytes": archive.stat().st_size,
            "builtin": False, "required": False, "domain": None, "owner": "T008: official plugin source authoring",
            "implementation": "native_aix_build_passed", "verification": "consumer_pending", "publication": "pending",
            "targets": [], "blockers": ["Explicit domain/license/target review", "Official-source immutable publication", "Real consumer lifecycle and required surfaces"],
            "evidence": str(evidence / "original-aix-builds.json"),
        })
    for item in dependencies["entries"]:
        source = Path("plugins/plugins") / item["domain"] / item["packageId"]
        manifest = source / "aster-use-extension.acl"
        recipe = {
            "aster-box": Path("plugins/recipes/aster-box/darwin-arm64.py"),
            "aster-sandbox": Path("plugins/recipes/runtime/darwin-arm64.py"),
            "aster-document": Path("plugins/recipes/runtime/darwin-arm64.py"),
            "aster-office": Path("plugins/recipes/runtime/darwin-arm64.py"),
        }.get(item["id"])
        if recipe is not None:
            assert recipe.is_file()
        records = [x for x in catalogue if x["packageId"] == item["packageId"]]
        entries.append({
            "ordinal": len(entries) + 1, "kind": "dependency", "originalId": item["id"],
            "packageId": item["packageId"], "originalName": item.get("displayName", item["id"]),
            "source": str(source) if manifest.exists() else str(recipe) if recipe else None,
            "sourceKind": "native_manifest" if manifest.exists() else "native_recipe" if recipe else "missing",
            "manifestSha256": sha(manifest) if manifest.exists() else None,
            "recipeSha256": sha(recipe) if recipe else None,
            "domain": item["domain"], "owner": "T008: official plugin source and Studio runtime owner",
            "builtin": item["id"] in ("aster-sandbox", "aster-document", "aster-office"),
            "required": item.get("required", False),
            "implementation": "maintained_source" if manifest.exists() else "native_recipe" if recipe else "source_missing",
            "verification": "full_consumer_pending", "publication": "catalogue_present" if records else "pending",
            "targets": [{"target": x["target"], "version": x["version"], "requiresUse": x.get("requiresUse"), "archive": x["archive"], "package": x["package"]} for x in records],
            "blockers": ["All applicable native targets and original consumer workflows remain required"],
            "evidence": str(previous / "desktop-plugin-inventory.json"),
        })
    assert len(entries) == 49 and len({x["packageId"] for x in entries}) == 49
    assert [x["originalId"] for x in entries[35:]] == [x["id"] for x in dependencies["entries"]]
    write(evidence / "original-aix-builds.json", {"sdkSha256": sha(sdk), "passed": len(captures), "results": captures})
    write(contracts / "legacy-plugin-inventory.json", {
        "schemaVersion": 1, "observed": "2026-11-03", "originalApplicationCount": 35,
        "originalDependencyCount": 14, "count": 49, "goalComplete": False,
        "sourceInventory": {"applications": str(previous / "plugins-source-build-35.json"), "dependencies": str(previous / "desktop-plugin-inventory.json")},
        "entries": entries,
    })
    print(f"Native AIX builds: {len(captures)} passed; exact original inventory: {len(entries)} retained; consumer/publication qualification remains open.")


if __name__ == "__main__":
    main()
