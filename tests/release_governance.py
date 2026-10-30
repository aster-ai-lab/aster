"""Exercise release namespace and CLI channel contracts without GitHub access."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.1.4"
SOURCE_SHA = "a" * 40


def workflow_script(name, step):
    lines = (ROOT / ".github/workflows" / name).read_text().splitlines()
    start = lines.index(f"      - name: {step}")
    start = lines.index("        run: |", start) + 1
    end = start
    while end < len(lines) and (not lines[end].strip() or lines[end].startswith("          ")):
        end += 1
    return textwrap.dedent("\n".join(lines[start:end]))


def manifest(version=VERSION):
    assets = {}
    for platform, extension in (("macos", "tar.gz"), ("linux", "tar.gz"), ("windows", "zip")):
        name = f"aster-cli-v{version}-{platform}.{extension}"
        assets[platform] = {
            "name": name,
            "sha256": hashlib.sha256(name.encode()).hexdigest(),
        }
    return {"version": version, "tag": f"aster-cli-v{version}", "assets": assets}


class CliChannelPublication(unittest.TestCase):
    def publish(self, *, draft=False, prerelease=False, stable_exists=True,
                latest_tag="v0.1.4", manifest_body=None):
        script = workflow_script(
            "release-aster-cli-channel.yml", "Publish a verified CLI channel"
        )
        with tempfile.TemporaryDirectory(prefix="aster-channel-") as directory:
            root = Path(directory)
            commands = root / "commands"
            commands.mkdir()
            source_manifest = root / "source-manifest.json"
            source_manifest.write_text(json.dumps(manifest_body or manifest()))
            log = root / "mutations.log"
            gh = commands / "gh"
            gh.write_text('''#!/bin/sh
set -eu
case "$1:$2" in
  release:view)
    if [ "$3" = aster-cli-stable ]; then
      test "$TEST_STABLE_EXISTS" = 1
      exit
    fi
    case "${5:-}" in
      isDraft) printf '%s\\n' "$TEST_DRAFT" ;;
      isPrerelease) printf '%s\\n' "$TEST_PRERELEASE" ;;
      *) exit 92 ;;
    esac ;;
  release:download)
    output=""
    previous=""
    for argument in "$@"; do
      if [ "$previous" = --output ]; then output="$argument"; fi
      previous="$argument"
    done
    test -n "$output"
    cp "$TEST_MANIFEST" "$output" ;;
  api:repos/fixture/aster/releases/latest) printf '%s\\n' "$TEST_LATEST_TAG" ;;
  api:repos/fixture/aster/commits/aster-cli-v0.1.4) printf '%s\\n' "$TEST_SOURCE_SHA" ;;
  release:create|release:upload) printf '%s\\n' "$*" >> "$TEST_LOG" ;;
  *) exit 91 ;;
esac
''')
            gh.chmod(0o755)
            env = os.environ.copy()
            env.update({
                "PATH": str(commands) + os.pathsep + env["PATH"],
                "GH_REPO": "fixture/aster",
                "GH_TOKEN": "test",
                "VERSION": VERSION,
                "TEST_DRAFT": str(draft).lower(),
                "TEST_PRERELEASE": str(prerelease).lower(),
                "TEST_STABLE_EXISTS": "1" if stable_exists else "0",
                "TEST_LATEST_TAG": latest_tag,
                "TEST_MANIFEST": str(source_manifest),
                "TEST_SOURCE_SHA": SOURCE_SHA,
                "TEST_LOG": str(log),
            })
            result = subprocess.run(
                ["bash", "-c", script], cwd=root, env=env,
                text=True, capture_output=True, timeout=10,
            )
            mutations = log.read_text().splitlines() if log.exists() else []
            return result, mutations

    def test_published_version_updates_stable_and_legacy_latest(self):
        result, mutations = self.publish()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(mutations), 2)
        self.assertIn("release upload aster-cli-stable latest.json --clobber", mutations[0])
        self.assertIn("release upload v0.1.4 latest.json --clobber", mutations[1])

    def test_missing_stable_is_created_without_taking_latest(self):
        result, mutations = self.publish(stable_exists=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("release create aster-cli-stable latest.json", mutations[0])
        self.assertIn(f"--target {SOURCE_SHA}", mutations[0])
        self.assertIn("--latest=false", mutations[0])

    def test_draft_and_prerelease_never_mutate_a_channel(self):
        for flags in ({"draft": True}, {"prerelease": True}):
            with self.subTest(**flags):
                result, mutations = self.publish(**flags)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(mutations, [])

    def test_mismatched_manifest_never_mutates_a_channel(self):
        body = manifest()
        body["tag"] = "aster-cli-v9.9.9"
        result, mutations = self.publish(manifest_body=body)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("identity mismatch", result.stderr)
        self.assertEqual(mutations, [])

    def test_non_desktop_latest_is_refused_before_channel_mutation(self):
        for tag in ("a3s-code-v9.1.6", "aster-cli-v0.1.4", "aster-cli-stable"):
            with self.subTest(tag=tag):
                result, mutations = self.publish(latest_tag=tag)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("repository Latest is not an Aster desktop release", result.stderr)
                self.assertEqual(mutations, [])


class ReleaseContracts(unittest.TestCase):
    def test_default_cli_discovery_pins_the_stable_channel(self):
        stable = "releases/download/aster-cli-stable/latest.json"
        legacy = "releases/latest/download/latest.json"
        for name in ("scripts/install.sh", "scripts/install.ps1"):
            text = (ROOT / name).read_text()
            self.assertIn(stable, text)
            self.assertNotIn(legacy, text)
        updater = (ROOT / "apps/aster/src/cli_pair_update.rs").read_text()
        self.assertIn('{OFFICIAL_RELEASES}/download/aster-cli-stable/{MANIFEST_NAME}', updater)
        self.assertNotIn('{OFFICIAL_RELEASES}/latest/download/{MANIFEST_NAME}', updater)

    def test_code_release_uses_its_namespace_and_never_takes_latest(self):
        workflow = (ROOT / ".github/workflows/release-code.yml").read_text()
        self.assertIn('      - "a3s-code-v[0-9]+.[0-9]+.[0-9]+"', workflow)
        self.assertNotIn('      - "v[0-9]+.[0-9]+.[0-9]+"', workflow)
        self.assertIn("gh release create", workflow)
        self.assertIn("--latest=false", workflow)
        self.assertIn('version="${GITHUB_REF_NAME#a3s-code-v}"', workflow)
        self.assertIn('test "$version" = "$acp_version"', workflow)
        self.assertIn('test "$version" = "$core_version"', workflow)

    def test_desktop_rolling_channel_never_takes_latest(self):
        workflow = (ROOT / ".github/workflows/release-aster-desktop.yml").read_text()
        command = workflow.split("gh release create desktop-stable", 1)[1].split("\n          fi", 1)[0]
        self.assertIn("--latest=false", command)

    def test_ci_and_cli_preflight_run_release_governance(self):
        for name in (".github/workflows/ci.yml", ".github/workflows/release-aster-cli.yml"):
            self.assertIn("python3 tests/release_governance.py", (ROOT / name).read_text())


if __name__ == "__main__":
    unittest.main()
