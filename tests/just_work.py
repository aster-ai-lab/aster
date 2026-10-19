import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
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


@unittest.skipUnless(os.name == "posix", "The dev recipe delegates through a POSIX shell")
class DevLauncherTest(unittest.TestCase):
    def test_root_dev_enables_renderer_hot_reload(self):
        with tempfile.TemporaryDirectory(prefix="aster-dev-test-") as directory:
            root = Path(directory)
            shutil.copy2(Path(__file__).resolve().parents[1] / "justfile", root / "justfile")
            work = root / "apps/work"
            work.mkdir(parents=True)
            shutil.copy2(Path(__file__).resolve().parents[1] / "apps/work/justfile", work / "justfile")
            script = work / "scripts/studio-shell.sh"
            script.parent.mkdir()
            script.write_text('#!/bin/sh\nprintf "%s\\n" "${STUDIO_RENDERER_DEV:-unset}" > "$ASTER_OPEN_LOG"\n')
            script.chmod(0o755)
            log = root / "renderer.txt"
            env = dict(os.environ)
            env.pop("STUDIO_RENDERER_DEV", None)
            env["ASTER_OPEN_LOG"] = str(log)
            result = subprocess.run(["just", "dev"], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(log.read_text().strip(), "1")

    def test_studio_exports_the_default_tenant(self):
        with tempfile.TemporaryDirectory(prefix="aster-studio-test-") as directory:
            root = Path(directory)
            shutil.copy2(Path(__file__).resolve().parents[1] / "justfile", root / "justfile")
            tools = root / "bin"
            tools.mkdir()
            bun = tools / "bun"
            bun.write_text('#!/bin/sh\nprintf "%s\\n" "$2" > "$ASTER_TENANT_LOG"\n')
            bun.chmod(0o755)
            script = root / "apps/studio/scripts/studio-shell.sh"
            script.parent.mkdir(parents=True)
            script.write_text(
                '#!/bin/sh\nprintf "%s %s\\n" "${A3S_TENANT_SLUG:-unset}" "${STUDIO_RENDERER_DEV:-unset}" > "$ASTER_OPEN_LOG"\n'
            )
            script.chmod(0o755)
            tenant = root / "tenant.txt"
            opened = root / "opened.txt"
            env = dict(os.environ, PATH=f"{tools}:{os.environ['PATH']}",
                       ASTER_TENANT_LOG=str(tenant), ASTER_OPEN_LOG=str(opened))
            env.pop("A3S_TENANT_SLUG", None)
            env.pop("STUDIO_RENDERER_DEV", None)
            result = subprocess.run(["just", "studio"], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(tenant.read_text().strip(), "toyou")
            self.assertEqual(opened.read_text().strip(), "toyou 1")
            result = subprocess.run(["just", "studio", "a3s"], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(opened.read_text().strip(), "a3s 1")
            result = subprocess.run(["just", "studio", "Bad"], cwd=root, env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("invalid tenant slug", result.stderr)


@unittest.skipUnless(sys.platform == "darwin", "Work launcher uses macOS process paths")
class WorkLauncherTest(unittest.TestCase):
    def prepare(self, root):
        shutil.copy2(Path(__file__).resolve().parents[1] / "justfile", root / "justfile")
        tools = root / "bin"
        tools.mkdir()
        cargo = tools / "cargo"
        cargo.write_text(
            '#!/usr/bin/env python3\nimport json, os, sys\n'
            'log = os.environ.get("ASTER_CARGO_LOG")\n'
            'if log:\n'
            '    with open(log, "a") as handle:\n'
            '        handle.write(" ".join(sys.argv[1:]) + "\\n")\n'
            'if sys.argv[1] == "metadata": print(json.dumps({"target_directory": os.environ["ASTER_TEST_TARGET"]}))\n'
            'elif sys.argv[1] == "build": raise SystemExit(int(os.environ.get("ASTER_TEST_FAIL", "0")))\n'
            'else: raise SystemExit("unexpected Cargo command")\n'
        )
        cargo.chmod(0o755)

    def work_env(self, root, **extra):
        env = dict(os.environ)
        env.update(
            PATH=f"{root / 'bin'}:{os.environ['PATH']}",
            ASTER_TEST_TARGET=str(root / "target"),
            ASTER_WORK_DRY_RUN="1",
            ASTER_WORK_SUPPORT=str(root / "empty-support"),
        )
        env.update(extra)
        return env

    def test_work_builds_then_launches_that_debug_binary(self):
        with tempfile.TemporaryDirectory(prefix="aster-work-test-") as directory:
            root = Path(directory)
            self.prepare(root)
            bundle = root / "apps/work/src-tauri/target/release/bundle/macos/New App.app/Contents/MacOS/aster-dream"
            bundle.parent.mkdir(parents=True)
            bundle.write_text("newer bundle")
            bundle.chmod(0o755)
            os.utime(bundle, (900, 900))
            binary = root / "target/debug/aster-dream"
            binary.parent.mkdir(parents=True)
            binary.write_text("cargo")
            binary.chmod(0o755)
            os.utime(binary, (100, 100))
            log = root / "opened.txt"
            cargo_log = root / "cargo.txt"
            result = subprocess.run(
                ["just", "work"], cwd=root,
                env=self.work_env(root, ASTER_OPEN_LOG=str(log), ASTER_CARGO_LOG=str(cargo_log)),
                capture_output=True, text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(Path(log.read_text().strip()), binary.resolve())
            self.assertEqual(cargo_log.read_text().splitlines(), [
                "build --locked -p a3s-code-acp -p a3s-office-cli",
                "build --locked -p aster-dream",
                "metadata --locked --offline --no-deps --format-version 1",
            ])

    def test_failed_build_does_not_launch(self):
        with tempfile.TemporaryDirectory(prefix="aster-work-fail-") as directory:
            root = Path(directory)
            self.prepare(root)
            binary = root / "target/debug/aster-dream"
            binary.parent.mkdir(parents=True)
            binary.write_text('#!/bin/sh\nprintf "started\\n" > "$ASTER_OPEN_LOG"\n')
            binary.chmod(0o755)
            log = root / "opened.txt"
            result = subprocess.run(
                ["just", "work"], cwd=root,
                env=self.work_env(root, ASTER_OPEN_LOG=str(log), ASTER_TEST_FAIL="1"),
                capture_output=True, text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(log.exists(), "failed builds must not launch a stale executable")

    def test_launch_replaces_an_older_running_build(self):
        with tempfile.TemporaryDirectory(prefix="aster-work-replace-") as directory:
            root = Path(directory)
            self.prepare(root)
            old = root / "target/release/aster-dream"
            old.parent.mkdir(parents=True)
            shutil.copy("/bin/sleep", old)
            new = root / "target/debug/aster-dream"
            new.parent.mkdir(parents=True)
            new.write_text('#!/bin/sh\nprintf "started\\n" > "$ASTER_OPEN_LOG"\n')
            new.chmod(0o755)
            log = root / "opened.txt"
            sleeper = subprocess.Popen([str(old), "30"])
            try:
                result = subprocess.run(
                    ["just", "work"], cwd=root,
                    env=self.work_env(root, ASTER_OPEN_LOG=str(log), ASTER_WORK_DRY_RUN="0"),
                    capture_output=True, text=True,
                )
                deadline = time.time() + 3
                while time.time() < deadline and (sleeper.poll() is None or not log.exists()):
                    time.sleep(0.05)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIsNotNone(sleeper.poll(), "the older AsterDream must exit")
                self.assertEqual(log.read_text().strip(), "started")
            finally:
                if sleeper.poll() is None:
                    sleeper.kill()

    def test_missing_binary_after_build_does_not_launch(self):
        with tempfile.TemporaryDirectory(prefix="aster-work-missing-") as directory:
            root = Path(directory)
            self.prepare(root)
            log = root / "opened.txt"
            result = subprocess.run(
                ["just", "work"], cwd=root,
                env=self.work_env(root, ASTER_OPEN_LOG=str(log)),
                capture_output=True, text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Build did not produce", result.stderr)
            self.assertFalse(log.exists())


if __name__ == "__main__":
    unittest.main()
