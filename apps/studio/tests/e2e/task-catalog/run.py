#!/usr/bin/env python3
"""Prepare one ../tasks job. --live is refused until the content-gate binaries are passed explicitly."""

from __future__ import annotations

import argparse
import json

from common import case_by_id, prepare_job, sha256
from grade import deliverable_text, grade


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", required=True)
    parser.add_argument("--live", action="store_true")
    options = parser.parse_args()
    case = case_by_id(options.case)
    if options.live:
        raise SystemExit(
            "内容门沿用 reference_task_boyue_live，需要单独传入 native test 与 Linux ACP。"
            "这一步只准备作业，不调用模型。"
        )
    if case.get("blocked"):
        raise SystemExit(f"{case['id']} 阻塞：{case['blocked']}")
    job = prepare_job(case)
    copied = json.loads((job / "copied.json").read_text(encoding="utf-8"))
    print(f"job={job}")
    print(f"instruction_sha256={sha256(job / 'instruction.txt')}")
    print(f"copied={len(copied)}")
    if case.get("grader"):
        failures = grade(case["id"], deliverable_text(job / "workspace", set(copied)))
        if failures == [f"{case['id']} 的评分器尚未冻结"]:
            raise SystemExit(failures[0])
        print("grader=waiting-for-deliverable")
        print("empty_deliverable_failures=" + str(len(failures)))


if __name__ == "__main__":
    main()
