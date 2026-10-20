import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


@unittest.skipUnless(os.name == "posix", "Launcher fixtures use POSIX scripts")
class CodeLauncherTest(unittest.TestCase):
    def test_code_runs_the_built_binary_and_preserves_arguments(self):
        with tempfile.TemporaryDirectory(prefix="aster-code-test-") as directory:
            root = Path(directory)
            shutil.copy2(Path(__file__).resolve().parents[1] / "justfile", root / "justfile")
            tools = root / "bin"
            tools.mkdir()
            cargo = tools / "cargo"
            cargo.write_text(
                '#!/usr/bin/env python3\nimport json, os, sys\n'
                'if sys.argv[1] == "metadata": print(json.dumps({"target_directory": os.environ["ASTER_TEST_TARGET"]}))\n'
                'elif sys.argv[1] == "build": raise SystemExit(int(os.environ.get("ASTER_TEST_FAIL", "0")))\n'
                'else: raise SystemExit("unexpected Cargo command")\n'
            )
            cargo.chmod(0o755)
            target = root / "custom target"
            executable = target / "debug/aster"
            executable.parent.mkdir(parents=True)
            executable.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > "$ASTER_OPEN_LOG"\n')
            executable.chmod(0o755)
            log = root / "arguments.txt"
            env = dict(os.environ, PATH=f"{tools}:{os.environ['PATH']}",
                       ASTER_TEST_TARGET=str(target), ASTER_OPEN_LOG=str(log))
            result = subprocess.run(["just", "code", "--cwd", "/tmp/aster project"],
                                    cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(log.read_text().splitlines(), ["--cwd", "/tmp/aster project"])
            log.unlink()
            env["ASTER_TEST_FAIL"] = "1"
            result = subprocess.run(["just", "code"], cwd=root, env=env, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(log.exists(), "failed builds must not launch a stale executable")


@unittest.skipUnless(os.name == "posix", "Studio entry fixtures use POSIX scripts")
class StudioLauncherTest(unittest.TestCase):
    def test_default_entrypoints_open_aster_studio_without_legacy_work_process_control(self):
        for recipe in ("studio", "dev", "work"):
            with self.subTest(recipe=recipe), tempfile.TemporaryDirectory(prefix="aster-studio-entry-") as directory:
                root, env, opened = self.fixture(directory)
                result = subprocess.run(["just", recipe], cwd=root, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(opened.read_text().strip(), "aster 1")
                self.assertFalse((root / "cargo.txt").exists(), "the Studio shell owns its builds")

    def test_explicit_toyou_and_environment_selection_are_preserved(self):
        with tempfile.TemporaryDirectory(prefix="aster-studio-selection-") as directory:
            root, env, opened = self.fixture(directory)
            env["A3S_TENANT_SLUG"] = "aster"
            result = subprocess.run(["just", "studio", "toyou"], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(opened.read_text().strip(), "toyou 1")
            env["A3S_TENANT_SLUG"] = "toyou"
            result = subprocess.run(["just", "studio"], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(opened.read_text().strip(), "toyou 1")

    def test_product_build_selects_exactly_one_feature_without_default_product(self):
        for tenant in ("aster", "toyou"):
            with self.subTest(tenant=tenant), tempfile.TemporaryDirectory(prefix="aster-studio-build-") as directory:
                root, env, _ = self.fixture(directory)
                source = Path(__file__).resolve().parents[1] / "apps/studio/scripts/studio_product.py"
                helper = root / "apps/studio/scripts/studio_product.py"
                shutil.copy2(source, helper)
                for name in ("icon.icns", "icon.png", "icon.ico"):
                    icon = root / "apps/web/public/tenant" / tenant / name
                    icon.parent.mkdir(parents=True, exist_ok=True)
                    icon.write_bytes(b"fixture icon")
                cargo = root / "bin/cargo"
                cargo.write_text('#!/usr/bin/env python3\nimport json,os,sys\nwith open(os.environ["ASTER_CARGO_LOG"],"a") as out: out.write(json.dumps(sys.argv[1:])+"\\n")\n')
                result = subprocess.run(["just", "build-studio", tenant], cwd=root, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                import json
                calls = [json.loads(line) for line in (root / "cargo.txt").read_text().splitlines()]
                self.assertEqual(calls[0], ["build", "--locked", "-p", "a3s-code-acp", "-p", "a3s-office-cli"])
                self.assertEqual(calls[1], ["build", "--locked", "-p", "a3s-studio", "--no-default-features", "--features", f"tenant-{tenant}"])

    def test_invalid_tenant_refuses_before_launch(self):
        for slug in ("", " ", "Bad", "../aster"):
            with self.subTest(slug=slug), tempfile.TemporaryDirectory(prefix="aster-studio-invalid-") as directory:
                root, env, opened = self.fixture(directory)
                result = subprocess.run(["just", "studio", slug], cwd=root, env=env, capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("invalid tenant slug", result.stderr)
                self.assertFalse(opened.exists())

    def fixture(self, directory):
        root = Path(directory)
        shutil.copy2(Path(__file__).resolve().parents[1] / "justfile", root / "justfile")
        tools = root / "bin"
        tools.mkdir()
        for name, script in {
            "bun": '#!/bin/sh\nexit 0\n',
            "cargo": '#!/bin/sh\nprintf "unexpected cargo\\n" > "$ASTER_CARGO_LOG"\nexit 37\n',
        }.items():
            command = tools / name
            command.write_text(script)
            command.chmod(0o755)
        script = root / "apps/studio/scripts/studio-shell.sh"
        script.parent.mkdir(parents=True)
        script.write_text('#!/bin/sh\nprintf "%s %s\\n" "$A3S_TENANT_SLUG" "$STUDIO_RENDERER_DEV" > "$ASTER_OPEN_LOG"\n')
        script.chmod(0o755)
        opened = root / "opened.txt"
        env = dict(os.environ, PATH=f"{tools}:{os.environ['PATH']}", ASTER_OPEN_LOG=str(opened),
                   ASTER_CARGO_LOG=str(root / "cargo.txt"))
        env.pop("A3S_TENANT_SLUG", None)
        env.pop("STUDIO_RENDERER_DEV", None)
        return root, env, opened


if __name__ == "__main__":
    unittest.main()
