"""One standard Linux archive, with a verified glibc 2.31 ceiling."""

from __future__ import annotations

import hashlib
import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHECK = ROOT / "scripts/check-glibc-symbols.py"
PACKAGE = ROOT / "scripts/package-aster-cli.sh"
INSTALL = ROOT / "scripts/install.sh"
IDENTITY = "aster-cli-pair-v1:fixture-source:fixture-target"
NAMES = ("aster", "a3s-code-acp")
VERSION = "0.1.2"


def _load_check():
    spec = importlib.util.spec_from_file_location("check_glibc_symbols", CHECK)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


check = _load_check()

# Shaped like the version-needs section readelf prints for a dynamic binary.
DUMP_231 = """
Version needs section '.gnu.version_r' contains 2 entries:
  0x0000: Version: 1  File: libc.so.6  Cnt: 2
  0x0010:   Name: GLIBC_2.14  Flags: none  Version: 5
  0x0020:   Name: GLIBC_2.2.5  Flags: none  Version: 2
"""

DUMP_238 = """
Version needs section '.gnu.version_r' contains 2 entries:
  0x0000: Version: 1  File: libc.so.6  Cnt: 3
  0x0010:   Name: GLIBC_2.34  Flags: none  Version: 6
  0x0020:   Name: GLIBC_2.2.5  Flags: none  Version: 2
"""


class GlibcCeilingTest(unittest.TestCase):
    def test_versions_are_compared_numerically_not_lexically(self):
        # "2.9" sorts after "2.31" as text and is genuinely older as a version.
        self.assertEqual(check.evaluate({"GLIBC_2.9"}, "2.31"), [])
        self.assertEqual(check.evaluate({"GLIBC_2.30"}, "2.31"), [])
        self.assertEqual(check.evaluate({"GLIBC_2.31"}, "2.31"), [])

    def test_a_newer_requirement_fails_the_ceiling(self):
        self.assertEqual(check.evaluate({"GLIBC_2.34"}, "2.31"), ["GLIBC_2.34"])
        self.assertEqual(
            check.evaluate({"GLIBC_2.17", "GLIBC_2.34", "GLIBC_2.2.5"}, "2.31"),
            ["GLIBC_2.34"],
        )

    def test_unprovable_labels_fail_instead_of_passing_silently(self):
        self.assertEqual(check.evaluate({"GLIBC_ABI_DT_RELR"}, "2.31"), ["GLIBC_ABI_DT_RELR"])
        self.assertEqual(check.evaluate({"GLIBC_FUTURE"}, "2.31"), ["GLIBC_FUTURE"])
        # Internal to glibc, never a loader requirement of an ordinary program.
        self.assertEqual(check.evaluate({"GLIBC_PRIVATE"}, "2.31"), [])

    def test_dump_parsing_reads_the_version_needs_section(self):
        self.assertEqual(
            check.parse_versions(DUMP_231), {"GLIBC_2.14", "GLIBC_2.2.5"}
        )
        self.assertEqual(
            check.parse_versions(DUMP_238), {"GLIBC_2.34", "GLIBC_2.2.5"}
        )

    def test_cli_reports_a_violation_as_a_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            dump = Path(directory) / "needs.txt"
            dump.write_text(DUMP_238)
            completed = subprocess.run(
                [sys.executable, str(CHECK), "--max", "2.31", "--dump", str(dump)],
                capture_output=True,
                text=True,
            )
        self.assertEqual(completed.returncode, 1, completed.stdout)
        self.assertIn("GLIBC_2.34", completed.stderr)

    def test_cli_passes_a_dump_within_the_ceiling(self):
        with tempfile.TemporaryDirectory() as directory:
            dump = Path(directory) / "needs.txt"
            dump.write_text(DUMP_231)
            completed = subprocess.run(
                [sys.executable, str(CHECK), "--max", "2.31", "--dump", str(dump)],
                capture_output=True,
                text=True,
            )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_an_unreadable_ceiling_is_an_error_not_a_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            dump = Path(directory) / "needs.txt"
            dump.write_text(DUMP_231)
            completed = subprocess.run(
                [sys.executable, str(CHECK), "--max", "kylin", "--dump", str(dump)],
                capture_output=True,
                text=True,
            )
        self.assertEqual(completed.returncode, 2, completed.stdout)

    def test_a_missing_binary_is_an_error_not_a_pass(self):
        completed = subprocess.run(
            [sys.executable, str(CHECK), "--max", "2.31", "/nonexistent/aster"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 2, completed.stdout)


class LinuxPackagingTest(unittest.TestCase):
    def package_fixture(self, dump):
        with tempfile.TemporaryDirectory(prefix="aster-linux-package-") as directory:
            root = Path(directory)
            bins, commands = root / "bins", root / "commands"
            bins.mkdir()
            commands.mkdir()
            for name in NAMES:
                path = bins / name
                path.write_text(f"#!/bin/sh\nprintf '%s\\n' '{IDENTITY}'\n")
                path.chmod(0o755)
            reader = commands / "readelf"
            reader.write_text('#!/bin/sh\nprintf "%s\\n" "$@" >> "$FAKE_READELF_LOG"\nprintf "%s\\n" "$FAKE_ABI_DUMP"\n')
            reader.chmod(0o755)
            trace = root / "readelf.log"
            archive = root / "linux.tar.gz"
            env = dict(os.environ, PATH=str(commands) + os.pathsep + os.environ["PATH"], FAKE_READELF_LOG=str(trace), FAKE_ABI_DUMP=dump)
            result = subprocess.run(["bash", str(PACKAGE), "linux", str(bins), str(archive)], env=env, capture_output=True, text=True, timeout=10)
            return result, archive.exists(), trace.read_text() if trace.exists() else ""

    def test_linux_packaging_checks_both_binaries_against_the_baseline(self):
        result, exists, trace = self.package_fixture(DUMP_231)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(exists)
        self.assertIn("--version-info", trace)
        for name in NAMES:
            self.assertIn("/" + name + "\n", trace)

    def test_linux_packaging_refuses_a_newer_glibc_requirement(self):
        result, exists, _ = self.package_fixture(DUMP_238)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("GLIBC_2.34", result.stderr)
        self.assertFalse(exists)

    def test_retired_and_unknown_platforms_are_refused(self):
        for platform in ("kylin", "plan9"):
            with self.subTest(platform=platform):
                result = subprocess.run(["bash", str(PACKAGE), platform, "/missing", "/missing/out.tar.gz"], capture_output=True, text=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("unsupported platform", result.stderr)


class LinuxChannelSelectionTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="aster-linux-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.assets, self.prefix, self.fake = (self.root / name for name in ("assets", "prefix", "fake"))
        for path in (self.assets, self.prefix, self.fake):
            path.mkdir()
        digest = "0" * 64
        (self.assets / "latest.json").write_text(
            f'{{"version":"{VERSION}","tag":"aster-cli-v{VERSION}","assets":{{"linux":{{"name":"aster-cli-v{VERSION}-linux.tar.gz","sha256":"{digest}"}}}}}}\n'
        )
        self.fake_tool("uname", 'printf "%s\\n" Linux')

    def fake_tool(self, name, body):
        path = self.fake / name
        path.write_text("#!/bin/sh\n" + body + "\n")
        path.chmod(0o755)

    def environment(self):
        env = dict(os.environ, ASTER_RELEASE_BASE=str(self.assets), PATH=str(self.fake) + os.pathsep + os.environ["PATH"])
        env.pop("ASTER_INSTALL_URL", None)
        env.pop("GROK_INSTALL_URL", None)
        return env

    def test_linux_hosts_request_the_same_archive_without_a_special_channel(self):
        for version in ("2.31", "2.39", None):
            with self.subTest(reported_glibc=version):
                self.fake_tool("getconf", f'printf "%s\\n" "glibc {version}"' if version else "exit 1")
                self.fake_tool("ldd", "exit 1")
                result = subprocess.run(["bash", str(INSTALL), "--prefix", str(self.prefix)], env=self.environment(), capture_output=True, text=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(f"local release is missing aster-cli-v{VERSION}-linux.tar.gz", result.stderr)

    def test_retired_direct_archive_is_refused_before_network_access(self):
        marker = self.root / "network-called"
        self.fake_tool("curl", 'printf called > "$NETWORK_MARKER"; exit 99')
        env = self.environment()
        env["NETWORK_MARKER"] = str(marker)
        result = subprocess.run(["bash", str(INSTALL), "--prefix", str(self.prefix), f"https://github.com/aster-ai-lab/aster/releases/download/aster-cli-v{VERSION}/aster-cli-v{VERSION}-kylin.tar.gz"], env=env, capture_output=True, text=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not download that binary", result.stderr)
        self.assertFalse(marker.exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
