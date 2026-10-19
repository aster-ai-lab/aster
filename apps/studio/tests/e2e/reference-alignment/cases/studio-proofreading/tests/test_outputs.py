"""Synthetic source-bound behavior checks for the four reference workflows."""

import json
from pathlib import Path

ROOT = Path("/app")


def output(name):
    return json.loads((ROOT / f"{name}.json").read_text(encoding="utf-8-sig"))


def findings(items, filename, kinds):
    source = (ROOT / "inputs" / filename).read_text()
    seen = set()
    for item in items:
        quote = item["errortext"]
        assert quote and quote in source
        assert item["checkresult"].strip() and item["checkresult"] != quote
        assert item["reason"].strip()
        assert item["type"] in kinds
        key = (quote, item["type"], item["checkresult"])
        assert key not in seen
        seen.add(key)


def test_w01_ambiguity_selected_checks_whitelist_and_mixed_dispatch():
    data = output("W01")
    ambiguous = data["ambiguous"]
    assert ambiguous["selectedFiles"] == [] and ambiguous["findings"] == []
    assert ambiguous["clarification"].strip()
    selected = data["selected"]
    assert selected["selectedFiles"] == ["inputs/a.txt"]
    assert selected["findings"]
    findings(selected["findings"], "a.txt", {"basic"})
    assert all("按排" not in item["errortext"] for item in selected["findings"])
    assert any("以经" in item["errortext"] and "已经" in item["checkresult"] for item in selected["findings"])
    mixed = data["mixed"]
    assert mixed["selectedFiles"] == ["inputs/notice.txt"]
    assert mixed["documentType"]
    findings(mixed["findings"], "notice.txt", {"basic", "official", "security"})
    assert {item["type"] for item in mixed["findings"]} == {"basic", "official", "security"}


def test_w02_only_selected_official_checks_have_original_quotes_and_corrections():
    data = output("W02")
    assert data["selectedChecks"] == ["公文格式", "行文规范"]
    assert len(data["findings"]) >= 2
    findings(data["findings"], "notice.txt", {"official"})
    assert all(item["checkItem"] in data["selectedChecks"] for item in data["findings"])
    assert any("2026年13月40日" in item["errortext"] for item in data["findings"])
    assert any("报告" in item["errortext"] for item in data["findings"])


def test_w03_deep_security_problems_without_forcing_irrelevant_text():
    data = output("W03")
    assert data["relevant"]["applicable"] is True
    items = data["relevant"]["findings"]
    assert len(items) >= 2
    findings(items, "security.txt", {"security"})
    assert any("密码" in item["errortext"] or "账号" in item["errortext"] for item in items)
    assert any("身份证" in item["errortext"] for item in items)
    assert data["irrelevant"]["applicable"] is False
    assert data["irrelevant"]["findings"] == []


def test_w04_obvious_corrections_and_zero_false_positives_on_correct_text():
    data = output("W04")
    items = data["incorrect"]["findings"]
    assert items
    findings(items, "a.txt", {"basic"})
    assert any("以经" in item["errortext"] and "已经" in item["checkresult"] for item in items)
    assert any("按排" in item["errortext"] and "安排" in item["checkresult"] for item in items)
    assert data["correct"]["findings"] == []
