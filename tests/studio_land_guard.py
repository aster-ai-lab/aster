"""Hermetic coverage for Studio land guards (pointer, dirty, restore messages)."""

from __future__ import annotations

import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "studio_land_guard.py"


def load_guard():
    spec = importlib.util.spec_from_file_location("studio_land_guard", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


GUARD = load_guard()


def git(cwd: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=cwd,
        text=True,
        capture_output=True,
        check=check,
        env={
            **os.environ,
            "GIT_AUTHOR_NAME": "studio-land-guard",
            "GIT_AUTHOR_EMAIL": "guard@example.com",
            "GIT_COMMITTER_NAME": "studio-land-guard",
            "GIT_COMMITTER_EMAIL": "guard@example.com",
        },
    )


def init_repo(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    git(path, "init")
    git(path, "config", "user.name", "studio-land-guard")
    git(path, "config", "user.email", "guard@example.com")


def pin_gitlink(root: Path, studio: Path, sha: str, message: str) -> None:
    git(root, "update-index", "--add", "--cacheinfo", f"160000,{sha},apps/studio")
    # Keep a checked-out studio tree for ancestry probes.
    if not (root / "apps" / "studio").exists():
        (root / "apps").mkdir(parents=True, exist_ok=True)
        # Use the real studio object store via a worktree-like checkout directory.
        # Symlink the path so cat-file/merge-base resolve in the studio repo.
        (root / "apps" / "studio").symlink_to(studio, target_is_directory=True)
    git(root, "commit", "-m", message)


class StudioLandGuard(unittest.TestCase):
    def test_pointer_refuses_ancestor_rollback(self) -> None:
        with tempfile.TemporaryDirectory(prefix="studio-land-") as temporary:
            base = Path(temporary)
            studio = base / "studio-src"
            root = base / "parent"
            init_repo(studio)
            (studio / "README").write_text("one\n", encoding="utf-8")
            git(studio, "add", "README")
            git(studio, "commit", "-m", "one")
            older = git(studio, "rev-parse", "HEAD").stdout.strip()
            (studio / "README").write_text("two\n", encoding="utf-8")
            git(studio, "add", "README")
            git(studio, "commit", "-m", "two")
            newer = git(studio, "rev-parse", "HEAD").stdout.strip()

            init_repo(root)
            # Seed an empty first commit so later commits have a parent tree.
            (root / ".keep").write_text("", encoding="utf-8")
            git(root, "add", ".keep")
            git(root, "commit", "-m", "seed")
            pin_gitlink(root, studio, newer, "pin newer")
            git(root, "update-index", "--add", "--cacheinfo", f"160000,{older},apps/studio")

            original_root = GUARD.ROOT
            original_studio = GUARD.STUDIO
            try:
                GUARD.ROOT = root
                GUARD.STUDIO = root / "apps" / "studio"
                self.assertEqual(GUARD.cmd_pointer(None), 1)
            finally:
                GUARD.ROOT = original_root
                GUARD.STUDIO = original_studio

    def test_pointer_allows_forward_bump(self) -> None:
        with tempfile.TemporaryDirectory(prefix="studio-land-") as temporary:
            base = Path(temporary)
            studio = base / "studio-src"
            root = base / "parent"
            init_repo(studio)
            (studio / "README").write_text("one\n", encoding="utf-8")
            git(studio, "add", "README")
            git(studio, "commit", "-m", "one")
            older = git(studio, "rev-parse", "HEAD").stdout.strip()
            (studio / "README").write_text("two\n", encoding="utf-8")
            git(studio, "add", "README")
            git(studio, "commit", "-m", "two")
            newer = git(studio, "rev-parse", "HEAD").stdout.strip()

            init_repo(root)
            (root / ".keep").write_text("", encoding="utf-8")
            git(root, "add", ".keep")
            git(root, "commit", "-m", "seed")
            pin_gitlink(root, studio, older, "pin older")
            git(root, "update-index", "--add", "--cacheinfo", f"160000,{newer},apps/studio")

            original_root = GUARD.ROOT
            original_studio = GUARD.STUDIO
            try:
                GUARD.ROOT = root
                GUARD.STUDIO = root / "apps" / "studio"
                self.assertEqual(GUARD.cmd_pointer(None), 0)
            finally:
                GUARD.ROOT = original_root
                GUARD.STUDIO = original_studio

    def test_dirty_refuses_local_changes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="studio-land-") as temporary:
            studio = Path(temporary) / "studio"
            init_repo(studio)
            (studio / "README").write_text("one\n", encoding="utf-8")
            git(studio, "add", "README")
            git(studio, "commit", "-m", "one")
            (studio / "README").write_text("dirty\n", encoding="utf-8")

            original_root = GUARD.ROOT
            original_studio = GUARD.STUDIO
            try:
                GUARD.ROOT = Path(temporary)
                GUARD.STUDIO = studio
                self.assertEqual(GUARD.cmd_dirty(None), 1)
                git(studio, "checkout", "--", "README")
                self.assertEqual(GUARD.cmd_dirty(None), 0)
            finally:
                GUARD.ROOT = original_root
                GUARD.STUDIO = original_studio

    def test_messages_refuse_stash_restore_subjects(self) -> None:
        with tempfile.TemporaryDirectory(prefix="studio-land-") as temporary:
            studio = Path(temporary) / "studio"
            init_repo(studio)
            (studio / "README").write_text("one\n", encoding="utf-8")
            git(studio, "add", "README")
            git(studio, "commit", "-m", "one")
            older = git(studio, "rev-parse", "HEAD").stdout.strip()
            (studio / "README").write_text("two\n", encoding="utf-8")
            git(studio, "add", "README")
            git(studio, "commit", "-m", "fix(studio): restore pre-merge local optimizations from stash")
            newer = git(studio, "rev-parse", "HEAD").stdout.strip()
            (studio / "README").write_text("three\n", encoding="utf-8")
            git(studio, "add", "README")
            git(studio, "commit", "-m", "feat(ui): land verified selector")
            tip = git(studio, "rev-parse", "HEAD").stdout.strip()

            original_root = GUARD.ROOT
            original_studio = GUARD.STUDIO
            try:
                GUARD.ROOT = Path(temporary)
                GUARD.STUDIO = studio
                self.assertEqual(
                    GUARD.cmd_messages(argparse_namespace(old=older, new=newer)),
                    1,
                )
                self.assertEqual(
                    GUARD.cmd_messages(argparse_namespace(old=older, new=tip)),
                    1,
                )
                self.assertEqual(
                    GUARD.cmd_messages(argparse_namespace(old=newer, new=tip)),
                    0,
                )
            finally:
                GUARD.ROOT = original_root
                GUARD.STUDIO = original_studio

    def test_contracts_manifest_lists_existing_files(self) -> None:
        if not (ROOT / "apps" / "studio" / "package.json").is_file():
            self.skipTest("studio submodule not checked out")
        paths = GUARD.load_contract_paths()
        self.assertGreaterEqual(len(paths), 5)
        for relative in paths:
            self.assertTrue((ROOT / "apps" / "studio" / relative).is_file(), relative)


def argparse_namespace(**kwargs):
    return type("NS", (), kwargs)()


if __name__ == "__main__":
    unittest.main()
