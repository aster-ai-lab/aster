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
NAMES = [f"aster-cli-v0.1.4-{os}.{extension}" for os, extension in
         [("macos", "tar.gz"), ("linux", "tar.gz"), ("windows", "zip")]]


class ReleaseAssembly(unittest.TestCase):
    def assemble(self, missing=False, extra=False, existing_draft=False, draft=True, draft_sha=SOURCE_SHA):
        lines = (ROOT / ".github/workflows/release-aster-cli.yml").read_text().splitlines()
        start = lines.index("      - name: Assemble draft aster-cli v0.1.4")
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
            for name in NAMES:
                (dist / name).write_bytes(("opaque fixture " + name).encode())
            if missing:
                (dist / NAMES[-1]).unlink()
            if extra:
                (dist / "aster-cli-v0.1.4-kylin.tar.gz").write_bytes(b"retired fixture")
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
            # The Linux publisher uses GNU sha256sum; this keeps its shell hermetic on macOS too.
            sha = commands / "sha256sum"
            sha.write_text('''#!/usr/bin/env python3
import hashlib, pathlib, sys
name = sys.argv[-1]
print(hashlib.sha256(pathlib.Path(name).read_bytes()).hexdigest() + " *" + name)
''')
            sha.chmod(0o755)
            receipt = root / "create.args"
            env = {"PATH": str(commands) + os.pathsep + os.environ["PATH"],
                   "HOME": directory, "GH_REPO": "fixture/aster", "GH_TOKEN": "test",
                   "ARTIFACT_RUN_ID": "123", "BUILD_SHA": "b" * 40,
                   "TEST_SOURCE_SHA": SOURCE_SHA, "TEST_CREATE_ARGS": str(receipt),
                   "TEST_EXISTING_DRAFT": "1" if existing_draft else "0",
                   "TEST_IS_DRAFT": "true" if draft else "false", "TEST_DRAFT_SHA": draft_sha}
            result = subprocess.run(["bash", "-c", script], cwd=root, env=env, text=True, capture_output=True, timeout=10)
            manifest = json.loads((dist / "latest.json").read_text()) if (dist / "latest.json").exists() else None
            args = receipt.read_text().splitlines() if receipt.exists() else None
            return result, manifest, args

    def test_three_archives_form_a_draft_bound_to_the_artifact_source(self):
        result, manifest, args = self.assemble()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(manifest["version"], "0.1.4")
        self.assertEqual(manifest["tag"], "aster-cli-v0.1.4")
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


class ReleaseWorkflowContract(unittest.TestCase):
    def test_tag_builds_require_premerge_qualification_without_repeating_it(self):
        text = (ROOT / ".github/workflows/release-aster-cli.yml").read_text()
        self.assertIn('tags:\n      - "aster-cli-v[0-9]+.[0-9]+.[0-9]+"', text)
        self.assertIn("name: Verify pre-merge qualification", text)
        self.assertIn("actions/workflows/qualify-aster-cli.yml/runs?head_sha=${RELEASE_SHA}&event=push&status=completed", text)
        self.assertIn(".head_branch !=", text)
        self.assertEqual(text.count("needs: qualified"), 2)
        self.assertNotIn("preflight", text)
        self.assertNotIn("cargo fmt", text)
        self.assertNotIn("cargo test", text)
        self.assertNotIn("python3 tests/", text)

    def test_all_three_cli_platforms_have_product_specific_caches(self):
        text = (ROOT / ".github/workflows/release-aster-cli.yml").read_text()
        self.assertEqual(text.count("uses: Swatinem/rust-cache@"), 2)
        self.assertIn("shared-key: aster-cli-release-${{ matrix.platform }}", text)
        self.assertIn("shared-key: aster-cli-release-linux", text)
        self.assertIn("asset: aster-cli-v0.1.4-macos.tar.gz", text)
        self.assertIn("asset: aster-cli-v0.1.4-windows.zip", text)
        self.assertIn('package-aster-cli.sh linux "$bin_dir" "aster-cli-v0.1.4-linux.tar.gz"', text)
        self.assertIn('run-id: ${{ inputs.artifact_run_id != \'\' && inputs.artifact_run_id || github.run_id }}', text)

    def test_premerge_cli_workflow_runs_tests_and_delivery_contracts(self):
        text = (ROOT / ".github/workflows/qualify-aster-cli.yml").read_text()
        self.assertIn('push:\n    branches: ["**"]\n  pull_request:', text)
        self.assertIn("cargo build --locked -p aster -p a3s-code-acp", text)
        self.assertIn("cargo test --locked -p aster --bin aster -- --test-threads=4", text)
        self.assertIn("python3 tests/cli_pair_delivery.py", text)
        self.assertIn("python3 tests/aster_cli_linux.py", text)
        self.assertIn("python3 tests/cli_release_workflow.py", text)
        self.assertIn("shared-key: aster-cli-qualify-${{ matrix.os }}", text)


if __name__ == "__main__":
    unittest.main()
