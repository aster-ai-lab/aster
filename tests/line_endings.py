"""Exercise repository text attributes through real Git add and checkout."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
GIT = os.environ.get("ASTER_TEST_GIT", "git")


class LineEndings(unittest.TestCase):
    def setUp(self):
        temporary_root = ROOT / "target" / "tmp"
        temporary_root.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(
            prefix="line-endings-", dir=temporary_root
        )
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.git("init", "--quiet", "-b", "fixture")
        self.git("config", "core.attributesFile", ".git/no-global-attributes")
        (self.root / ".gitattributes").write_bytes(
            (ROOT / ".gitattributes").read_bytes()
        )
        self.git("add", "--", ".gitattributes")

    def git(self, *args):
        return subprocess.check_output(
            [GIT, "-c", "core.safecrlf=false", *args],
            cwd=self.root,
            stderr=subprocess.PIPE,
        )

    def stage(self, path, content):
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(content)
        self.git("add", "--", path)
        return self.git("show", f":{path}")

    def checkout(self, path):
        # A fresh checkout must materialize from the index, not reuse the
        # existing file whose normalized content Git already considers clean.
        (self.root / path).unlink()
        self.git("checkout-index", "--", path)
        return (self.root / path).read_bytes()

    def test_utf8_sources_stage_as_lf_with_both_autocrlf_settings(self):
        content = "// 中文\nfn main() {}\n".encode("utf-8")
        for autocrlf in ("true", "false"):
            self.git("config", "core.autocrlf", autocrlf)
            for suffix in ("rs", "acl", "json", "md", "ps1"):
                with self.subTest(autocrlf=autocrlf, suffix=suffix):
                    path = f"sources/{autocrlf}/text.{suffix}"
                    self.assertEqual(
                        self.stage(path, content.replace(b"\n", b"\r\n")), content
                    )

    def test_shell_checkout_is_lf_and_runs_after_windows_style_input(self):
        content = b"#!/bin/sh\nprintf '%s\\n' LINE-ENDINGS-OK\n"
        for autocrlf in ("true", "false"):
            with self.subTest(autocrlf=autocrlf):
                self.git("config", "core.autocrlf", autocrlf)
                path = f"scripts/{autocrlf}.sh"
                self.assertEqual(
                    self.stage(path, content.replace(b"\n", b"\r\n")), content
                )
                self.git("update-index", "--chmod=+x", "--", path)
                self.assertEqual(self.checkout(path), content)
                self.assertTrue(self.git("ls-files", "--stage", "--", path).startswith(b"100755 "))
                if os.name == "posix":
                    self.assertEqual(
                        subprocess.check_output(["/bin/sh", str(self.root / path)]),
                        b"LINE-ENDINGS-OK\n",
                    )

    def test_windows_command_checkout_is_crlf_with_canonical_lf_index(self):
        content = b"@echo off\necho LINE-ENDINGS-OK\n"
        for autocrlf in ("true", "false"):
            self.git("config", "core.autocrlf", autocrlf)
            for suffix in ("bat", "cmd"):
                with self.subTest(autocrlf=autocrlf, suffix=suffix):
                    path = f"scripts/{autocrlf}.{suffix}"
                    self.assertEqual(self.stage(path, content), content)
                    self.assertEqual(
                        self.checkout(path), content.replace(b"\n", b"\r\n")
                    )

    def test_immutable_text_keeps_padding_crlf_and_final_newline_bytes(self):
        content = b'  {"bytes":"fixed"}  \r\n\r\n'
        for autocrlf in ("true", "false"):
            self.git("config", "core.autocrlf", autocrlf)
            for directory in ("docs/evidence/repository/fixture", "crates/deepseek/fixtures"):
                with self.subTest(autocrlf=autocrlf, directory=directory):
                    path = f"{directory}/{autocrlf}.json"
                    self.assertEqual(self.stage(path, content), content)
                    self.assertEqual(self.checkout(path), content)

    def test_binary_magic_and_embedded_newlines_are_unchanged(self):
        content = b"\x89PNG\r\n\x1a\n\x00binary\r\nbytes\n"
        for autocrlf in ("true", "false"):
            with self.subTest(autocrlf=autocrlf):
                self.git("config", "core.autocrlf", autocrlf)
                path = f"assets/{autocrlf}.png"
                self.assertEqual(self.stage(path, content), content)
                self.assertEqual(self.checkout(path), content)

    def test_reviewed_legacy_renormalization_leaves_other_paths_untouched(self):
        content = b"#!/bin/sh\nprintf '%s\\n' LEGACY\n"
        legacy = content.replace(b"\n", b"\r\n")
        for autocrlf in ("true", "false"):
            with self.subTest(autocrlf=autocrlf):
                self.git("config", "core.autocrlf", autocrlf)
                reviewed = f"legacy/{autocrlf}/reviewed.sh"
                unrelated = f"legacy/{autocrlf}/unrelated.sh"
                blob = subprocess.check_output(
                    [GIT, "hash-object", "-w", "--stdin"], input=legacy, cwd=self.root
                ).decode().strip()
                for path in (reviewed, unrelated):
                    destination = self.root / path
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_bytes(legacy)
                    self.git("update-index", "--add", "--cacheinfo", "100644", blob, path)
                self.git("add", "--renormalize", "--", reviewed)
                self.assertEqual(self.git("show", f":{reviewed}"), content)
                self.assertEqual(self.checkout(reviewed), content)
                self.assertEqual(self.git("show", f":{unrelated}"), legacy)
                self.assertEqual((self.root / unrelated).read_bytes(), legacy)


if __name__ == "__main__":
    unittest.main()
