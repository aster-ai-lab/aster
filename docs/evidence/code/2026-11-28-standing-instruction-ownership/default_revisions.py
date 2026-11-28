#!/usr/bin/env python3
"""Use both ACP builds' default catalogs with the original six-turn grader."""

from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-six-turn-dsh-comparison'))
import revisions


if __name__ == '__main__':
    revisions.candidate_with_catalog = lambda _directory, binary: binary
    revisions.main()
