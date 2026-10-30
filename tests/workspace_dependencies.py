"""Prevent internal libraries from silently returning to remote releases."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]


class WorkspaceDependenciesTest(unittest.TestCase):
    def test_products_share_one_flow_engine(self):
        metadata = json.loads(subprocess.check_output([
            "cargo", "metadata", "--locked", "--offline", "--no-deps",
            "--format-version", "1",
        ], cwd=ROOT))
        engines = [package for package in metadata["packages"]
                   if any(target["name"] == "a3s_flow" for target in package["targets"])]
        self.assertEqual(len(engines), 1, [package["manifest_path"] for package in engines])
        engine = engines[0]
        self.assertEqual(engine["name"], "a3s-flow")
        self.assertEqual(Path(engine["manifest_path"]), ROOT / "crates/flow/Cargo.toml")
        for consumer in ["a3s-code-core", "a3s-studio"]:
            package = next(package for package in metadata["packages"] if package["name"] == consumer)
            dependencies = [dependency for dependency in package["dependencies"]
                            if dependency["name"] == engine["name"]]
            self.assertEqual(len(dependencies), 1, consumer)
            self.assertEqual(Path(dependencies[0]["path"]), ROOT / "crates/flow", consumer)

    def test_desktop_sources_and_executable_fixtures_are_present(self):
        desktop = ROOT / "apps/studio"
        host = desktop / "src-tauri/Cargo.toml"
        metadata = json.loads(subprocess.check_output([
            "cargo", "metadata", "--locked", "--offline", "--no-deps",
            "--format-version", "1", "--manifest-path", str(host),
        ], cwd=ROOT))
        self.assertEqual(Path(metadata["workspace_root"]).resolve(), ROOT)
        hosts = [package for package in metadata["packages"]
                 if Path(package["manifest_path"]).resolve() == host.resolve()]
        self.assertEqual(len(hosts), 1)
        self.assertEqual(hosts[0]["name"], "a3s-studio")
        self.assertNotIn("aster-dream", {package["name"] for package in metadata["packages"]})
        self.assertIn(hosts[0]["id"], metadata["workspace_members"])
        for manifest in [desktop / "Cargo.toml", host]:
            if manifest.is_file():
                self.assertNotIn("workspace", tomllib.loads(manifest.read_text()), str(manifest))
        for lock in [desktop / "Cargo.lock", desktop / "src-tauri/Cargo.lock"]:
            self.assertFalse(lock.exists(), str(lock))
        for path in [
            "packages/office/src/core.ts",
            "apps/studio/src-tauri/src/lib.rs",
            "packages/office/generated/office-kernel.wasm",
            "packages/ce/dist/js/basecoat.js",
        ]:
            self.assertTrue((ROOT / path).is_file(), path)
        for path in [
            "plugin-v3-package/package/tools/echo",
            "plugin-v3/package/tools/convert/bin/convert",
        ]:
            self.assertTrue(os.access(ROOT / "crates/use/extension/fixtures/packages" / path, os.X_OK))

    def test_office_kernel_rebuilds_from_the_workspace(self):
        package = (ROOT / "packages/office/package.json").read_text()
        script = (ROOT / "packages/office/scripts/build-office-kernel.sh").read_text()
        package_host = (ROOT / "apps/studio/scripts/package-host.sh").read_text()
        kylin = (ROOT / "apps/studio/scripts/kylin-flatpak/build-host.sh").read_text()
        shell = (ROOT / "apps/studio/scripts/studio-shell.sh").read_text()
        self.assertIn('"build": "bash scripts/build-office-kernel.sh && rslib"', package)
        self.assertIn("--locked", script)
        self.assertIn("--package a3s-office-web-kernel", script)
        self.assertIn("OFFICE_KERNEL_REQUIRED", script)
        self.assertIn("generated/office-kernel.wasm", script)
        self.assertNotIn("apps/work", script)
        self.assertLess(package_host.index("OFFICE_KERNEL_REQUIRED=1"), package_host.index("pnpm build:office"))
        self.assertIn("packages/office/scripts/build-office-kernel.sh", kylin)
        self.assertIn("pnpm build:office", package_host)

    def test_studio_uses_one_workspace_without_external_sources(self):
        studio = ROOT / "apps/studio"
        modules = subprocess.check_output([
            "git", "config", "--file", str(ROOT / ".gitmodules"),
            "--get", "submodule.studio.url",
        ], cwd=ROOT, text=True).strip()
        self.assertEqual(modules, "git@github.com:kense-labs/studio.git")
        tracked = subprocess.check_output([
            "git", "ls-files", "--stage", "--", "apps/studio",
        ], cwd=ROOT, text=True).splitlines()
        self.assertEqual(len(tracked), 1, "desktop sources belong only to the private submodule")
        self.assertTrue(tracked[0].startswith("160000 "), tracked)
        self.assertFalse(list(studio.rglob("Cargo.lock")))
        for base in [studio, ROOT / "packages", ROOT / "apps/web", ROOT / "apps/use-registry"]:
            for directory, dirs, files in os.walk(base):
                dirs[:] = [name for name in dirs if name not in {"node_modules", "dist", "target", ".git"}]
                for name in dirs + files:
                    path = Path(directory) / name
                    if path.is_symlink():
                        self.assertTrue(path.resolve().is_relative_to(ROOT), str(path))
                        self.assertTrue(path.exists(), str(path))
        for path in ["src-tauri/Cargo.toml", "native/data-sqlite/Cargo.toml", "src-tauri/contract/Cargo.toml"]:
            manifest = tomllib.loads((studio / path).read_text())
            self.assertNotIn("workspace", manifest, path)

    def test_studio_stages_companions_from_the_active_cargo_profile(self):
        with tempfile.TemporaryDirectory(prefix="studio-build-") as temporary:
            root = Path(temporary)
            stub = root / "tauri_build.rs"
            stub.write_text("pub fn build() {}")
            library = root / "libtauri_build.rlib"
            subprocess.run(["rustc", "--crate-type=rlib", str(stub), "-o", str(library)], check=True)
            executable = root / "build-script"
            subprocess.run([
                "rustc", "--edition=2024", str(ROOT / "apps/studio/src-tauri/build.rs"),
                "--extern", f"tauri_build={library}", "-o", str(executable),
            ], check=True)
            output = root / "custom target/debug/build/studio/out"
            output.mkdir(parents=True)
            manifest = root / "app"
            env = {**os.environ, "OUT_DIR": str(output), "CARGO_MANIFEST_DIR": str(manifest),
                   "CARGO_CFG_TARGET_OS": "windows", "PROFILE": "release"}
            refused = subprocess.run([str(executable)], env=env, capture_output=True, text=True)
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("build ACP and Office first", refused.stderr)
            env["PROFILE"] = "debug"
            subprocess.run([str(executable)], env=env, check=True, capture_output=True)
            for name in ["a3s-code-acp.exe", "a3s-office.exe"]:
                (output.parents[2] / name).write_bytes(name.encode())
            subprocess.run([str(executable)], env=env, check=True, capture_output=True)
            for name in ["a3s-code-acp.exe", "a3s-office.exe"]:
                self.assertEqual((manifest / "resources/agent" / name).read_bytes(), name.encode())
            resource = manifest / "resources/agent"
            stamps = {path: path.stat().st_mtime_ns for path in [resource, *resource.iterdir()]}
            subprocess.run([str(executable)], env=env, check=True, capture_output=True)
            self.assertEqual({path: path.stat().st_mtime_ns for path in stamps}, stamps)
            source = output.parents[2] / "a3s-office.exe"
            source.write_bytes(b"x" * source.stat().st_size)
            subprocess.run([str(executable)], env=env, check=True, capture_output=True)
            self.assertEqual((resource / source.name).read_bytes(), source.read_bytes())
            env["STUDIO_COMPANIONS_STAGED"] = "1"
            refused = subprocess.run([str(executable)], env=env, capture_output=True, text=True)
            self.assertNotEqual(refused.returncode, 0)
            private = output.parents[2] / "studio-resources/agent"
            private.mkdir(parents=True, exist_ok=True)
            for name in ["a3s-code-acp.exe", "a3s-office.exe"]:
                (private / name).write_bytes((output.parents[2] / name).read_bytes())
            neighbor = resource / "a3s-code-acp.exe"
            neighbor.write_bytes(b"another target's companion")
            subprocess.run([str(executable)], env=env, check=True, capture_output=True)
            self.assertEqual(neighbor.read_bytes(), b"another target's companion")
            (private / "a3s-office.exe").write_bytes(b"stale private companion")
            refused = subprocess.run([str(executable)], env=env, capture_output=True, text=True)
            self.assertNotEqual(refused.returncode, 0)
            env.pop("STUDIO_COMPANIONS_STAGED")
            subprocess.run([str(executable)], env=env, check=True, capture_output=True)
            if sys.platform == "darwin":
                locked = manifest / "resources/agent" / "a3s-code-acp.exe"
                locked.write_bytes(b"stale")
                subprocess.run(["chflags", "uchg", str(locked)], check=True)
                try:
                    (output.parents[2] / "a3s-code-acp.exe").write_bytes(b"replaced")
                    subprocess.run([str(executable)], env=env, check=True, capture_output=True)
                    self.assertEqual(locked.read_bytes(), b"replaced")
                finally:
                    subprocess.run(["chflags", "nouchg", str(locked)], check=False)
                binary = output.parents[2] / "Studio.exe"
                binary.write_bytes(b"studio")
                subprocess.run(["chflags", "uchg", str(binary)], check=True)
                try:
                    subprocess.run([str(executable)], env=env, check=True, capture_output=True)
                    flags = subprocess.check_output(["stat", "-f", "%Sf", str(binary)], text=True).strip()
                    self.assertNotIn("uchg", flags)
                finally:
                    subprocess.run(["chflags", "nouchg", str(binary)], check=False)

    def test_internal_dependencies_are_local_and_unpublishable(self):
        output = subprocess.check_output([
            "cargo", "metadata", "--locked", "--offline", "--no-deps",
            "--format-version", "1",
        ], cwd=ROOT)
        metadata = json.loads(output)
        names = {package["name"] for package in metadata["packages"]}
        self.assertTrue({"aster", "a3s-code-acp", "a3s-office-cli", "a3s-studio", "a3s-studio-data-sqlite", "a3s-studio-desktop-contract"} <= names)
        for package in metadata["packages"]:
            self.assertEqual(package["publish"], [], package["name"])
            for dependency in package["dependencies"]:
                if dependency["name"].startswith("a3s-"):
                    self.assertIsNone(dependency["source"], dependency["name"])
                    self.assertTrue(Path(dependency["path"]).resolve().is_relative_to(ROOT))
        lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
        for package in lock["package"]:
            if package["name"].startswith("a3s-"):
                self.assertNotIn("source", package, package["name"])


if __name__ == "__main__":
    unittest.main()
