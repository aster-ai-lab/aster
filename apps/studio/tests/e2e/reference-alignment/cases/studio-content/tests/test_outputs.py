"""Minimum source-fact checks; prose quality still needs retained PM review."""

from pathlib import Path

ROOT = Path("/app")


def text(number):
    value = (ROOT / f"F{number:02}.md").read_text(encoding="utf-8-sig")
    assert len(value.strip()) > 120
    return value


def test_f09_quarter_outline_preserves_data_and_unknowns():
    value = text(9)
    assert all(token in value for token in ("12", "10", "20", "18", "2", "60%", "75%", "陈安"))
    assert "营收" in value and any(token in value for token in ("待补", "未提供", "缺失"))


def test_f10_prd_covers_source_scope_and_verifiable_limits():
    value = text(10)
    assert all(token in value for token in ("背景", "目标", "验收", "XLSX", "CSV", "邮箱", "大小写", "原文件", "10 MiB"))
    assert "同名" in value or "相同姓名" in value


def test_f11_contract_findings_quote_each_critical_original_clause():
    value = text(11)
    clauses = [line.partition("：")[2] for line in (ROOT / "inputs/briefs.md").read_text().splitlines() if line.startswith(("第一条：", "第二条：", "第三条：", "第四条："))]
    assert len(clauses) == 4
    assert all(clause in value for clause in clauses)
    assert "验收标准" in value and "交付日期" in value
    assert "建议" in value


def test_f12_minutes_do_not_turn_proposals_into_approved_decisions():
    value = text(12)
    assert all(token in value for token in ("网页", "小程序", "陈安", "2026-10-17", "验收清单", "离线", "日志", "待确认"))
    assert "预算" in value and any(token in value for token in ("未讨论", "没有讨论", "未批准", "没有批准"))


def test_f13_email_has_original_recipient_sender_request_and_deadline():
    value = text(13)
    assert all(token in value for token in ("主题", "周工", "林安", "联调", "2026-10-18", "17:00", "清单", "反馈"))


def test_f14_trip_keeps_fixed_meetings_dates_and_accommodation_limit():
    value = text(14)
    assert all(token in value for token in ("上海", "杭州", "2026-11-02", "2026-11-03", "2026-11-04", "09:30", "11:00", "14:00", "15:00", "450"))
    # Booking claims and time arithmetic require the separate source-bound PM review.
    assert "估" in value and "45" in value and "30" in value


def test_f15_gifts_are_traceable_to_the_given_sample_catalog():
    value = text(15)
    assert all(token in value for token in ("保温杯", "收纳", "笔", "239", "189", "148", "G01", "G02", "G03"))
    assert "估" in value and ("合成" in value or "非实时" in value or "不是实时" in value)


def test_f16_brief_preserves_dates_sources_and_news_status():
    value = text(16)
    assert all(token in value for token in ("2026-10-15", "15:00", "2026-10-16", "10:00", "2026-10-14", "草案", "未生效", "来源"))
    assert "实时" in value and ("未提供" in value or "没有" in value or "未进行" in value)
