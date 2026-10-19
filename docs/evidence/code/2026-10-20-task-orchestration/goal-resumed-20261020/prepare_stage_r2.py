#!/usr/bin/env python3
"""Prepare the one authorized R2 development bundle; never launch it."""

import datetime
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import stat
import subprocess
import time


ROOT = Path("/Users/roylin/code/aster")
EVIDENCE = ROOT / "docs/evidence/code/2026-10-20-task-orchestration/goal-resumed-20261020"
STAGE = ROOT / "target/development-stage/task-optimization-r2-20261020"
APP = STAGE / "AsterDream-TaskOptimization-R2.app"
ACL = STAGE / "host-capabilities.acl"
REPORT = EVIDENCE / "stage.json"
IDENTIFIER = "dev.a3s.studio.aster.task-optimization-r2"
ASSETS = ROOT / "target/t013-assets-hardware"
ICON = ROOT / "apps/work/build/icon.icns"
ACL_BYTES = b'tool_presentation { mode = "adaptive" }\nworkspace_retrieval { enabled = false }\n'
ACL_SHA256 = "213428fb5061c855144ef5a551512eab0112ac6196ceef14abc5b35337161418"
OLD_APPS = [
    Path("/Applications/AsterDream.app"),
    Path("/Users/roylin/Library/Application Support/dev.a3s.studio/aster/AsterDream.app"),
    ROOT / "target/development-stage/task-orchestration-20261020/AsterDream-TaskOrchestration-Dev.app",
]
PUBLIC_FILES = [
    "Contents/Info.plist",
    "Contents/MacOS/AsterDream",
    "Contents/MacOS/a3s-code-acp",
    "Contents/MacOS/a3s-office",
    "Contents/Resources/AsterDream.icns",
]


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def identity(path):
    metadata = path.lstat()
    assert stat.S_ISREG(metadata.st_mode), f"expected a regular file: {path}"
    return {"path": str(path), "sha256": sha(path), "bytes": metadata.st_size,
            "mode": oct(stat.S_IMODE(metadata.st_mode))}


def public_bundle_snapshot(app):
    assert app.is_dir() and not app.is_symlink(), f"expected existing app: {app}"
    executable = plistlib.loads((app / "Contents/Info.plist").read_bytes())["CFBundleExecutable"]
    assert isinstance(executable, str) and executable and Path(executable).name == executable
    declared_binary = f"Contents/MacOS/{executable}"
    result = {}
    for relative in dict.fromkeys([*PUBLIC_FILES, declared_binary]):
        path = app / relative
        if not path.exists() and not path.is_symlink():
            assert relative not in ["Contents/Info.plist", declared_binary], f"missing old app identity: {path}"
            result[relative] = None
        else:
            result[relative] = identity(path)
    return result


def main():
    assert Path(__file__).resolve() == EVIDENCE / "prepare_stage_r2.py"
    assert ROOT.resolve() == ROOT and STAGE.resolve() == STAGE
    assert STAGE.is_dir() and not STAGE.is_symlink()
    assert not APP.exists() and not APP.is_symlink(), "refuse to overwrite an existing app"
    assert not ACL.exists() and not ACL.is_symlink(), "require a fresh host ACL"
    assert not REPORT.exists() and not REPORT.is_symlink(), "require a fresh stage report"
    assert len(ACL_BYTES) == 80 and hashlib.sha256(ACL_BYTES).hexdigest() == ACL_SHA256
    assert ASSETS.is_dir() and not ASSETS.is_symlink()

    # A missing/incomplete build report fails before creating the stage.
    builds = {}
    for name in ["paired-build", "build-work"]:
        path = EVIDENCE / f"{name}.json"
        recorded = json.loads(path.read_text())
        assert recorded.get("exit_code") == 0, f"{name} did not complete successfully"
        builds[name] = {"identity": identity(path), "exit_code": recorded["exit_code"],
                        "command": recorded["command"]}
    sources = {name: ROOT / "target/debug" / name
               for name in ["aster-dream", "a3s-code-acp", "a3s-office", "aster"]}
    source_before = {name: identity(path) for name, path in sources.items()}
    old_before = {str(app): public_bundle_snapshot(app) for app in OLD_APPS}
    icon_before = identity(ICON)
    template = ROOT / "apps/work/src-tauri/src/macos_app.rs"
    template_before = identity(template)
    metadata_path = ROOT / "apps/work/src-tauri/tauri.conf.json"
    metadata_before = identity(metadata_path)
    version = json.loads(metadata_path.read_text())["version"]
    assert isinstance(version, str) and version, "missing public package version"
    environment = {"A3S_CONFIG": str(ACL), "A3S_STUDIO_RENDERER_PORT": "4322",
                   "ASTER_SANDBOX_DIR": str(ASSETS)}
    report = {"schema": "aster.development-stage-signing.r2",
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "state": "preparing", "app": str(APP), "launched": False,
              "script": identity(Path(__file__)), "builds": builds,
              "source_before": source_before, "old_public_files_before": old_before,
              "icon_source_before": icon_before, "template_source_before": template_before,
              "package_metadata_before": metadata_before, "environment": environment,
              "binaries": {}, "commands": [], "external_assets": {
                  "path": str(ASSETS), "modified": False, "signed": False,
                  "full_integrity_check": "Separate root-owned qualification; not performed by this script"}}
    with REPORT.open("x") as destination:
        destination.write(json.dumps(report, indent=2) + "\n")

    def save():
        REPORT.write_text(json.dumps(report, indent=2) + "\n")

    def command(name, argv):
        stdout = EVIDENCE / f"stage-r2-{name}.stdout.log"
        stderr = EVIDENCE / f"stage-r2-{name}.stderr.log"
        assert not stdout.exists() and not stderr.exists(), "refuse to overwrite command logs"
        started = time.monotonic()
        completed = subprocess.run(argv, capture_output=True, text=True, timeout=60)
        with stdout.open("x") as destination:
            destination.write(completed.stdout)
        with stderr.open("x") as destination:
            destination.write(completed.stderr)
        report["commands"].append({"name": name, "command": argv,
                                   "exit_code": completed.returncode,
                                   "wall_seconds": time.monotonic() - started,
                                   "stdout": str(stdout), "stderr": str(stderr)})
        save()
        assert completed.returncode == 0, f"stage command failed: {name}"
        return completed.stdout + completed.stderr

    def signature(name, path, expected_identifier):
        output = command(name, ["/usr/bin/codesign", "-d", "--verbose=4", str(path)])
        fields = dict(line.split("=", 1) for line in output.splitlines() if "=" in line)
        assert fields.get("Identifier") == expected_identifier
        assert fields.get("Signature") == "adhoc" and fields.get("CDHash")
        return {key: fields[key] for key in ["Identifier", "Signature", "CDHash"]}

    try:
        APP.mkdir()
        contents = APP / "Contents"
        contents.mkdir()
        macos = contents / "MacOS"
        macos.mkdir()
        resources = contents / "Resources"
        resources.mkdir()
        descriptor = os.open(ACL, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "wb") as destination:
            os.fchmod(destination.fileno(), 0o600)
            destination.write(ACL_BYTES)
        report["host_capabilities"] = identity(ACL)
        assert report["host_capabilities"]["sha256"] == ACL_SHA256
        assert report["host_capabilities"]["mode"] == "0o600"

        for name, filename in [("aster-dream", "AsterDream"),
                               ("a3s-code-acp", "a3s-code-acp"), ("a3s-office", "a3s-office")]:
            source, destination = sources[name], macos / filename
            assert identity(source) == source_before[name], "source binary changed before copy"
            with destination.open("xb") as target, source.open("rb") as origin:
                shutil.copyfileobj(origin, target, 1024 * 1024)
            destination.chmod(0o755)
            copied = identity(destination)
            assert copied["sha256"] == source_before[name]["sha256"], "copied binary differs"
            report["binaries"][name] = {"unsigned_copy": copied}
            save()
        icon_target = resources / "AsterDream.icns"
        with icon_target.open("xb") as target, ICON.open("rb") as origin:
            shutil.copyfileobj(origin, target)
        assert identity(icon_target)["sha256"] == icon_before["sha256"]
        report["icon_copy"] = identity(icon_target)
        plist = {"CFBundleDisplayName": "AsterDream", "CFBundleExecutable": "AsterDream",
                 "CFBundleIconFile": "AsterDream", "CFBundleIdentifier": IDENTIFIER,
                 "CFBundleInfoDictionaryVersion": "6.0", "CFBundleName": "AsterDream",
                 "CFBundlePackageType": "APPL", "CFBundleShortVersionString": version,
                 "CFBundleVersion": version, "NSAppTransportSecurity": {
                     "NSAllowsArbitraryLoads": True, "NSAllowsArbitraryLoadsInWebContent": True},
                 "NSHighResolutionCapable": True, "NSPrincipalClass": "NSApplication",
                 "LSEnvironment": environment}
        plist_path = contents / "Info.plist"
        with plist_path.open("xb") as destination:
            plistlib.dump(plist, destination, fmt=plistlib.FMT_XML, sort_keys=False)
        report["plist"] = {"identity": identity(plist_path), "values": plist}
        command("plist-lint", ["/usr/bin/plutil", "-lint", str(plist_path)])
        for name, suffix in [("a3s-code-acp", "acp"), ("a3s-office", "office")]:
            path = macos / name
            command(f"sign-{suffix}", ["/usr/bin/codesign", "--force", "--sign", "-",
                                        "--identifier", f"{IDENTIFIER}.{suffix}", str(path)])
            report["binaries"][name]["inner_signed"] = identity(path)
        command("sign-app", ["/usr/bin/codesign", "--force", "--sign", "-",
                             "--identifier", IDENTIFIER, str(APP)])
        command("verify-strict-deep", ["/usr/bin/codesign", "--verify", "--deep", "--strict",
                                       "--verbose=2", str(APP)])
        report["app_signature"] = signature("signature-app", APP, IDENTIFIER)
        for name, filename, suffix in [("aster-dream", "AsterDream", ""),
                                      ("a3s-code-acp", "a3s-code-acp", ".acp"),
                                      ("a3s-office", "a3s-office", ".office")]:
            path = macos / filename
            report["binaries"][name]["signed_final"] = identity(path)
            report["binaries"][name]["signature"] = signature(
                f"signature-{name}", path, IDENTIFIER + suffix)
        assert set(path.name for path in macos.iterdir()) == {"AsterDream", "a3s-code-acp", "a3s-office"}
        assert not any(path.is_symlink() for path in APP.rglob("*")), "stage contains a symlink"
        assert plistlib.loads(plist_path.read_bytes()) == plist
        assert identity(ACL) == report["host_capabilities"]
        report["state"] = "signed_strict_verified_not_launched"
    except BaseException as error:
        report["state"] = "failed"
        report["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        report["source_after"] = {name: identity(path) for name, path in sources.items()}
        report["old_public_files_after"] = {str(app): public_bundle_snapshot(app) for app in OLD_APPS}
        report["icon_source_after"] = identity(ICON)
        report["template_source_after"] = identity(template)
        report["package_metadata_after"] = identity(metadata_path)
        unchanged = (report["source_after"] == source_before
                     and report["old_public_files_after"] == old_before
                     and report["icon_source_after"] == icon_before
                     and report["template_source_after"] == template_before
                     and report["package_metadata_after"] == metadata_before)
        if not unchanged:
            report["state"] = "failed"
            report["error"] = "source binary, old public bundle identity, icon or metadata changed"
        report["terminal_companion"] = {"before": source_before["aster"],
                                         "after": report["source_after"]["aster"], "copied": False}
        report["ended_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        save()
        assert unchanged, report.get("error")
    print(json.dumps({"state": report["state"], "app": str(APP), "report": str(REPORT)}))


if __name__ == "__main__":
    main()
