"""Structural preflight only; never claims to execute page JavaScript or clicks."""

from html.parser import HTMLParser
from pathlib import Path

ROOT = Path("/app")


class Page(HTMLParser):
    def __init__(self, source):
        super().__init__()
        self.tags, self.ids, self.fragments = [], set(), []
        self.feed(source)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        self.tags.append((tag, attrs))
        if attrs.get("id"):
            self.ids.add(attrs["id"])
        if tag == "a" and attrs.get("href", "").startswith("#"):
            self.fragments.append(attrs["href"][1:])
        assert tag not in {"select", "datalist"}
        for attribute in ("src", "href"):
            if tag in {"script", "link"}:
                assert not attrs.get(attribute, "").startswith(("https:", "http:", "//"))


def page(number):
    source = (ROOT / f"F{number:02}.html").read_text(encoding="utf-8-sig")
    parsed = Page(source)
    assert len(source) > 400
    assert any(tag == "meta" and attrs.get("name", "").lower() == "viewport" for tag, attrs in parsed.tags)
    assert all(fragment and fragment in parsed.ids for fragment in parsed.fragments)
    return source, parsed


def test_f05_timer_controls_exist_but_interaction_remains_unqualified():
    source, parsed = page(5)
    assert all(label in source for label in ("开始", "暂停", "重置"))
    assert sum(tag == "button" for tag, _ in parsed.tags) >= 3
    assert any(tag == "script" for tag, _ in parsed.tags)


def test_f06_product_claims_and_local_trial_form_are_present():
    source, parsed = page(6)
    assert all(label in source for label in ("表单助手", "多表合并", "重复项核对", "汇总表导出", "开始体验"))
    assert any(tag == "form" for tag, _ in parsed.tags)
    assert any(tag == "input" and attrs.get("type") == "email" for tag, attrs in parsed.tags)


def test_f07_all_prototype_destinations_and_settings_control_exist():
    source, parsed = page(7)
    assert all(label in source for label in ("首页", "详情", "设置", "仅显示未完成任务"))
    assert any(tag == "input" and attrs.get("type") == "checkbox" for tag, attrs in parsed.tags)
    assert any(tag == "script" for tag, _ in parsed.tags)


def test_f08_document_structure_table_and_last_acceptance_clause_are_preserved():
    source, parsed = page(8)
    assert all(label in source for label in ("背景", "功能", "字段表", "验收标准", "空邮箱", "非法日期保留原值并提示"))
    assert any(tag == "table" for tag, _ in parsed.tags)
    assert len(parsed.fragments) >= 3
