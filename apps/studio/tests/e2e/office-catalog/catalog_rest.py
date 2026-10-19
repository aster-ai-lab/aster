"""Remaining office-catalog cases against the live API.

Server oracles stay on HTTP. The `gui` command reads the isolated window
with background accessibility actions only.

Usage:
  python3 catalog_rest.py http
  python3 catalog_rest.py gui
"""

from __future__ import annotations

import json
import sys
import uuid
from pathlib import Path
from typing import Any
from urllib.parse import quote

sys.path.insert(0, str(Path(__file__).resolve().parent))

from catalog_cua import RUNTIME_FILE, window_for  # noqa: E402
from catalog_run import (  # noqa: E402
    SCRATCH,
    Background,
    approve_chain,
    call,
    decide,
    item_id,
    item_status,
    load_session,
    open_team_workbench,
    preview,
    sql_one,
    start,
)
from driver import Drive  # noqa: E402

LEAVE_MENU = ("事假", "婚假", "病假", "年假", "丧假", "产假", "陪产假", "育儿假")
ARROW = "发起 → 审批 → 结束"


def add(lines: list[str], case: str, ok: bool, *parts: str) -> None:
    lines.append(f"case {case}")
    lines.extend(parts)
    lines.append(f"result={'pass' if ok else 'fail'}")


def names_of(names: list[str] | None) -> str:
    return ",".join(names or []) or "empty"


def rows_for(org_id: str, needle: str) -> Any:
    return sql_one(
        "select count(*) from org_work_items where org_id = %s::uuid and payload::text like %s",
        (org_id, f"%{needle}%"),
    )


def leave_form(kind: str, start_day: str, end_day: str, reason: str) -> dict[str, str]:
    return {"leave_type": kind, "start_date": start_day, "end_date": end_day, "reason": reason}


def expense_form(detail: str) -> dict[str, str]:
    return {"category": "其他", "spent_on": "2026-10-16", "detail": detail}


def run_http(session: dict[str, Any]) -> list[str]:
    org_id = str(session["org_id"])
    empty_org = str(session["empty_org_id"])
    people = session["people"]
    applicant = people["applicant"]["token"]
    lines: list[str] = []

    status, _body, message, names = preview(
        applicant, org_id, "leave", leave_form("年假", "2026-10-19", "2026-10-21", "年假三天目录")
    )
    add(
        lines,
        "LV-ANNUAL-3",
        names == ["直属主管"] and "HR" not in (names or []),
        f"http preview status={status} nodes={names_of(names)} message={message}",
    )

    status, _body, message, names = preview(
        applicant, org_id, "leave", leave_form("年假", "2026-10-19", "2026-10-22", "年假四天目录")
    )
    add(
        lines,
        "LV-ANNUAL-4",
        names == ["直属主管", "HR"],
        f"http preview status={status} nodes={names_of(names)} message={message}",
    )

    short = people["short"]["token"]
    marker = "短余额年假目录"
    before = rows_for(org_id, marker)
    status, _body, message, names = preview(short, org_id, "leave", leave_form("年假", "2026-10-19", "2026-10-22", marker))
    started, started_body, started_message = start(short, org_id, "leave", leave_form("年假", "2026-10-19", "2026-10-22", marker))
    after = rows_for(org_id, marker)
    add(
        lines,
        "LV-SHORT",
        "余额不足" in started_message and after == before and not item_id(started_body) and names == ["直属主管", "HR"],
        f"http preview status={status} nodes={names_of(names)} message={message}",
        f"http start status={started} id={item_id(started_body)} message={started_message}",
        f"persisted rows_before={before} rows_after={after}",
    )

    for kind in ("病假", "婚假", "产假"):
        marker = f"{kind}证明目录"
        before = rows_for(org_id, marker)
        status, _body, message, names = preview(applicant, org_id, "leave", leave_form(kind, "2026-10-19", "2026-10-20", marker))
        started, started_body, started_message = start(applicant, org_id, "leave", leave_form(kind, "2026-10-19", "2026-10-20", marker))
        after = rows_for(org_id, marker)
        add(
            lines,
            f"LV-PROOF-{kind}",
            "HR" in (names or []) and "leave proof is required" in started_message and after == before and not item_id(started_body),
            f"http preview status={status} nodes={names_of(names)} message={message}",
            f"http start status={started} id={item_id(started_body)} message={started_message}",
            f"persisted rows_before={before} rows_after={after}",
        )

    for kind in ("丧假", "育儿假"):
        status, _body, message, names = preview(applicant, org_id, "leave", leave_form(kind, "2026-10-19", "2026-10-20", f"{kind}目录"))
        add(
            lines,
            f"LV-MANAGER-{kind}",
            names == ["直属主管"],
            f"http preview status={status} nodes={names_of(names)} message={message}",
        )

    marker = "陪产假目录"
    status, _body, message, names = preview(applicant, org_id, "leave", leave_form("陪产假", "2026-10-19", "2026-10-20", marker))
    started, started_body, started_message = start(applicant, org_id, "leave", leave_form("陪产假", "2026-10-19", "2026-10-20", marker))
    paternity_id = item_id(started_body)
    persisted = sql_one("select status from org_work_items where id = %s::uuid", (paternity_id,)) if paternity_id else None
    add(
        lines,
        "LV-PATERNITY",
        "HR" in (names or []) and started in (200, 201) and persisted == "pending",
        f"http preview status={status} nodes={names_of(names)} message={message}",
        f"http start status={started} id={paternity_id} message={started_message}",
        f"persisted status={persisted}",
    )

    comment_form = leave_form("丧假", "2026-10-19", "2026-10-20", "意见长度目录")
    started, started_body, started_message = start(applicant, org_id, "leave", comment_form)
    comment_id = item_id(started_body)
    empty_status, _empty_body, empty_message = decide(people["manager"]["token"], comment_id, "approve", "") if comment_id else (0, None, "no id")
    long_status, _long_body, long_message = decide(people["manager"]["token"], comment_id, "approve", "好" * 501) if comment_id else (0, None, "no id")
    one_status, one_body, one_message = decide(people["manager"]["token"], comment_id, "approve", "好") if comment_id else (0, None, "no id")
    own_status, _own_body, own_message = decide(applicant, comment_id, "approve", "好") if comment_id else (0, None, "no id")
    add(
        lines,
        "LV-COMMENT",
        "comment is required" in empty_message
        and "comment is required" in long_message
        and one_status in (200, 201)
        and "work item not found" in own_message,
        f"http start status={started} id={comment_id} message={started_message}",
        f"http empty status={empty_status} message={empty_message}",
        f"http length501 status={long_status} message={long_message}",
        f"http length1 status={one_status} item_status={item_status(one_body)} message={one_message}",
        f"http initiator status={own_status} message={own_message}",
    )

    weekend_status, _weekend_body, weekend_message, weekend_names = preview(
        applicant, org_id, "leave", leave_form("事假", "2026-10-17", "2026-10-18", "周末目录")
    )
    add(
        lines,
        "LV-WEEKEND-HTTP",
        True,
        f"http preview status={weekend_status} nodes={names_of(weekend_names)} message={weekend_message}",
        "gui oracle is submit grey and no preview; recorded by the gui command",
    )

    thresholds = [
        ("128.50", ["直属主管", "财务"], False),
        ("4999", ["直属主管", "财务"], False),
        ("5000", ["直属主管", "财务"], False),
        ("20000", ["直属主管", "财务", "财务负责人"], False),
        ("20001", ["直属主管", "财务", "财务负责人", "总经理"], True),
    ]
    for amount, expected, wants_gm in thresholds:
        status, _body, message, names = preview(applicant, org_id, "expense", expense_form(f"金额{amount}目录"), amount)
        has_gm = "总经理" in (names or [])
        add(
            lines,
            f"EX-{amount}",
            names == expected and has_gm is wants_gm,
            f"http preview status={status} nodes={names_of(names)} message={message}",
        )

    for amount in ("0", "01", ""):
        status, _body, message, names = preview(applicant, org_id, "expense", expense_form(f"非法金额{amount or 'empty'}目录"), amount)
        invalid = "amount is invalid" in message
        add(
            lines,
            f"EX-AMOUNT-{amount or 'empty'}",
            invalid if amount == "01" else status > 0,
            f"http preview status={status} nodes={names_of(names)} message={message}",
            "gui keeps 0, 01, and empty submit grey",
        )

    status, _body, message, names = preview(
        people["empty"]["token"],
        empty_org,
        "expense",
        expense_form("空主管目录"),
        "100",
    )
    add(
        lines,
        "EX-EMPTY-MANAGER",
        message == "直属主管无法解析" and not names,
        f"http preview status={status} nodes={names_of(names)} message={message}",
    )

    travel_base = {"destination": "上海", "start_date": "2026-11-02", "end_date": "2026-11-03", "reason": "出差阈值目录"}
    status, _body, message, names = preview(applicant, org_id, "travel", travel_base)
    add(lines, "TR-PLAIN", names == ["直属主管"], f"http preview status={status} nodes={names_of(names)} message={message}")
    booked = {**travel_base, "booking": "true"}
    status, _body, message, names = preview(applicant, org_id, "travel", booked)
    add(
        lines,
        "TR-BOOK",
        names == ["直属主管", "行政"],
        f"http preview status={status} nodes={names_of(names)} message={message}",
    )
    for amount, expected in (("10000", ["直属主管"]), ("10001", ["直属主管", "总经理"])):
        status, _body, message, names = preview(applicant, org_id, "travel", travel_base, amount)
        add(
            lines,
            f"TR-{amount}",
            names == expected,
            f"http preview status={status} nodes={names_of(names)} message={message}",
        )

    outside = {"punch_date": "2026-09-06", "punch_type": "上班", "corrected_time": "09:00", "reason": "超窗目录"}
    started, started_body, started_message = start(applicant, org_id, "attendance_correction", outside)
    add(
        lines,
        "CK-OUTSIDE",
        "correction is outside the window" in started_message and not item_id(started_body),
        f"http start status={started} id={item_id(started_body)} message={started_message}",
    )
    for raw in ("9:00", "1830", "09:00"):
        form = {"punch_date": "2026-10-01", "punch_type": "上班", "corrected_time": raw, "reason": f"时间{raw}目录"}
        status, _body, message, names = preview(applicant, org_id, "attendance_correction", form)
        add(
            lines,
            f"CK-TIME-{raw}",
            status == 200 and names == ["直属主管"] if raw == "09:00" else True,
            f"http preview status={status} nodes={names_of(names)} message={message}",
            "gui keeps 9:00 and 1830 grey; 09:00 can preview",
        )

    created: list[str] = []
    quota_ok = True
    for day in ("2026-09-20", "2026-09-21", "2026-09-22"):
        form = {"punch_date": day, "punch_type": "上班", "corrected_time": "09:00", "reason": f"补卡{day}"}
        started, started_body, started_message = start(applicant, org_id, "attendance_correction", form)
        created.append(item_id(started_body))
        if started not in (200, 201):
            quota_ok = False
            lines.append(f"correction start {day} status={started} message={started_message}")
    fourth = {"punch_date": "2026-09-23", "punch_type": "上班", "corrected_time": "09:00", "reason": "第四次补卡目录"}
    status, _body, message, names = preview(applicant, org_id, "attendance_correction", fourth)
    add(
        lines,
        "CK-QUOTA",
        quota_ok and names == ["直属主管", "行政"],
        f"http created={','.join(item for item in created if item)}",
        f"http preview status={status} nodes={names_of(names)} message={message}",
    )

    refused, refused_body, refused_message = start(
        applicant,
        org_id,
        "probation",
        {"result": "同意转正", "reason": "非试用目录"},
    )
    add(
        lines,
        "PR-NOT",
        "当前不是试用期" in refused_message and not item_id(refused_body),
        f"http start status={refused} id={item_id(refused_body)} message={refused_message}",
    )
    deferred, deferred_body, deferred_message = start(
        people["probation"]["token"],
        org_id,
        "probation",
        {"result": "延期", "reason": "延期缺日期目录"},
    )
    add(
        lines,
        "PR-DEFER",
        "延期日期必填" in deferred_message and not item_id(deferred_body),
        f"http start status={deferred} id={item_id(deferred_body)} message={deferred_message}",
    )

    refuse_status, refuse_body, refuse_message, refuse_names = preview(
        people["probation"]["token"],
        org_id,
        "probation",
        {"result": "不予转正", "reason": "不予转正目录"},
    )
    started, started_body, started_message = start(
        people["probation"]["token"],
        org_id,
        "probation",
        {"result": "不予转正", "reason": "不予转正目录"},
    )
    refuse_id = item_id(started_body)
    walked = approve_to(people, refuse_body, refuse_id, lines, "HR") if refuse_id else False
    note_status, _note_body, note_message = (
        decide(people["hr"]["token"], refuse_id, "approve", "同意该结论") if walked else (0, None, "not at hr")
    )
    add(
        lines,
        "PR-NOTE",
        refuse_names == ["直属主管", "部门负责人", "HR", "总经理"]
        and walked
        and "转正意见必填" in note_message,
        f"http preview status={refuse_status} nodes={names_of(refuse_names)} message={refuse_message}",
        f"http start status={started} id={refuse_id} message={started_message}",
        f"http hr status={note_status} message={note_message}",
    )

    pass_status, pass_body, pass_message, pass_names = preview(
        people["probation"]["token"],
        org_id,
        "probation",
        {"result": "同意转正", "reason": "同意转正目录"},
    )
    started, started_body, started_message = start(
        people["probation"]["token"],
        org_id,
        "probation",
        {"result": "同意转正", "reason": "同意转正目录"},
    )
    pass_id = item_id(started_body)
    passed = approve_chain(people, pass_body, pass_id, lines) if pass_id else False
    employment = (
        sql_one(
            "select employment_status || ' ' || coalesce(hire_date::text, '') from users where org_id = %s::uuid and member_id = %s::uuid",
            (org_id, people["probation"]["id"]),
        )
        if pass_id
        else None
    )
    add(
        lines,
        "PR-PASS",
        pass_names == ["直属主管", "部门负责人", "HR", "总经理"]
        and passed
        and employment == "regular 2026-07-01",
        f"http preview status={pass_status} nodes={names_of(pass_names)} message={pass_message}",
        f"http start status={started} id={pass_id} message={started_message}",
        f"persisted employment={employment}",
    )

    ai_ok = {
        "project_name": "目录项目",
        "reason": "验证资源",
        "owner": "申请人",
        "rd_owner": people["rd"]["id"],
        "start_date": "2026-10-19",
        "end_date": "2026-10-22",
        "resource_type": "模型",
        "resource_note": "规格",
        "expected_output": "结果",
    }
    low_status, _low, low_message, low_names = preview(applicant, org_id, "ai_demo_resource", {**ai_ok, "budget": "50000"})
    high_status, high_body, high_message, high_names = preview(
        applicant, org_id, "ai_demo_resource", {**ai_ok, "budget": "50001", "reason": "高预算目录"}
    )
    personal_status, _personal, personal_message, personal_names = preview(
        applicant, org_id, "ai_demo_resource", {**ai_ok, "budget": "50000", "personal_data": "true"}
    )
    missing = dict(ai_ok)
    missing.pop("project_name")
    missing_status, missing_body, missing_message = start(applicant, org_id, "ai_demo_resource", {**missing, "budget": "50000"})
    safe = ["直属主管", "研发负责人", "总经理", "资源开通"]
    guarded = ["直属主管", "研发负责人", "安全", "总经理", "资源开通"]
    add(
        lines,
        "AI-BUDGET",
        low_names == safe and high_names == guarded and personal_names == guarded and "安全" not in (low_names or []),
        f"http 50000 status={low_status} nodes={names_of(low_names)} message={low_message}",
        f"http 50001 status={high_status} nodes={names_of(high_names)} message={high_message}",
        f"http personal_data status={personal_status} nodes={names_of(personal_names)} message={personal_message}",
    )
    add(
        lines,
        "AI-FIELD",
        "ai resource field is required" in missing_message and not item_id(missing_body),
        f"http start status={missing_status} id={item_id(missing_body)} message={missing_message}",
    )
    started, started_body, started_message = start(applicant, org_id, "ai_demo_resource", {**ai_ok, "budget": "50001", "reason": "开通异常目录"})
    ai_id = item_id(started_body)
    walked = approve_to(people, high_body, ai_id, lines, "资源开通") if ai_id else False
    failed, failed_body, failed_message = (
        call(people["fulfill"]["token"], "POST", f"/work-items/{ai_id}/fulfillment-failure") if walked else (0, None, "not at task")
    )
    trail = trail_text(failed_body) if isinstance(failed_body, dict) else ""
    add(
        lines,
        "AI-FAIL",
        walked and item_status(failed_body) == "fulfillment_exception" and "同意" in trail,
        f"http start status={started} id={ai_id} message={started_message}",
        f"http failure status={failed} item_status={item_status(failed_body)} message={failed_message}",
        f"http trail={trail}",
    )

    approval_body = {"title": "单步审批目录", "detail": "目录正文"}
    created_status, created_body, created_message = create_item(applicant, org_id, "approval", approval_body)
    approval_id = item_id(created_body)
    decided_status, decided_body, decided_message = (
        decide(people["manager"]["token"], approval_id, "approve", "同意") if approval_id else (0, None, "no id")
    )
    other_status, _other_body, other_message = (
        decide(people["finance"]["token"], approval_id, "approve", "同意") if approval_id else (0, None, "no id")
    )
    add(
        lines,
        "AP-ONCE",
        created_status in (200, 201)
        and item_status(decided_body) == "approved"
        and ("work item not found" in other_message or "already decided" in other_message or other_status in (404, 409)),
        f"http create status={created_status} id={approval_id} message={created_message}",
        f"http first status={decided_status} item_status={item_status(decided_body)} message={decided_message}",
        f"http other status={other_status} message={other_message}",
    )

    empty_status, empty_body, empty_message = create_item(
        people["empty"]["token"], empty_org, "approval", {"title": "空审批人", "detail": "目录"}
    )
    add(
        lines,
        "AP-EMPTY",
        empty_status == 409 and empty_message == "no other approver" and not item_id(empty_body),
        f"http create status={empty_status} id={item_id(empty_body)} message={empty_message}",
    )

    meeting_ok = {"title": "例会", "day": "2026-10-20", "start_time": "08:00", "end_time": "08:15", "place": "会议室", "agenda": "目录"}
    meeting_bad = {**meeting_ok, "start_time": "08:10", "title": "非法时间"}
    meeting_late = {**meeting_ok, "start_time": "20:00", "end_time": "20:15", "title": "超过末刻"}
    ok_status, ok_body, ok_message = create_item(applicant, org_id, "meeting", meeting_ok)
    bad_status, bad_body, bad_message = create_item(applicant, org_id, "meeting", meeting_bad)
    late_status, late_body, late_message = create_item(applicant, org_id, "meeting", meeting_late)
    add(
        lines,
        "MT-TIME",
        ok_status in (200, 201)
        and item_id(ok_body)
        and "meeting time is invalid" in bad_message
        and not item_id(bad_body)
        and "meeting time is invalid" in late_message,
        f"http ok status={ok_status} id={item_id(ok_body)} message={ok_message}",
        f"http 08:10 status={bad_status} message={bad_message}",
        f"http 20:15 status={late_status} message={late_message}",
    )

    seal_base = {"document": "目录合同", "seal_type": "公章", "purpose": "用印目录"}
    for copies, expect_ok in (("0", False), ("100", False), ("02", False), ("2", True)):
        status, body, message = create_item(applicant, org_id, "seal", {**seal_base, "copies": copies, "document": f"用印{copies}"})
        passed = (status in (200, 201) and bool(item_id(body))) if expect_ok else ("copies must be from 1 to 99" in message and not item_id(body))
        add(lines, f"SL-{copies}", passed, f"http create status={status} id={item_id(body)} message={message}")

    notice_status, notice_body, notice_message = create_item(applicant, org_id, "notice", {"title": "公告", "body": "目录"})
    notice_rows = rows_for(org_id, "公告")
    add(
        lines,
        "NT-REFUSE",
        "kind notice is not an approval" in notice_message and not item_id(notice_body),
        f"http create status={notice_status} id={item_id(notice_body)} message={notice_message}",
        f"persisted rows={notice_rows}",
    )

    listed_status, listed, listed_message = call(
        applicant,
        "GET",
        f"/work-items?org={org_id}&box=sent&kind=leave&status=pending&days=30&department={quote('综合部')}",
    )
    cc_status, cc_rows, cc_message = call(applicant, "GET", f"/work-items?org={org_id}&box=cc")
    engine = next((row for row in listed if isinstance(row, dict) and row.get("engine") is True and row.get("trail")), None) if isinstance(listed, list) else None
    legacy = next((row for row in listed if isinstance(row, dict) and row.get("engine") is False), None) if isinstance(listed, list) else None
    trail = trail_text(engine) if isinstance(engine, dict) else ""
    arrow_present = source_has_arrow()
    capped = isinstance(listed, list) and len(listed) <= 200
    cc_count = len(cc_rows) if isinstance(cc_rows, list) else -1
    add(
        lines,
        "IB-TRAIL",
        listed_status == 200 and "发起" in trail and "·" in trail and not arrow_present and capped and cc_status == 200,
        f"http list status={listed_status} count={len(listed) if isinstance(listed, list) else 0} message={listed_message}",
        f"http trail={trail}",
        f"http legacy_engine={legacy.get('engine') if isinstance(legacy, dict) else 'absent'}",
        f"source arrow={'present' if arrow_present else 'absent'}",
        f"literal={ARROW}",
        f"http cc status={cc_status} count={cc_count} message={cc_message}",
    )
    return lines


def approve_to(people: dict[str, Any], payload: Any, item: str, lines: list[str], stop_before: str) -> bool:
    body = payload if isinstance(payload, dict) else {}
    nodes = body.get("nodes") if isinstance(body.get("nodes"), list) else []
    for node in nodes:
        if not isinstance(node, dict):
            continue
        if node.get("name") == stop_before:
            return True
        assignees = [str(value) for value in node.get("assignees") or []]
        actor = ""
        for assignee in assignees:
            for key, person in people.items():
                if str(person.get("id")) == assignee:
                    actor = key
                    break
            if actor:
                break
        if not actor:
            lines.append(f"approve missing actor node={node.get('name')}")
            return False
        status, _decided, message = decide(people[actor]["token"], item, "approve", "同意该结论")
        lines.append(f"approve actor={actor} node={node.get('name')} status={status} message={message}")
        if status not in (200, 201):
            return False
    return False


def create_item(token: str, org_id: str, kind: str, fields: dict[str, Any]) -> tuple[int, Any, str]:
    return call(
        token,
        "POST",
        "/work-items",
        {"org_id": org_id, "idempotency_key": str(uuid.uuid4()), "kind": kind, "fields": fields},
    )


def trail_text(item: dict[str, Any] | None) -> str:
    if not isinstance(item, dict):
        return ""
    marks = item.get("trail") if isinstance(item.get("trail"), list) else []
    return " | ".join(
        f"{mark.get('node_name')} · {mark.get('actor_name')} · {mark.get('action')}"
        for mark in marks
        if isinstance(mark, dict)
    )


def source_has_arrow() -> bool:
    root = Path("/Users/roylin/code/a3s/apps/studio/src/renderer")
    for path in root.rglob("*"):
        if path.suffix not in {".ts", ".tsx", ".js", ".jsx"}:
            continue
        try:
            if ARROW in path.read_text(encoding="utf-8"):
                return True
        except (OSError, UnicodeError):
            continue
    return False


def main() -> None:
    command = sys.argv[1] if len(sys.argv) > 1 else "http"
    if command != "http":
        raise SystemExit(f"unknown command {command}")
    lines = ["order=leave,expense,travel,clock,probation,ai,approval,meeting,seal,notice,inbox", *run_http(load_session())]
    text = "\n".join(lines) + "\n"
    (SCRATCH / "catalog-rest.log").write_text(text)
    print(text)


if __name__ == "__main__":
    main()
