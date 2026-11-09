"""Execute draft assembly with local artifacts and a fake GitHub CLI."""
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE_SHA = "a" * 40
NAMES = [f"aster-cli-v0.1.6-{os}.{extension}" for os, extension in
         [("macos", "tar.gz"), ("linux", "tar.gz"), ("linux-arm64", "tar.gz"), ("windows", "zip")]]
EXPECTED_MACOS_MEMBERS = {
    "aster",
    "aster-acp",
    "BUILD_ID",
    "SHA256SUMS",
    "embedding/potion-code-16M-v2.safetensors",
    "embedding/potion-code-16M-v2.tokenizer.json",
    "embedding/potion-code-16M-v2.config.json",
}


def write_macos_pair_archive(path, version_label):
    """Minimal pair archive that satisfies publish-aster-cli member verification."""
    with tarfile.open(path, "w:gz") as package:
        for name in sorted(EXPECTED_MACOS_MEMBERS):
            body = f"fixture {version_label} {name}".encode()
            entry = tarfile.TarInfo(name)
            entry.size = len(body)
            entry.mode = 0o755
            package.addfile(entry, io.BytesIO(body))


class ReleaseAssembly(unittest.TestCase):
    def assemble(self, missing=False, extra=False, existing_draft=False, draft=True, draft_sha=SOURCE_SHA,
                 artifact_run_id="123", branch_qualification=1,
                 version="0.1.6", source_clean=True,
                 checkout_sha=None, source_status_exit=0, core_version="9.2.3",
                 macos_members=None):
        lines = (ROOT / ".github/workflows/release-aster-cli.yml").read_text().splitlines()
        start = lines.index("      - name: Assemble draft aster-cli")
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
            scripts = root / "scripts"
            scripts.mkdir()
            (scripts / "publish-aster-cli.sh").write_text(
                (ROOT / "scripts/publish-aster-cli.sh").read_text())
            for package, package_version in (("apps/aster", version), ("crates/code/acp", "9.2.3"),
                                             ("crates/code/core", core_version)):
                path = root / package
                path.mkdir(parents=True)
                (path / "Cargo.toml").write_text(f'[package]\nversion = "{package_version}"\n')
                (path / "manifest.json").write_text(json.dumps({"version": package_version}))
            names = [name.replace("0.1.6", version) for name in NAMES]
            for name in names:
                path = dist / name
                if name.endswith("-macos.tar.gz"):
                    if macos_members is None:
                        write_macos_pair_archive(path, name)
                    else:
                        with tarfile.open(path, "w:gz") as package:
                            for member in sorted(macos_members):
                                body = f"fixture {member}".encode()
                                entry = tarfile.TarInfo(member)
                                entry.size = len(body)
                                entry.mode = 0o755
                                package.addfile(entry, io.BytesIO(body))
                else:
                    path.write_bytes(("opaque fixture " + name).encode())
            if missing:
                (dist / names[-1]).unlink()
            if extra:
                (dist / "aster-cli-v0.1.6-kylin.tar.gz").write_bytes(b"retired fixture")
            gh = commands / "gh"
            gh.write_text('''#!/bin/sh
case "$1:$2" in
  api:repos/fixture/aster/actions/runs/123) printf '%s\\n' "$TEST_SOURCE_SHA" ;;
  api:repos/fixture/aster/actions/workflows/qualify-aster-cli.yml/runs*)
    test "$2" = "repos/fixture/aster/actions/workflows/qualify-aster-cli.yml/runs?head_sha=$TEST_QUALIFIED_SHA&event=push&status=completed&per_page=100" || exit 92
    printf '%s\\n' "$TEST_BRANCH_QUALIFICATION" ;;
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
            git = commands / "git"
            git.write_text('''#!/bin/sh
case "$1" in
  rev-parse) printf '%s\\n' "$TEST_CHECKOUT_SHA" ;;
  status)
    test "$TEST_SOURCE_STATUS_EXIT" = 0 || exit "$TEST_SOURCE_STATUS_EXIT"
    test "$TEST_SOURCE_CLEAN" = 1 || printf ' M apps/aster/src/main.rs\\n' ;;
  *) exit 93 ;;
esac
''')
            git.chmod(0o755)
            cargo = commands / "cargo"
            cargo.write_text('''#!/bin/sh
test "$1" = read-manifest && test "$2" = --manifest-path || exit 94
cat "${3%Cargo.toml}manifest.json"
''')
            cargo.chmod(0o755)
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
                   "TEST_QUALIFIED_SHA": SOURCE_SHA if artifact_run_id else "b" * 40,
                   "TEST_EXISTING_DRAFT": "1" if existing_draft else "0",
                   "TEST_IS_DRAFT": "true" if draft else "false", "TEST_DRAFT_SHA": draft_sha,
                   "TEST_BRANCH_QUALIFICATION": str(branch_qualification),
                   "TEST_CHECKOUT_SHA": checkout_sha or (SOURCE_SHA if artifact_run_id else "b" * 40),
                   "TEST_SOURCE_CLEAN": "1" if source_clean else "0",
                   "TEST_SOURCE_STATUS_EXIT": str(source_status_exit)}
            result = subprocess.run(["bash", "-c", script], cwd=root, env=env, text=True, capture_output=True, timeout=10)
            manifest = json.loads((dist / "latest.json").read_text()) if (dist / "latest.json").exists() else None
            args = receipt.read_text().splitlines() if receipt.exists() else None
            return result, manifest, args

    def test_new_source_version_controls_every_published_asset_and_manifest(self):
        result, manifest, args = self.assemble(version="0.2.3")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(manifest["version"], "0.2.3")
        self.assertEqual(manifest["tag"], "aster-cli-v0.2.3")
        for entry in manifest["assets"].values():
            self.assertIn("aster-cli-v0.2.3-", entry["name"])
            self.assertIn("dist/" + entry["name"], args)

    def test_dirty_or_different_checkout_never_creates_or_uploads_a_release(self):
        for flags in ({"source_clean": False}, {"checkout_sha": "c" * 40}):
            with self.subTest(**flags):
                result, manifest, args = self.assemble(**flags)
                self.assertNotEqual(result.returncode, 0)
                self.assertIsNone(manifest)
                self.assertIsNone(args)

    def test_failed_source_status_check_never_creates_or_uploads_a_release(self):
        result, manifest, args = self.assemble(source_status_exit=37)
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertIsNone(manifest)
        self.assertIsNone(args)

    def test_invalid_cli_or_mismatched_core_versions_refuse_publication(self):
        for flags in ({"version": "0.1.6-beta"}, {"core_version": "9.2.4"}):
            with self.subTest(**flags):
                result, manifest, args = self.assemble(**flags)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("invalid CLI version or mismatched Core/ACP versions", result.stderr)
                self.assertIsNone(manifest)
                self.assertIsNone(args)

    def test_three_archives_form_a_draft_bound_to_the_artifact_source(self):
        result, manifest, args = self.assemble()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(manifest["version"], "0.1.6")
        self.assertEqual(manifest["tag"], "aster-cli-v0.1.6")
        self.assertEqual(set(manifest["assets"]), {"macos", "linux", "linux-arm64", "windows"})
        self.assertEqual(args[args.index("--target") + 1], SOURCE_SHA)
        self.assertIn("--draft", args)
        for entry in manifest["assets"].values():
            name = entry["name"]
            self.assertIn(name, NAMES)
            self.assertIn("dist/" + name, args)
            if name.endswith("-macos.tar.gz"):
                self.assertEqual(len(entry["sha256"]), 64)
                self.assertTrue(all(c in "0123456789abcdef" for c in entry["sha256"]))
            else:
                self.assertEqual(
                    entry["sha256"],
                    hashlib.sha256(("opaque fixture " + name).encode()).hexdigest(),
                )

    def test_missing_or_retired_archives_refuse_before_release_creation(self):
        for missing, extra in [(True, False), (False, True)]:
            with self.subTest(missing=missing, extra=extra):
                result, manifest, args = self.assemble(missing=missing, extra=extra)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("expected 4 release archives", result.stderr)
                self.assertIsNone(manifest)
                self.assertIsNone(args)

    def test_normal_build_qualifies_and_binds_the_draft_to_its_own_source(self):
        result, manifest, args = self.assemble(artifact_run_id="")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(args[args.index("--target") + 1], "b" * 40)
        self.assertEqual(set(manifest["assets"]), {"macos", "linux", "linux-arm64", "windows"})

    def test_matching_existing_draft_reuses_its_source_without_a_tag_lookup(self):
        result, manifest, args = self.assemble(existing_draft=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(args[:2], ["release", "upload"])
        self.assertIn("--clobber", args)
        self.assertEqual(set(manifest["assets"]), {"macos", "linux", "linux-arm64", "windows"})

    def test_published_or_different_source_release_refuses_upload(self):
        for draft, source in [(False, SOURCE_SHA), (True, "b" * 40)]:
            with self.subTest(draft=draft, source=source):
                result, _, args = self.assemble(existing_draft=True, draft=draft, draft_sha=source)
                self.assertNotEqual(result.returncode, 0)
                self.assertIsNone(args)

    def test_unexpected_macos_archive_members_block_create_and_upload(self):
        for members in (
            EXPECTED_MACOS_MEMBERS | {"sandbox/aster-oci"},
            EXPECTED_MACOS_MEMBERS - {"BUILD_ID"},
            {"aster", "aster-acp"},
        ):
            with self.subTest(members=sorted(members)):
                result, manifest, args = self.assemble(macos_members=members)
                self.assertNotEqual(result.returncode, 0, result.stderr)
                self.assertIn("unexpected archive members", result.stderr)
                self.assertIsNone(manifest)
                self.assertIsNone(args)

    def test_normal_and_reused_artifacts_require_branch_qualification_for_their_own_source(self):
        for artifact_run_id in ("", "123"):
            for existing_draft in (False, True):
                with self.subTest(artifact_run_id=artifact_run_id, existing_draft=existing_draft):
                    result, manifest, args = self.assemble(existing_draft=existing_draft,
                        artifact_run_id=artifact_run_id, branch_qualification=0)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("no successful branch qualification", result.stderr)
                    self.assertIsNone(manifest)
                    self.assertIsNone(args)

    def test_windows_build_uses_a_short_cargo_home(self):
        workflow = (ROOT / ".github/workflows/release-aster-cli.yml").read_text()
        self.assertIn(
            "      - name: Shorten Windows Cargo home\n        if: runner.os == 'Windows'",
            workflow,
        )
        self.assertIn("CARGO_HOME=D:\\c", workflow)


class ReleaseWorkflowContract(unittest.TestCase):
    def test_tag_builds_require_premerge_qualification_without_repeating_it(self):
        text = (ROOT / ".github/workflows/release-aster-cli.yml").read_text()
        self.assertIn('tags:\n      - "aster-cli-v[0-9]+.[0-9]+.[0-9]+"', text)
        self.assertIn("name: Verify pre-merge qualification", text)
        self.assertIn("actions/workflows/qualify-aster-cli.yml/runs?head_sha=${RELEASE_SHA}&event=push&status=completed", text)
        self.assertIn(".head_branch !=", text)
        # Platform builds start from preflight so they overlap qualify polling.
        self.assertEqual(text.count("needs: preflight"), 4)
        self.assertIn("needs: [preflight, qualified, build, build-macos, build-linux]", text)
        self.assertIn("name: macos-bins", text)
        self.assertNotIn("name: macos-sandbox", text)
        self.assertIn("ASTER_QUALIFY_WAIT_SECONDS", text)
        self.assertIn("python3 tests/release_governance.py", text)
        self.assertNotIn("cargo fmt", text)
        self.assertNotIn("cargo test", text)
        self.assertIn("bash scripts/publish-aster-cli.sh", text)
        self.assertIn("--profile \"$CLI_PROFILE\"", text)
        self.assertIn("CLI_PROFILE: release-dist", text)
        publisher = (ROOT / "scripts/publish-aster-cli.sh").read_text()
        self.assertNotIn("cli_sandbox_smoke", publisher)
        self.assertIn("embedding/potion-code-16M-v2.safetensors", publisher)
        self.assertIn("verified archive members for", publisher)
        self.assertIn("unexpected archive members", publisher)
        publish_job = text.split("\n  publish:\n", 1)[1]
        self.assertIn("runs-on: ${{ inputs.qualified_runner }}", publish_job)
        self.assertIn("inputs.qualified_runner != ''", publish_job)
        self.assertIn("needs.preflight.result == 'success'", publish_job)
        self.assertIn("needs.build-macos.result == 'success'", publish_job)

    def test_all_three_cli_platforms_have_product_specific_caches(self):
        text = (ROOT / ".github/workflows/release-aster-cli.yml").read_text()
        self.assertEqual(text.count("uses: Swatinem/rust-cache@"), 3)
        self.assertIn("shared-key: aster-cli-release-dist-windows", text)
        self.assertIn("shared-key: aster-cli-release-dist-macos-bins", text)
        self.assertNotIn("shared-key: aster-cli-release-dist-macos-sandbox", text)
        self.assertIn("cache_key: aster-cli-release-dist-linux", text)
        self.assertIn("cache_key: aster-cli-release-dist-linux-arm64", text)
        self.assertIn("shared-key: ${{ matrix.cache_key }}", text)
        self.assertIn("CLI_ASSET_NAME: ${{ github.ref_name }}-windows.zip", text)
        self.assertIn("CLI_ASSET_NAME: ${{ github.ref_name }}-macos.tar.gz", text)
        self.assertIn("CLI_ASSET_NAME: ${{ github.ref_name }}-${{ matrix.platform }}.tar.gz", text)
        self.assertIn('package-aster-cli.sh "$CLI_PLATFORM" "$bin_dir" "$CLI_ASSET_NAME"', text)
        self.assertIn('run-id: ${{ inputs.artifact_run_id != \'\' && inputs.artifact_run_id || github.run_id }}', text)

    def test_premerge_cli_workflow_runs_tests_and_delivery_contracts(self):
        text = (ROOT / ".github/workflows/qualify-aster-cli.yml").read_text()
        self.assertIn('push:\n    branches: ["**"]\n', text)
        self.assertIn("pull_request:", text)
        self.assertIn("paths:", text)
        self.assertIn("Aster CLI tests (ubuntu-latest)", text)
        self.assertIn("runs-on: ubuntu-latest", text)
        self.assertNotIn("runs-on: macos-latest", text)
        self.assertNotIn("matrix:", text)
        self.assertIn("cargo build --locked -p aster -p aster-acp", text)
        self.assertIn("cargo test --locked -p aster --bin aster -- --test-threads=4", text)
        self.assertIn("python3 tests/cli_pair_delivery.py", text)
        self.assertIn("python3 tests/aster_cli_linux.py", text)
        self.assertIn("python3 tests/cli_release_workflow.py", text)
        self.assertIn("shared-key: aster-cli-qualify-ubuntu-latest", text)
        self.assertNotIn("CARGO_BUILD_JOBS", text)


if __name__ == "__main__":
    unittest.main()
