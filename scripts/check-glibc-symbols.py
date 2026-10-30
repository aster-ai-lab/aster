#!/usr/bin/env python3
"""Fail when a Linux binary needs a glibc newer than the target baseline.

The Linux CLI archive targets glibc 2.31. A binary built against a newer glibc links
against versioned symbols that the older loader cannot resolve, and the failure
only shows up on the target machine as a missing `GLIBC_2.3x`. This check makes
the baseline an assertion in the packaging step instead of a claim in the
release notes.

Usage:
    check-glibc-symbols.py --max 2.31 path/to/aster path/to/a3s-code-acp

Exit status is 0 when every required version is at or below the ceiling, 1 when
any binary exceeds it, and 2 when the versions cannot be read at all. Failing to
read is never treated as passing.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

VERSION_LABEL = re.compile(r"^(?:GLIBC_)?(\d+)\.(\d+)(?:\.(\d+))?$")
# Requirement-only pseudo versions. `GLIBC_PRIVATE` is internal to glibc itself
# and never appears as a symbol requirement of an ordinary program. `GLIBC_ABI_*`
# labels arrive from newer linkers and imply a baseline this check cannot prove.
IGNORED_LABELS = frozenset({"GLIBC_PRIVATE"})
ABI_PREFIX = "GLIBC_ABI_"


def parse_versions(dump: str) -> set[str]:
    """Return every `GLIBC_*` label mentioned in `readelf --version-info` output.

    The version-needs section prints one `Name: GLIBC_x.y  Flags: ...  Version: n`
    line per required version, which is exactly the set the loader must be able
    to resolve. The label is the first token after `Name:`.
    """
    versions = set()
    for line in dump.splitlines():
        _, marker, rest = line.partition("Name: ")
        if not marker:
            continue
        fields = rest.split()
        if fields and fields[0].startswith("GLIBC_"):
            versions.add(fields[0])
    return versions


def numeric(version: str) -> tuple[int, ...] | None:
    match = VERSION_LABEL.match(version)
    if match is None:
        return None
    return tuple(int(part) for part in match.groups(default="0"))


def read_versions(binary: Path, readelf: str) -> set[str]:
    completed = subprocess.run(
        [readelf, "--version-info", "--wide", str(binary)],
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise OSError(
            f"{readelf} failed for {binary}: {completed.stderr.strip() or completed.returncode}"
        )
    return parse_versions(completed.stdout)


def evaluate(versions: set[str], ceiling: str) -> list[str]:
    """Return the violations, newest first. An empty list means the check passes."""
    limit = numeric(ceiling)
    if limit is None:
        raise ValueError(f"ceiling is not a glibc version: {ceiling}")
    violations = []
    for version in sorted(versions):
        if version in IGNORED_LABELS:
            continue
        if version.startswith(ABI_PREFIX):
            violations.append(version)
            continue
        parsed = numeric(version)
        if parsed is None:
            # An unreadable GLIBC_ label is a failure: the baseline cannot be
            # proven, and guessing a meaning here is what ships a broken build.
            violations.append(version)
            continue
        if parsed > limit:
            violations.append(version)
    return violations


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binaries", nargs="*", type=Path)
    parser.add_argument("--max", required=True, dest="ceiling")
    parser.add_argument("--readelf", default="readelf")
    parser.add_argument(
        "--dump",
        type=Path,
        action="append",
        default=[],
        help="check a saved readelf output instead of a binary; repeatable",
    )
    args = parser.parse_args(argv)

    if numeric(args.ceiling) is None:
        print(f"error: --max is not a glibc version: {args.ceiling}", file=sys.stderr)
        return 2
    if not args.binaries and not args.dump:
        print("error: nothing to check", file=sys.stderr)
        return 2

    inspected: list[tuple[str, set[str]]] = []
    try:
        for binary in args.binaries:
            if not binary.is_file():
                print(f"error: missing {binary}", file=sys.stderr)
                return 2
            inspected.append((str(binary), read_versions(binary, args.readelf)))
        for dump in args.dump:
            inspected.append((str(dump), parse_versions(dump.read_text())))
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    failed = False
    for name, versions in inspected:
        violations = evaluate(versions, args.ceiling)
        if violations:
            failed = True
            print(
                f"{name}: needs glibc above {args.ceiling}: {', '.join(violations)}",
                file=sys.stderr,
            )
        else:
            print(f"{name}: glibc <= {args.ceiling}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
