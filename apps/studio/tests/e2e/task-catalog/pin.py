#!/usr/bin/env python3
"""Freeze or check the SHA-256 of every regular file under ../tasks."""

from __future__ import annotations

import argparse
import json

from common import PIN_PATH, source_files


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="rewrite expected/source-pin.json")
    options = parser.parse_args()
    current = source_files()
    if options.write:
        PIN_PATH.parent.mkdir(parents=True, exist_ok=True)
        PIN_PATH.write_text(json.dumps(current, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {len(current)} files to {PIN_PATH}")
        return
    frozen = json.loads(PIN_PATH.read_text(encoding="utf-8"))
    if frozen != current:
        missing = sorted(set(frozen) - set(current))
        extra = sorted(set(current) - set(frozen))
        changed = sorted(path for path in frozen.keys() & current.keys() if frozen[path] != current[path])
        raise SystemExit(f"source pin drifted missing={missing} extra={extra} changed={changed}")
    print(f"pin ok {len(current)} files")


if __name__ == "__main__":
    main()
