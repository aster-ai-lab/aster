#!/usr/bin/env python3
"""Refuse Studio pointer rollbacks, dirty resets, and stash-restore delivery."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STUDIO = ROOT / "apps" / "studio"
STUDIO_REL = "apps/studio"
FORBIDDEN_SUBJECT = re.compile(
    r"(restore\b.*\bstash\b|\bstash\b.*\brestore\b|reassert\b.*\bWIP\b|"
    r"restore pre-merge local optimizations|fully restore ArcUI stash)",
    re.IGNORECASE,
)
CONTRACTS_MANIFEST = STUDIO / "scripts" / "perf-layout-contracts.json"


def run_git(
    args: list[str],
    *,
    cwd: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT if cwd is None else cwd,
        text=True,
        capture_output=True,
        check=check,
    )


def git_output(args: list[str], *, cwd: Path | None = None) -> str:
    return run_git(args, cwd=cwd).stdout.strip()


def studio_gitlink(ref: str = "HEAD") -> str | None:
    line = git_output(["ls-tree", ref, "--", STUDIO_REL])
    if not line:
        return None
    mode, obj_type, rest = line.split(maxsplit=2)
    if mode != "160000" or obj_type != "commit":
        raise SystemExit(f"{STUDIO_REL} at {ref} is not a gitlink: {line}")
    sha, _path = rest.split("\t", 1)
    return sha


def index_studio_gitlink() -> str | None:
    """Return the apps/studio gitlink currently in the index, if any."""
    line = git_output(["ls-files", "-s", "--", STUDIO_REL])
    if not line:
        return None
    # 160000 <sha> <stage>\tapps/studio
    meta, path = line.split("\t", 1)
    if path != STUDIO_REL:
        return None
    mode, sha, _stage = meta.split(maxsplit=2)
    if mode != "160000":
        raise SystemExit(f"{STUDIO_REL} in the index is not a gitlink: {line}")
    return sha


def staged_studio_gitlink() -> str | None:
    """Return the staged tip when it differs from HEAD's gitlink."""
    indexed = index_studio_gitlink()
    if indexed is None:
        return None
    head = studio_gitlink("HEAD")
    if head is None or indexed == head:
        return None
    return indexed


def is_ancestor(candidate: str, descendant: str, *, cwd: Path) -> bool:
    result = run_git(
        ["merge-base", "--is-ancestor", candidate, descendant],
        cwd=cwd,
        check=False,
    )
    return result.returncode == 0


def cmd_pointer(_: argparse.Namespace) -> int:
    head = studio_gitlink("HEAD")
    if head is None:
        print(f"no gitlink at HEAD for {STUDIO_REL}; skip", file=sys.stderr)
        return 0
    staged = staged_studio_gitlink()
    if staged is None:
        print(f"{STUDIO_REL} pointer unchanged in the index")
        return 0
    if staged == head:
        print(f"{STUDIO_REL} staged pointer matches HEAD ({head})")
        return 0
    if not STUDIO.is_dir():
        raise SystemExit(f"{STUDIO_REL} worktree missing; cannot prove ancestry for {staged}")
    # Prefer the submodule object database; fall back to parent if needed.
    studio_git = STUDIO if (STUDIO / ".git").exists() or (STUDIO / ".git").is_file() else ROOT
    for label, sha in (("HEAD", head), ("staged", staged)):
        probe = run_git(["cat-file", "-e", f"{sha}^{{commit}}"], cwd=studio_git, check=False)
        if probe.returncode != 0:
            raise SystemExit(
                f"{STUDIO_REL} {label} commit {sha} is not present locally; "
                "fetch the studio submodule before bumping the pointer"
            )
    if is_ancestor(staged, head, cwd=studio_git):
        print(
            f"refused rollback of {STUDIO_REL}: staged {staged} is an ancestor of HEAD {head}",
            file=sys.stderr,
        )
        return 1
    print(f"{STUDIO_REL} pointer advances {head[:12]} -> {staged[:12]}")
    return 0


def cmd_dirty(_: argparse.Namespace) -> int:
    if not STUDIO.is_dir():
        raise SystemExit(f"{STUDIO_REL} is not checked out")
    porcelain = git_output(["status", "--porcelain", "--untracked-files=normal"], cwd=STUDIO)
    if porcelain:
        print(
            f"refused: {STUDIO_REL} worktree is dirty; commit, push, then bump the parent pointer\n"
            f"{porcelain}",
            file=sys.stderr,
        )
        return 1
    print(f"{STUDIO_REL} worktree is clean")
    return 0


def cmd_messages(args: argparse.Namespace) -> int:
    old = getattr(args, "old", None) or studio_gitlink("HEAD")
    new = (
        getattr(args, "new", None)
        or staged_studio_gitlink()
        or git_output(["rev-parse", "HEAD"], cwd=STUDIO)
    )
    if not old or not new:
        raise SystemExit("need both old and new studio SHAs (pass --old/--new or stage a bump)")
    if old == new:
        print("studio tip unchanged; no delivery commits to scan")
        return 0
    log = git_output(
        ["log", "--format=%H%x09%s", f"{old}..{new}"],
        cwd=STUDIO,
    )
    if not log:
        # Non-fast-forward or history rewrite: still refuse if new is ancestor of old.
        if is_ancestor(new, old, cwd=STUDIO):
            print(
                f"refused rollback range: {new} is an ancestor of {old}",
                file=sys.stderr,
            )
            return 1
        print(f"no commits in {old[:12]}..{new[:12]}")
        return 0
    bad: list[str] = []
    for line in log.splitlines():
        sha, subject = line.split("\t", 1)
        if FORBIDDEN_SUBJECT.search(subject):
            bad.append(f"{sha[:12]} {subject}")
    if bad:
        print(
            "refused stash/WIP restore delivery commits in studio tip range:\n"
            + "\n".join(bad),
            file=sys.stderr,
        )
        return 1
    print(f"studio delivery commits {old[:12]}..{new[:12]} are clean")
    return 0


def load_contract_paths() -> list[str]:
    if not CONTRACTS_MANIFEST.is_file():
        raise SystemExit(f"missing contracts manifest: {CONTRACTS_MANIFEST}")
    data = json.loads(CONTRACTS_MANIFEST.read_text(encoding="utf-8"))
    paths = data.get("tests")
    if not isinstance(paths, list) or not paths or not all(isinstance(p, str) for p in paths):
        raise SystemExit(f"{CONTRACTS_MANIFEST} must contain a non-empty string list under \"tests\"")
    missing = [p for p in paths if not (STUDIO / p).is_file()]
    if missing:
        raise SystemExit(
            "perf/layout contract tests missing:\n" + "\n".join(f"  {p}" for p in missing)
        )
    return paths


def cmd_contracts(_: argparse.Namespace) -> int:
    paths = load_contract_paths()
    env = os.environ.copy()
    cmd = ["pnpm", "exec", "vitest", "run", "--project", "renderer", *paths]
    print("+", " ".join(cmd), flush=True)
    result = subprocess.run(cmd, cwd=STUDIO, env=env)
    return result.returncode


def cmd_check(args: argparse.Namespace) -> int:
    """Pre-bump gate used by agents and just recipes."""
    for name, fn in (
        ("dirty", cmd_dirty),
        ("pointer", cmd_pointer),
        ("messages", cmd_messages),
    ):
        code = fn(args)
        if code != 0:
            print(f"studio land guard failed at: {name}", file=sys.stderr)
            return code
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    sub.add_parser("pointer", help="refuse staged apps/studio gitlink rollbacks")
    sub.add_parser("dirty", help="refuse when the studio worktree has local changes")
    messages = sub.add_parser(
        "messages",
        help="refuse stash/WIP restore subjects in a studio tip range",
    )
    messages.add_argument("--old", help="previous studio SHA (default: parent HEAD gitlink)")
    messages.add_argument("--new", help="new studio SHA (default: staged gitlink or studio HEAD)")
    sub.add_parser("contracts", help="run pinned perf/layout renderer contracts")
    sub.add_parser("check", help="dirty + pointer + messages before a parent bump")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    handlers = {
        "pointer": cmd_pointer,
        "dirty": cmd_dirty,
        "messages": cmd_messages,
        "contracts": cmd_contracts,
        "check": cmd_check,
    }
    return handlers[args.command](args)


if __name__ == "__main__":
    sys.exit(main())
