"""Exercise the real Rust source stamp against isolated Git fixtures."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class PairIdentity(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        (ROOT / "target").mkdir(exist_ok=True)
        cls.temporary = tempfile.TemporaryDirectory(prefix="pair-identity-", dir=ROOT / "target")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.directory = Path(cls.temporary.name)
        cls.probe = cls.directory / ("probe.exe" if os.name == "nt" else "probe")
        source = cls.directory / "probe.rs"
        source.write_text(
            '#[path = ' + json.dumps(str(ROOT / "scripts/cli_pair_build.rs"), ensure_ascii=False) + ']\n'
            'mod stamp;\nfn main() { stamp::stamp(std::path::Path::new(&std::env::args().nth(1).unwrap())).unwrap(); }\n'
        )
        subprocess.run(["rustc", "--edition=2021", str(source), "-o", str(cls.probe)], cwd=ROOT, check=True, timeout=120)
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"], cwd=ROOT, timeout=60
        ))
        packages = {str(Path(p["manifest_path"]).parent.resolve()): p for p in metadata["packages"]}
        pending = [p for p in metadata["packages"] if p["name"] in {"aster", "a3s-code-acp"}]
        sources = set()
        seen = set()
        while pending:
            package = pending.pop()
            if package["manifest_path"] in seen:
                continue
            seen.add(package["manifest_path"])
            target = next(t for t in package["targets"] if set(t["kind"]) & {"lib", "rlib", "proc-macro", "bin"})
            sources.add(Path(target["src_path"]).relative_to(ROOT))
            for dependency in package["dependencies"]:
                if dependency.get("path"):
                    pending.append(packages[str(Path(dependency["path"]).resolve())])
        cls.sources = sorted(sources)

    def setUp(self):
        self.repo = Path(tempfile.mkdtemp(prefix="repo-", dir=self.directory))
        templates = self.directory / "empty-templates"
        templates.mkdir(exist_ok=True)
        self.env = dict(os.environ, TARGET="fixture-target", GIT_CONFIG_NOSYSTEM="1",
                        GIT_CONFIG_GLOBAL=os.devnull, GIT_TEMPLATE_DIR=str(templates))
        subprocess.run(["git", "init", "--quiet", str(self.repo)], env=self.env, check=True, timeout=30)
        for path in self.sources:
            self.write(path, "// initial source\n")
        subprocess.run(["git", "add", "."], cwd=self.repo, env=self.env, check=True, timeout=30)

    def write(self, path, content):
        destination = self.repo / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(content)

    def identity(self, target="fixture-target"):
        result = subprocess.run([str(self.probe), str(self.repo)], env=dict(self.env, TARGET=target),
                                capture_output=True, text=True, check=True, timeout=120)
        return next(line for line in result.stdout.splitlines() if line.startswith("cargo:rustc-env=ASTER_PAIR_BUILD_ID="))

    def test_every_local_dependency_source_change_invalidates_the_pair(self):
        original = self.identity()
        for path in self.sources:
            with self.subTest(path=str(path)):
                self.write(path, "// changed worktree source\n")
                try:
                    self.assertNotEqual(original, self.identity(), str(path))
                finally:
                    self.write(path, "// initial source\n")

    def test_target_and_build_config_change_identity_but_docs_do_not(self):
        original = self.identity()
        self.write(Path("docs/guide.md"), "unrelated documentation\n")
        self.assertEqual(original, self.identity())
        self.assertNotEqual(original, self.identity("another-target"))
        self.write(Path(".cargo/config.toml"), '[build]\nrustflags = ["--cfg", "fixture_flag"]\n')
        self.assertNotEqual(original, self.identity())

    def test_swiftpm_build_cache_does_not_change_identity(self):
        self.write(Path(".gitignore"), (ROOT / ".gitignore").read_text())
        source = Path("crates/capture/Sources/CaptureKit/Probe.swift")
        self.write(source, "let source = 1\n")
        original = self.identity()
        cache = Path("crates/capture/.build/debug/build.db")
        self.write(cache, "generated output\n")
        self.assertEqual(original, self.identity())
        self.write(cache, "updated generated output\n")
        self.assertEqual(original, self.identity())
        self.write(source, "let source = 2\n")
        self.assertNotEqual(original, self.identity())


if __name__ == "__main__":
    unittest.main()
