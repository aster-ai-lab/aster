"""Synthetic outcome checks for features.xlsx rows 2-5, not original benchmark graders."""

from collections import Counter
import csv
from decimal import Decimal
import json
from pathlib import Path


ROOT = Path("/app")


def rows(name, headers):
    with (ROOT / name).open(encoding="utf-8-sig", newline="") as stream:
        table = csv.DictReader(stream)
        assert table.fieldnames == headers
        return list(table)


def test_f01_merge_keeps_distinct_people_and_missing_keys():
    data = rows("F01.csv", ["name", "email", "department", "date", "sources"])
    assert len(data) == 5
    assert Counter(row["name"] for row in data) == {"张青": 1, "李明": 2, "王宁": 2}
    named = {row["email"].lower(): row for row in data if row["email"]}
    assert set(named) == {"zhang@example.test", "li.one@example.test", "li.two@example.test"}
    assert named["zhang@example.test"]["date"] == "2026-10-01"
    assert "registration-a.xlsx" in named["zhang@example.test"]["sources"]
    assert "registration-b.xlsx" in named["zhang@example.test"]["sources"]
    assert named["li.one@example.test"]["date"] == "2026-10-02"
    assert named["li.two@example.test"]["date"] == "2026-10-04"
    assert all(row["date"] == "2026-10-03" and row["department"] == "研发" for row in data if row["name"] == "王宁")


def test_f02_project_summary_retains_sources_facts_and_conflict():
    data = json.loads((ROOT / "F02.json").read_text())
    projects = {project["name"]: project for project in data["projects"]}
    assert set(projects) == {"Orbit", "Nova"}
    originals = (ROOT / "inputs/weekly.md").read_text()
    for project in projects.values():
        for kind in ("updates", "risks", "nextActions", "conflicts"):
            assert isinstance(project[kind], list)
            for item in project[kind]:
                assert item["text"].strip()
                assert item["source"] in {"陈安", "赵宁", "吴敏"}
                assert item["quote"] and item["quote"] in originals
    assert any("8/10" in item["quote"] for item in projects["Orbit"]["updates"])
    assert any("60%" in item["quote"] for item in projects["Nova"]["updates"])
    assert projects["Orbit"]["conflicts"]
    assert any("缺少测试账号" in item["quote"] for item in projects["Nova"]["risks"])
    assert {item["source"] for item in projects["Orbit"]["nextActions"]} == {"陈安", "赵宁"}
    assert projects["Nova"]["nextActions"]


def test_f03_invoice_extraction_preserves_zero_missing_and_duplicate_pages():
    data = rows("F03.csv", ["invoice_id", "date", "amount", "tax_id", "source_pages"])
    assert len(data) == 2
    by_id = {row["invoice_id"]: row for row in data}
    assert set(by_id) == {"DEMO-A", "DEMO-B"}
    assert by_id["DEMO-A"]["date"] == "2026-10-11"
    assert Decimal(by_id["DEMO-A"]["amount"]) == Decimal("123.45")
    assert by_id["DEMO-A"]["tax_id"] == "DEMO-TAX-A"
    assert "1" in by_id["DEMO-A"]["source_pages"] and "3" in by_id["DEMO-A"]["source_pages"]
    assert by_id["DEMO-B"]["date"] == "2026-10-12"
    assert Decimal(by_id["DEMO-B"]["amount"]) == 0
    assert by_id["DEMO-B"]["tax_id"] == ""
    assert "2" in by_id["DEMO-B"]["source_pages"]


def test_f04_cleaning_does_not_invent_dates_or_lose_zeroes():
    data = rows("F04.csv", ["id", "date", "sku", "value", "status"])
    assert len(data) == 5
    observed = {row["id"]: row for row in data}
    expected = {
        "A1": ("2026-10-01", "ABC", "0", "ok"),
        "A2": ("2026-02-30", "ABC", "", "invalid_date"),
        "A3": ("", "XYZ", "10", "missing_date"),
        "A4": ("2026-02-29", "EFG", "8", "invalid_date"),
        "A5": ("2024-02-29", "HIJ", "2", "ok"),
    }
    assert set(observed) == set(expected)
    for key, wanted in expected.items():
        row = observed[key]
        date, sku, value, status = wanted
        assert (row["date"], row["sku"], row["status"]) == (date, sku, status)
        assert Decimal(row["value"]) == Decimal(value) if value else row["value"] == ""
