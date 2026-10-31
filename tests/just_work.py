import os
import platform
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest


@unittest.skipUnless(os.name == "posix", "Launcher fixtures use POSIX scripts")
class CodeLauncherTest(unittest.TestCase):
    def test_code_runs_the_built_binary_and_preserves_arguments(self):
        with tempfile.TemporaryDirectory(prefix="aster-test-") as directory:
            root = Path(directory)
            shutil.copy2(Path(__file__).resolve().parents[1] / "justfile", root / "justfile")
            scripts = root / "scripts"
            scripts.mkdir()
            (scripts / "aster_sandbox.py").write_text(
                'import os,sys\n'
                'if sys.argv[1]=="cache": print(os.environ["ASTER_TEST_SANDBOX"])\n'
            )
            tools = root / "bin"
            tools.mkdir()
            cargo = tools / "cargo"
            cargo.write_text(
                '#!/usr/bin/env python3\nimport json, os, sys\nfrom pathlib import Path\n'
                'if sys.argv[1] == "metadata": print(json.dumps({"target_directory": os.environ["ASTER_TEST_TARGET"]}))\n'
                'elif sys.argv[1] == "build": Path(os.environ["ASTER_WRAPPER_LOG"]).write_text(os.environ.get("RUSTC_WRAPPER", "")); raise SystemExit(int(os.environ.get("ASTER_TEST_FAIL", "0")))\n'
                'else: raise SystemExit("unexpected Cargo command")\n'
            )
            cargo.chmod(0o755)
            (tools / "sccache").write_text("#!/bin/sh\nexit 0\n")
            (tools / "sccache").chmod(0o755)
            target = root / "custom target"
            executable = target / "debug/aster"
            executable.parent.mkdir(parents=True)
            executable.write_text(
                '#!/bin/sh\nprintf "%s\\n" "$@" > "$ASTER_OPEN_LOG"\n'
                'printf "%s" "${ASTER_CONFIG-}" > "$ASTER_CONFIG_LOG"\n'
                'printf "%s" "${ASTER_SANDBOX_DIR-}" > "$ASTER_SANDBOX_LOG"\n'
            )
            executable.chmod(0o755)
            log = root / "arguments.txt"
            config_log = root / "config.txt"
            env = dict(os.environ, PATH=f"{tools}:{os.environ['PATH']}",
                       ASTER_TEST_TARGET=str(target), ASTER_OPEN_LOG=str(log),
                       ASTER_CONFIG_LOG=str(config_log), ASTER_WRAPPER_LOG=str(root / "wrapper.txt"),
                       ASTER_TEST_SANDBOX=str(root / "trusted runtime"), ASTER_SANDBOX_LOG=str(root / "sandbox.txt"))
            env.pop("RUSTC_WRAPPER", None)
            env.pop("ASTER_CONFIG", None)
            env.pop("ASTER_SANDBOX_DIR", None)
            result = subprocess.run(["just", "code", "--cwd", "/tmp/aster project"],
                                    cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(log.read_text().splitlines(), ["--cwd", "/tmp/aster project"])
            self.assertEqual(config_log.read_text(), "")
            self.assertEqual((root / "wrapper.txt").read_text(), "sccache")
            if sys.platform == "darwin" and platform.machine() in ("arm64", "aarch64"):
                self.assertEqual((root / "sandbox.txt").read_text(), env["ASTER_TEST_SANDBOX"])
            env["RUSTC_WRAPPER"] = "my-compiler-wrapper"
            acl = root / ".aster" / "config.acl"
            acl.parent.mkdir()
            acl.write_text('default_model = "fixture/local"\n')
            result = subprocess.run(["just", "code"], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(Path(config_log.read_text()), acl.resolve())
            self.assertEqual((root / "wrapper.txt").read_text(), "my-compiler-wrapper")
            env["RUSTC_WRAPPER"] = ""
            env["ASTER_CONFIG"] = "/tmp/kept-by-user.acl"
            env["ASTER_SANDBOX_DIR"] = "/tmp/explicit-sandbox"
            result = subprocess.run(["just", "code"], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(config_log.read_text(), "/tmp/kept-by-user.acl")
            self.assertEqual((root / "wrapper.txt").read_text(), "")
            self.assertEqual((root / "sandbox.txt").read_text(), "/tmp/explicit-sandbox")
            env.pop("ASTER_CONFIG", None)
            log.unlink()
            env["ASTER_TEST_FAIL"] = "1"
            result = subprocess.run(["just", "code"], cwd=root, env=env, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(log.exists(), "failed builds must not launch a stale executable")


@unittest.skipUnless(os.name == "nt", "Windows launcher uses powershell.exe")
class WindowsCodeLauncherTest(unittest.TestCase):
    def compile_probe(self, executable: Path) -> None:
        source = r"""
using System;
using System.IO;
public class AsterProbe {
    public static int Main(string[] args) {
        File.WriteAllLines(Environment.GetEnvironmentVariable("ASTER_OPEN_LOG"), args);
        var config = Environment.GetEnvironmentVariable("ASTER_CONFIG") ?? "";
        File.WriteAllText(Environment.GetEnvironmentVariable("ASTER_CONFIG_LOG"), config, new System.Text.UTF8Encoding(false));
        return 0;
    }
}
"""
        script = (
            "$source = @'\n"
            + source.strip()
            + "\n'@\n"
            + f"Add-Type -TypeDefinition $source -OutputAssembly '{executable}' -OutputType ConsoleApplication\n"
        )
        result = subprocess.run(
            ["powershell.exe", "-NoLogo", "-NoProfile", "-Command", script],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(executable.is_file(), result.stderr)

    def test_code_runs_the_built_binary_and_preserves_arguments(self):
        with tempfile.TemporaryDirectory(prefix="aster-win-") as directory:
            root = Path(directory)
            shutil.copy2(Path(__file__).resolve().parents[1] / "justfile", root / "justfile")
            tools = root / "bin"
            tools.mkdir()
            target = root / "custom target"
            executable = target / "debug" / "aster.exe"
            executable.parent.mkdir(parents=True)
            self.compile_probe(executable)
            target_arg = target.as_posix()
            (tools / "cargo.cmd").write_text(
                "@echo off\r\n"
                "if \"%~1\"==\"metadata\" (\r\n"
                "  echo {\"target_directory\":\"%ASTER_TEST_TARGET%\"}\r\n"
                "  exit /b 0\r\n"
                ")\r\n"
                "if \"%~1\"==\"build\" (\r\n"
                "  >\"%ASTER_PROTOC_LOG%\" echo %PROTOC%\r\n"
                "  exit /b %ASTER_TEST_FAIL%\r\n"
                ")\r\n"
                "echo unexpected Cargo command 1>&2\r\n"
                "exit /b 1\r\n",
                encoding="ascii",
            )
            log = root / "arguments.txt"
            config_log = root / "config.txt"
            protoc_log = root / "protoc.txt"
            project = root / "aster project"
            local_app = root / "localapp"
            known_protoc = local_app / "protoc" / "bin" / "protoc.exe"
            known_protoc.parent.mkdir(parents=True)
            known_protoc.write_bytes(b"")
            path_entries = [
                entry
                for entry in os.environ["PATH"].split(os.pathsep)
                if "protoc" not in entry.lower()
            ]
            env = dict(
                os.environ,
                PATH=f"{tools}{os.pathsep}{os.pathsep.join(path_entries)}",
                LOCALAPPDATA=str(local_app),
                ASTER_TEST_TARGET=target_arg,
                ASTER_OPEN_LOG=str(log),
                ASTER_CONFIG_LOG=str(config_log),
                ASTER_PROTOC_LOG=str(protoc_log),
                ASTER_TEST_FAIL="0",
            )
            env.pop("PROTOC", None)
            env.pop("ASTER_CONFIG", None)
            result = subprocess.run(
                ["just", "code", "--cwd", str(project)],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(log.read_text().splitlines(), ["--cwd", str(project)])
            self.assertEqual(config_log.read_text(), "")
            self.assertEqual(protoc_log.read_text().strip(), str(known_protoc))
            acl = root / ".aster" / "config.acl"
            acl.parent.mkdir()
            acl.write_text('default_model = "fixture/local"\n', encoding="ascii")
            result = subprocess.run(
                ["just", "code"],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(Path(config_log.read_text().strip()), acl.resolve())
            env["ASTER_CONFIG"] = str(root / "kept-by-user.acl")
            result = subprocess.run(
                ["just", "code"],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(config_log.read_text().strip(), env["ASTER_CONFIG"])
            env.pop("ASTER_CONFIG", None)
            log.unlink()
            env["ASTER_TEST_FAIL"] = "1"
            result = subprocess.run(
                ["just", "code"],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(log.exists(), "failed builds must not launch a stale executable")


@unittest.skipUnless(os.name == "posix", "Studio entry fixtures use POSIX scripts")
class StudioLauncherTest(unittest.TestCase):
    def test_removed_work_builds_are_not_distributed(self):
        root = Path(__file__).resolve().parents[1]
        result = subprocess.run(["just", "--summary"], cwd=root, check=True,
                                capture_output=True, text=True, timeout=5)
        recipes = result.stdout.split()
        self.assertNotIn("build-work", recipes)
        self.assertNotIn("build-work-release", recipes)

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
            env["ASTER_TENANT_SLUG"] = "aster"
            result = subprocess.run(["just", "studio", "toyou"], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(opened.read_text().strip(), "toyou 1")
            env["ASTER_TENANT_SLUG"] = "toyou"
            result = subprocess.run(["just", "studio"], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(opened.read_text().strip(), "toyou 1")

    def test_dev_forces_hot_reload_when_the_shell_requests_preview_mode(self):
        for recipe, expected in (("dev", "aster 1"), ("studio", "aster 0")):
            with self.subTest(recipe=recipe), tempfile.TemporaryDirectory(prefix="aster-studio-dev-mode-") as directory:
                root, env, opened = self.fixture(directory)
                env["STUDIO_RENDERER_DEV"] = "0"
                result = subprocess.run(["just", recipe], cwd=root, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(opened.read_text().strip(), expected)

    def test_dev_and_work_forward_the_tenant_to_studio(self):
        for recipe in ("dev", "work"):
            with self.subTest(recipe=recipe), tempfile.TemporaryDirectory(prefix="aster-studio-tenant-") as directory:
                root, env, opened = self.fixture(directory)
                env["ASTER_TENANT_SLUG"] = "aster"
                result = subprocess.run(["just", recipe, "toyou"], cwd=root, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(opened.read_text().strip(), "toyou 1")
                opened.unlink()
                env["ASTER_TENANT_SLUG"] = "kense"
                result = subprocess.run(["just", recipe], cwd=root, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(opened.read_text().strip(), "kense 1")

    def test_product_build_selects_exactly_one_feature_without_default_product(self):
        for tenant, override in (("aster", None), ("toyou", None), ("aster", "custom target")):
            with self.subTest(tenant=tenant), tempfile.TemporaryDirectory(prefix="aster-studio-build-") as directory:
                root, env, _ = self.fixture(directory)
                self.product_fixture(root, tenant)
                if override is not None:
                    env["CARGO_TARGET_DIR"] = str(root / override)
                cargo = root / "bin/cargo"
                cargo.write_text('#!/usr/bin/env python3\nimport json,os,sys\nfrom pathlib import Path\ntarget=Path(os.environ.get("CARGO_TARGET_DIR",str(Path.cwd()/"target")))\nif sys.argv[1]=="metadata": print(json.dumps({"target_directory":str(target)}))\nelse:\n with open(os.environ["ASTER_CARGO_LOG"],"a") as out: out.write(json.dumps(sys.argv[1:])+"\\n")\n for name in ["aster-acp","aster-office"]:\n  path=target/"debug"/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(b"fixture")\n')
                result = subprocess.run(["just", "build-studio", tenant], cwd=root, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                import json
                calls = [json.loads(line) for line in (root / "cargo.txt").read_text().splitlines()]
                shared = ["build", "--locked", "-p", "aster-studio", "-p", "aster-acp", "-p", "aster-office-cli", "--no-default-features", "--features",
                          f"aster-studio/tenant-{tenant},aster-acp/default,aster-acp/headless-search,aster-office-cli/default"]
                self.assertEqual(calls[0], [*shared, "--bin", "aster-acp", "--bin", "aster-office"])
                self.assertEqual(calls[1], [*shared, "--bin", "Studio"])
                expected_target = root / override if override else root / f"target/studio/{tenant}"
                self.assertTrue((expected_target / "debug/aster-acp").is_file())
                self.assertFalse((root / "target/debug/aster-acp").exists())

    def test_staged_build_reuses_shared_features_and_embeds_finished_companions(self):
        real_cargo = shutil.which("cargo")
        self.assertIsNotNone(real_cargo)
        with tempfile.TemporaryDirectory(prefix="aster-build-graph-") as directory:
            root, env, _ = self.fixture(directory)
            self.product_fixture(root, "toyou")
            (root / "bin/cargo").unlink()
            shutil.copy2(Path(__file__).resolve().parents[1] / "rust-toolchain.toml", root / "rust-toolchain.toml")
            env["RUSTC_WRAPPER"] = ""
            env.pop("CARGO_TARGET_DIR", None)
            (root / "Cargo.toml").write_text('[workspace]\nresolver="3"\nmembers=["shared","host","acp","office"]\n')
            packages = [
                ("shared", "shared", '[features]\nagent=[]\ndesktop=[]\n', 'pub fn value()->bool{cfg!(all(feature="agent",feature="desktop"))}'),
                ("acp", "aster-acp", '[features]\ndefault=["svg-png","account-import"]\nsvg-png=[]\naccount-import=[]\nheadless-search=[]\n[dependencies]\nshared={path="../shared",features=["agent"]}\n', 'fn main(){assert!(shared::value());assert!(cfg!(all(feature="svg-png",feature="account-import",feature="headless-search")));}'),
                ("office", "aster-office-cli", '[[bin]]\nname="aster-office"\npath="src/main.rs"\n[features]\ndefault=["browser"]\nbrowser=[]\n[dependencies]\nshared={path="../shared"}\n', 'fn main(){assert!(shared::value());assert!(cfg!(feature="browser"));}'),
                ("host", "aster-studio", '[[bin]]\nname="Studio"\npath="src/main.rs"\n[features]\ndefault=["tenant-aster"]\ntenant-aster=[]\ntenant-toyou=[]\n[dependencies]\nshared={path="../shared",features=["desktop"]}\n', 'fn main(){assert!(shared::value());assert!(cfg!(feature="tenant-toyou"));assert!(!cfg!(feature="tenant-aster"));}')
            ]
            for folder, name, settings, source in packages:
                base = root / folder
                (base / "src").mkdir(parents=True)
                (base / "Cargo.toml").write_text(f'[package]\nname="{name}"\nversion="0.0.0"\nedition="2021"\n'+settings)
                (base / "src" / ("lib.rs" if folder=="shared" else "main.rs")).write_text(source)
            (root / "shared/build.rs").write_text('use std::io::Write;fn main(){let p=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../compiled.txt");writeln!(std::fs::OpenOptions::new().create(true).append(true).open(p).unwrap(),"built").unwrap();}')
            (root / "host/build.rs").write_text('fn main(){for name in ["aster-acp","aster-office"]{assert!(std::process::Command::new(std::path::Path::new(&std::env::var("CARGO_TARGET_DIR").unwrap()).join("debug").join(name)).status().unwrap().success());}}')
            subprocess.run([real_cargo,"generate-lockfile","--offline"],cwd=root,env=env,check=True,capture_output=True)
            import fcntl
            global_target = root / "target/debug"
            global_target.mkdir(parents=True)
            with (global_target / ".cargo-lock").open("w") as locked:
                fcntl.flock(locked, fcntl.LOCK_EX | fcntl.LOCK_NB)
                blocked = subprocess.Popen([real_cargo, "build", "--locked", "-p", "shared"],
                                           cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                try:
                    with self.assertRaises(subprocess.TimeoutExpired):
                        blocked.communicate(timeout=2)
                finally:
                    blocked.terminate()
                    _, blocked_error = blocked.communicate(timeout=5)
                self.assertTrue(any(message in blocked_error for message in (
                    "Blocking waiting for file lock on build directory",
                    "Blocking waiting for file lock on artifact directory",
                )), blocked_error)
                for _ in range(2):
                    run = subprocess.run(["just","build-studio","toyou"],cwd=root,env=env,capture_output=True,text=True,timeout=45)
                    self.assertEqual(run.returncode,0,run.stderr)
                    subprocess.run([str(root / "target/studio/toyou/debug/Studio")],check=True)
            self.assertEqual((root / "compiled.txt").read_text().splitlines(),["built"],"shared dependencies must compile once and remain fresh on restart")

    def test_invalid_brand_refuses_before_compiling_either_product(self):
        for tenant in ("aster", "toyou"):
            with self.subTest(tenant=tenant), tempfile.TemporaryDirectory(prefix="aster-studio-brand-") as directory:
                root, env, _ = self.fixture(directory)
                self.product_fixture(root, tenant)
                info = root / "apps/web/public/tenant" / tenant / "INFO.md"
                info.write_text(info.read_text().replace('product: "Aster"', 'product: "Wrong"').replace('product: "ToWork"', 'product: "Wrong"'))
                result = subprocess.run(["just", "build-studio", tenant], cwd=root, env=env,
                                        capture_output=True, text=True, timeout=10)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("brand", result.stderr)
                self.assertFalse((root / "cargo.txt").exists(), "invalid brand must refuse before a compiler starts")

    def product_fixture(self, root, tenant):
        source = Path(__file__).resolve().parents[1] / "apps/studio/scripts/studio_product.py"
        shutil.copy2(source, root / "apps/studio/scripts/studio_product.py")
        brand = root / "apps/web/public/tenant" / tenant
        brand.mkdir(parents=True)
        for name in ("icon.icns", "icon.png", "icon.ico", "logo-light.png", "mark-light.png"):
            (brand / name).write_bytes(b"fixture icon")
        product = "Aster" if tenant == "aster" else "ToWork"
        (brand / "INFO.md").write_text(f'---\nname:\n  en: "{product}"\nproduct: "{product}"\nlogos:\n  light_surface: logo-light.png\n---\n')

    def test_invalid_tenant_refuses_before_launch(self):
        for recipe in ("studio", "dev"):
            for slug in ("", " ", "Bad", "../aster", "aster;true"):
                with self.subTest(recipe=recipe, slug=slug), tempfile.TemporaryDirectory(prefix="aster-studio-invalid-") as directory:
                    root, env, opened = self.fixture(directory)
                    result = subprocess.run(["just", recipe, slug], cwd=root, env=env, capture_output=True, text=True)
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
        script.write_text('#!/bin/sh\nprintf "%s %s\\n" "$ASTER_TENANT_SLUG" "$STUDIO_RENDERER_DEV" > "$ASTER_OPEN_LOG"\n')
        script.chmod(0o755)
        opened = root / "opened.txt"
        env = dict(os.environ, PATH=f"{tools}:{os.environ['PATH']}", ASTER_OPEN_LOG=str(opened),
                   ASTER_CARGO_LOG=str(root / "cargo.txt"))
        env.pop("ASTER_TENANT_SLUG", None)
        env.pop("STUDIO_RENDERER_DEV", None)
        env.pop("CARGO_TARGET_DIR", None)
        return root, env, opened


@unittest.skipUnless(os.name == "posix", "Build entrypoints use Bash")
class BuildPreflightTest(unittest.TestCase):
    def test_ci_refuses_config_errors_before_cargo(self):
        root = Path(__file__).resolve().parents[1]
        source = (root / ".github/workflows/ci.yml").read_text()
        step = source.split("- name: Build and test desktop product identities\n", 1)[1]
        body = textwrap.dedent(step.split("run: |\n", 1)[1].split("\n      - name:", 1)[0])
        self.refuses_before_cargo(body)

    def test_kylin_build_refuses_config_errors_before_cargo(self):
        root = Path(__file__).resolve().parents[1]
        source = (root / "apps/studio/scripts/kylin-flatpak/build-host.sh").read_text()
        body = source.split("bash -euo pipefail -c '\n", 1)[1].rsplit("\n'", 1)[0]
        self.refuses_before_cargo(body)

    def refuses_before_cargo(self, body):
        with tempfile.TemporaryDirectory(prefix="aster-build-refusal-") as directory:
            marker = Path(directory) / "compiler-called"
            env = dict(os.environ, ASTER_COMPILER_MARKER=str(marker), ASTER_TENANT_SLUG="toyou",
                       STUDIO_RENDERER_DIST="fixture-renderer", CARGO_TARGET_DIR=str(Path(directory) / "target"))
            runner = '''
python3() { if [[ "${3:-}" == tauri-config ]]; then return 42; fi; printf 'tenant-%s\\n' "$ASTER_TENANT_SLUG"; }
cargo() { printf called >"$ASTER_COMPILER_MARKER"; return 73; }
'''
            result = subprocess.run(["bash", "-euo", "pipefail", "-c", runner + body], env=env,
                                    capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 42, result.stderr)
            self.assertFalse(marker.exists(), "a failed config command must never reach Cargo")


if __name__ == "__main__":
    unittest.main()
