"""Execute draft assembly with local artifacts and a fake GitHub CLI."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE_SHA = "a" * 40
NAMES = [f"aster-cli-v0.1.5-{os}.{extension}" for os, extension in
         [("macos", "tar.gz"), ("linux", "tar.gz"), ("windows", "zip")]]


class ReleaseAssembly(unittest.TestCase):
    def assemble(self, missing=False, extra=False, existing_draft=False, draft=True, draft_sha=SOURCE_SHA,
                 qualification_exit=0, artifact_run_id="123"):
        lines = (ROOT / ".github/workflows/release-aster-cli.yml").read_text().splitlines()
        start = lines.index("      - name: Assemble draft aster-cli v0.1.5")
        start = lines.index("        run: |", start) + 1
        end = start
        while end < len(lines) and (not lines[end].strip() or lines[end].startswith("          ")):
            end += 1
        script = textwrap.dedent("\n".join(lines[start:end]))
        self.assertTrue(script)
        with tempfile.TemporaryDirectory(prefix="aster-release-") as directory:
            root = Path(directory)
            dist, commands = root / "dist", root / "commands"
            dist.mkdir()
            commands.mkdir()
            tests = root / "tests"
            tests.mkdir()
            (tests / "cli_sandbox_smoke.py").write_text(
                'import os\nraise SystemExit(int(os.environ["TEST_QUALIFICATION_EXIT"]))\n')
            for name in NAMES:
                (dist / name).write_bytes(("opaque fixture " + name).encode())
            if missing:
                (dist / NAMES[-1]).unlink()
            if extra:
                (dist / "aster-cli-v0.1.5-kylin.tar.gz").write_bytes(b"retired fixture")
            gh = commands / "gh"
            gh.write_text('''#!/bin/sh
case "$1:$2" in
  api:repos/fixture/aster/actions/runs/123) printf '%s\\n' "$TEST_SOURCE_SHA" ;;
  release:view)
    if [ "$TEST_EXISTING_DRAFT" = 1 ]; then
      case "${5:-}" in
        isDraft) printf '%s\\n' "$TEST_IS_DRAFT" ;;
        targetCommitish) printf '%s\\n' "$TEST_DRAFT_SHA" ;;
      esac
    else test -f "$TEST_CREATE_ARGS"; fi ;;
  release:create|release:upload) printf '%s\\n' "$@" > "$TEST_CREATE_ARGS" ;;
  *) exit 91 ;;
esac
''')
            gh.chmod(0o755)
            # Keep checksum generation hermetic on the macOS publisher and local test hosts.
            sha = commands / "shasum"
            sha.write_text('''#!/usr/bin/env python3
import hashlib, pathlib, sys
name = sys.argv[-1]
print(hashlib.sha256(pathlib.Path(name).read_bytes()).hexdigest() + " *" + name)
''')
            sha.chmod(0o755)
            receipt = root / "create.args"
            env = {"PATH": str(commands) + os.pathsep + os.environ["PATH"],
                   "HOME": directory, "GH_REPO": "fixture/aster", "GH_TOKEN": "test",
                   "ARTIFACT_RUN_ID": artifact_run_id, "BUILD_SHA": "b" * 40,
                   "TEST_SOURCE_SHA": SOURCE_SHA, "TEST_CREATE_ARGS": str(receipt),
                   "TEST_EXISTING_DRAFT": "1" if existing_draft else "0",
                   "TEST_IS_DRAFT": "true" if draft else "false", "TEST_DRAFT_SHA": draft_sha,
                   "TEST_QUALIFICATION_EXIT": str(qualification_exit)}
            result = subprocess.run(["bash", "-c", script], cwd=root, env=env, text=True, capture_output=True, timeout=10)
            manifest = json.loads((dist / "latest.json").read_text()) if (dist / "latest.json").exists() else None
            args = receipt.read_text().splitlines() if receipt.exists() else None
            return result, manifest, args

    def test_three_archives_form_a_draft_bound_to_the_artifact_source(self):
        result, manifest, args = self.assemble()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(manifest["version"], "0.1.5")
        self.assertEqual(manifest["tag"], "aster-cli-v0.1.5")
        self.assertEqual(set(manifest["assets"]), {"macos", "linux", "windows"})
        self.assertEqual(args[args.index("--target") + 1], SOURCE_SHA)
        self.assertIn("--draft", args)
        for entry in manifest["assets"].values():
            name = entry["name"]
            self.assertIn(name, NAMES)
            self.assertIn("dist/" + name, args)
            self.assertEqual(entry["sha256"], hashlib.sha256(("opaque fixture " + name).encode()).hexdigest())

    def test_missing_or_retired_archives_refuse_before_release_creation(self):
        for missing, extra in [(True, False), (False, True)]:
            with self.subTest(missing=missing, extra=extra):
                result, manifest, args = self.assemble(missing=missing, extra=extra)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("expected 3 release archives", result.stderr)
                self.assertIsNone(manifest)
                self.assertIsNone(args)

    def test_normal_build_qualifies_and_binds_the_draft_to_its_own_source(self):
        result, manifest, args = self.assemble(artifact_run_id="")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(args[args.index("--target") + 1], "b" * 40)
        self.assertEqual(set(manifest["assets"]), {"macos", "linux", "windows"})

    def test_matching_existing_draft_reuses_its_source_without_a_tag_lookup(self):
        result, manifest, args = self.assemble(existing_draft=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(args[:2], ["release", "upload"])
        self.assertIn("--clobber", args)
        self.assertEqual(set(manifest["assets"]), {"macos", "linux", "windows"})

    def test_published_or_different_source_release_refuses_upload(self):
        for draft, source in [(False, SOURCE_SHA), (True, "b" * 40)]:
            with self.subTest(draft=draft, source=source):
                result, _, args = self.assemble(existing_draft=True, draft=draft, draft_sha=source)
                self.assertNotEqual(result.returncode, 0)
                self.assertIsNone(args)

    def test_failed_installed_archive_qualification_blocks_create_and_upload(self):
        for artifact_run_id in ("", "123"):
            for existing_draft in (False, True):
                with self.subTest(artifact_run_id=artifact_run_id, existing_draft=existing_draft):
                    result, manifest, args = self.assemble(existing_draft=existing_draft,
                        qualification_exit=53, artifact_run_id=artifact_run_id)
                    self.assertEqual(result.returncode, 53)
                    self.assertIsNone(manifest)
                    self.assertIsNone(args)

    def test_windows_build_uses_a_short_cargo_home(self):
        workflow = (ROOT / ".github/workflows/release-aster-cli.yml").read_text()
        self.assertIn(
            "      - name: Shorten Windows Cargo home\n        if: runner.os == 'Windows'",
            workflow,
        )
        self.assertIn("CARGO_HOME=D:\\c", workflow)


if __name__ == "__main__":
    unittest.main()
