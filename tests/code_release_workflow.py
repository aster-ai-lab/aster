import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from release_governance import workflow_script


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github" / "workflows"
WORKFLOW = WORKFLOWS / "release-code.yml"
QUALIFY = WORKFLOWS / "qualify-code.yml"
CI = WORKFLOWS / "ci.yml"


class CodeReleaseWorkflowTest(unittest.TestCase):
    def test_core_acp_release_does_not_build_test_or_package_product_apps(self):
        text = WORKFLOW.read_text()
        self.assertIn("cargo build --locked --release -p aster-acp --bin aster-acp", text)
        self.assertNotIn("cargo test", text)
        self.assertNotIn("cargo fmt", text)
        self.assertNotIn("cargo build --locked --release -p aster", text)
        self.assertNotIn("package-aster", text)
        self.assertNotIn("release-aster-desktop", text)
        self.assertNotIn("release-aster-cli", text)

    def test_tag_publish_waits_for_qualification_and_all_cached_code_lanes(self):
        text = WORKFLOW.read_text()
        self.assertIn('tags:\n      - "aster-v[0-9]+.[0-9]+.[0-9]+"', text)
        self.assertIn("needs: qualified", text)
        self.assertIn("needs: build", text)
        self.assertIn("actions/workflows/qualify-code.yml/runs?head_sha=${RELEASE_SHA}&event=push&status=completed", text)
        self.assertIn(".head_branch !=", text)
        self.assertIn("if: github.ref_type == 'tag'", text)
        for platform in ["macos", "linux", "windows"]:
            self.assertIn(f"platform: {platform}", text)
        self.assertIn("shared-key: code-release-${{ matrix.platform }}", text)
        self.assertEqual(text.count("uses: Swatinem/rust-cache@"), 1)
        self.assertIn("expected 3 ACP release archives", text)
        self.assertIn("--verify-tag", text)
        self.assertIn('--notes-file "$notes"', text)
        self.assertIn('notes="crates/code/manual/RELEASE_${version}.md"', text)
        self.assertIn("--latest=false", text)
        self.assertNotIn("RELEASE_9.1.6.md", text)
        self.assertEqual(text.count("CARGO_HOME=D:\\c"), 1)

    def test_premerge_qualification_retains_coverage_and_ignores_release_tags(self):
        for workflow in [CI, QUALIFY]:
            self.assertIn('push:\n    branches: ["**"]\n  pull_request:', workflow.read_text())
        text = QUALIFY.read_text()
        self.assertIn("name: Test ACP unit and wire contracts", text)
        self.assertIn("cargo test --locked -p aster-acp --lib -- --test-threads=1", text)
        self.assertIn("cargo test --locked -p aster-acp --test acp_conformance_stdio", text)
        self.assertIn("Core/ACP portable tests (windows-latest)", text)
        self.assertIn("agent::tests::unsupported_prompt_content_fails_closed_before_a_turn", text)
        self.assertEqual(text.count("uses: Swatinem/rust-cache@"), 2)


class QualificationGate(unittest.TestCase):
    def qualify(self, workflow, *, count=1, expected_sha=None):
        script = workflow_script(workflow, "Require successful branch qualification for the tagged commit")
        qualifier = "qualify-aster-cli.yml" if workflow == "release-aster-cli.yml" else "qualify-code.yml"
        source = "a" * 40
        with tempfile.TemporaryDirectory(prefix="aster-qualifier-") as directory:
            commands = Path(directory)
            gh = commands / "gh"
            gh.write_text('''#!/bin/sh
set -eu
test "$1" = api
test "$2" = "$EXPECTED_URL"
printf '%s\\n' "$QUALIFIED_COUNT"
''')
            gh.chmod(0o755)
            env = os.environ.copy()
            env.update({
                "PATH": directory + os.pathsep + env["PATH"],
                "GH_REPO": "fixture/aster", "GH_TOKEN": "test",
                "RELEASE_SHA": source, "RELEASE_TAG": "fixture-tag",
                "QUALIFIED_COUNT": str(count),
                "EXPECTED_URL": f"repos/fixture/aster/actions/workflows/{qualifier}/runs?head_sha={expected_sha or source}&event=push&status=completed&per_page=100",
            })
            return subprocess.run(["bash", "-c", script], env=env, text=True, capture_output=True, timeout=10)

    def test_both_release_gates_require_a_successful_run_for_the_exact_source(self):
        for workflow in ["release-aster-cli.yml", "release-code.yml"]:
            with self.subTest(workflow=workflow):
                result = self.qualify(workflow)
                self.assertEqual(result.returncode, 0, result.stderr)
                result = self.qualify(workflow, count=0)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("no successful pre-merge", result.stderr)
                self.assertNotEqual(self.qualify(workflow, expected_sha="b" * 40).returncode, 0)


class CodePublication(unittest.TestCase):
    def publish(self, *, tag="aster-v9.1.6", core="9.1.6", notes=True, wrong_asset=False):
        script = workflow_script("release-code.yml", "Publish versioned release")
        with tempfile.TemporaryDirectory(prefix="aster-release-") as directory:
            root = Path(directory)
            commands, dist = root / "commands", root / "dist"
            commands.mkdir()
            dist.mkdir()
            for package, version in [("core", core), ("acp", "9.1.6")]:
                manifest = root / "crates/code" / package / "Cargo.toml"
                manifest.parent.mkdir(parents=True)
                manifest.write_text(f'[package]\nversion = "{version}"\n')
            note = root / "crates/code/manual/RELEASE_9.1.6.md"
            note.parent.mkdir()
            if notes:
                note.write_text("Qualified source notes.\n")
            for platform, suffix in [("macos", "tar.gz"), ("linux", "tar.gz"), ("windows", "zip")]:
                name = f"aster-acp-v9.1.6-{platform}.{suffix}"
                if wrong_asset and platform == "windows":
                    name = "retired-product.zip"
                (dist / name).write_bytes(b"opaque archive fixture")
            receipt = root / "create.args"
            gh = commands / "gh"
            gh.write_text('''#!/bin/sh
case "$1:$2" in
  release:create) printf '%s\\n' "$@" > "$CREATE_ARGS" ;;
  release:view) : ;;
  *) exit 91 ;;
esac
''')
            gh.chmod(0o755)
            (commands / "python3").symlink_to(sys.executable)
            checksum = commands / "sha256sum"
            checksum.write_text(f'#!{sys.executable}\nimport hashlib,pathlib,sys\nfor name in sys.argv[1:]:\n print(hashlib.sha256(pathlib.Path(name).read_bytes()).hexdigest()+"  "+name)\n')
            checksum.chmod(0o755)
            env = os.environ.copy()
            env.update({"PATH": str(commands) + os.pathsep + env["PATH"],
                        "GH_TOKEN": "test", "GITHUB_REF_NAME": tag, "CREATE_ARGS": str(receipt)})
            result = subprocess.run(["bash", "-c", script], cwd=root, env=env, text=True, capture_output=True, timeout=10)
            return result, receipt.read_text().splitlines() if receipt.exists() else None

    def test_source_versions_and_release_notes_bind_the_core_publication(self):
        result, args = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--latest=false", args)
        self.assertIn("--verify-tag", args)
        self.assertEqual(args[args.index("--notes-file") + 1], "crates/code/manual/RELEASE_9.1.6.md")
        for options in [{"tag": "v9.1.6"}, {"core": "9.1.5"}, {"notes": False}, {"wrong_asset": True}]:
            with self.subTest(options=options):
                result, args = self.publish(**options)
                self.assertNotEqual(result.returncode, 0)
                self.assertIsNone(args)


if __name__ == "__main__":
    unittest.main()
