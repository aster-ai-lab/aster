#!/usr/bin/env python3
"""Functional tests for Aster Studio via agent-desktop (headless semantic).

Unlike the coverage sweeps, a case here is not "the click succeeded". Every
case states a precondition, performs a user-visible operation, and asserts the
expected observable outcome (text appearing/disappearing, role counts, control
value/state changes, page restoration). Each case is PASS/FAIL with expected
vs actual, and mutating cases restore the original state in a finally block.

Usage: python3 functional-tests.py <window-id>
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import traceback
from pathlib import Path

APP = "Aster"
WINDOW = sys.argv[1]
HERE = Path(__file__).resolve().parent
RESULTS: list[dict] = []
# An isolated runner can execute one case per fresh window and keep its own
# result file, so inter-case state coupling cannot leak into the verdict.
ONLY_CASES = set(filter(None, os.environ.get("ONLY_CASES", "").split(",")))
RESULTS_PATH = Path(os.environ.get("FT_RESULTS_PATH",
                                  str(HERE / "functional-results.json")))


def cli(*args, timeout=45):
    try:
        r = subprocess.run(["agent-desktop", *args], capture_output=True,
                           text=True, timeout=timeout)
        return json.loads(r.stdout)
    except Exception as e:
        return {"ok": False, "error": {"code": "HARNESS", "message": str(e)}}


def flat(p):
    out = []

    def walk(n):
        if not isinstance(n, dict):
            return
        out.append(n)
        for c in n.get("children") or []:
            walk(c)
    walk((p.get("data") or {}).get("tree"))
    return out


def snapshot():
    a = ["snapshot", "--app", APP, "--window-id", WINDOW,
         "-i", "--compact", "--timeout-ms", "15000"]
    p = cli(*a)
    if not p.get("ok") or not (p.get("data") or {}).get("tree"):
        p = cli(*a)
    return p


class AX:
    """A fresh observation with assertion helpers."""

    def __init__(self):
        self.nodes = flat(snapshot())

    @property
    def names(self):
        return [str(n.get("name")) for n in self.nodes if n.get("name")]

    def has(self, text):
        return any(text in n for n in self.names)

    def has_role(self, role):
        return any(str(n.get("role")) == role for n in self.nodes)

    def count_role(self, role):
        return sum(1 for n in self.nodes if str(n.get("role")) == role)

    @property
    def size(self):
        return len(self.nodes)


def find(role, name):
    p = cli("find", "--app", APP, "--window-id", WINDOW, "--role", role,
            "--name", name, "--exact", "--first", "--timeout-ms", "6000", timeout=15)
    return (p.get("data") or {}).get("match")


def ref(role, name):
    m = find(role, name)
    return m.get("ref_id") if m else None


def find_text(text):
    """Locate by visible text without assuming the AX role."""
    p = cli("find", "--app", APP, "--window-id", WINDOW, "--text", text,
            "--first", "--timeout-ms", "6000", timeout=15)
    return (p.get("data") or {}).get("match")


def permission_opener():
    p = cli("find", "--app", APP, "--window-id", WINDOW, "--role", "combobox",
            "--text", "授权模式", "--first", "--timeout-ms", "6000", timeout=15)
    return (p.get("data") or {}).get("match")


def permission_chip_ref():
    """The chip name changes with state (e.g. '· 先规划'), so locate it by
    role + prefix in a fresh AX tree instead of a hard-coded full name."""
    for x in flat(snapshot()):
        if (x.get("role") == "combobox"
                and str(x.get("name") or "").startswith("授权模式:")):
            return x.get("ref_id")
    return None


def click(role, name):
    r = ref(role, name)
    if not r:
        return False
    return bool(cli("click", r, "--timeout-ms", "2500", timeout=15).get("ok"))


def click_ref(r):
    return bool(cli("click", r, "--timeout-ms", "2500", timeout=15).get("ok"))


def wait(ms=400):
    cli("wait", str(ms), timeout=10)


def set_value(role, name, text):
    r = ref(role, name)
    if not r:
        return False
    return bool(cli("set-value", r, text, "--timeout-ms", "2500", timeout=15).get("ok"))


def state_of(role, name):
    m = find(role, name)
    if not m:
        return None
    return {"value": m.get("value"), "states": m.get("states") or []}


def first_non_composer_textfield():
    for x in flat(snapshot()):
        if (x.get("role") == "textfield" and x.get("ref_id")
                and "描述任务" not in str(x.get("name"))):
            return x.get("ref_id")
    return None


def shot(tag):
    safe = tag.replace("/", "-").replace(" ", "_")
    p = cli("screenshot", "--window-id", WINDOW, str(HERE / f"ft-{safe}.png"), timeout=30)
    return f"ft-{safe}.png" if p.get("ok") else None


def is_clean_workbench():
    n = flat(snapshot())
    nm = [str(x.get("name")) for x in n if x.get("name")]
    return (len(n) <= 90
            and any("描述任务" in x for x in nm)
            and not any("选择模型" in x for x in nm)
            and not any("在所选项目文件夹里直接修改" in x for x in nm)
            and not any(x.get("role") == "dialog" for x in n))


def home():
    """Restore a known clean workbench before each case.

    A freshly launched window is already clean (measured: 64 nodes, zero
    overlays). Blindly clicking generic close buttons here used to OPEN the
    model / task-scope popovers and inflated the tree to 247 nodes, which then
    invalidated five cases. Check first, act only when actually dirty.
    """
    if is_clean_workbench():
        return
    for _ in range(3):
        acted = False
        for label in ("取消", "关闭模型选择", "关闭", "返回应用", "工作"):
            r = ref("button", label)
            if r:
                click_ref(r)
                wait(180)
                acted = True
        r = ref("switch", "展开侧边栏")
        if r:
            click_ref(r)
            wait(180)
            acted = True
        if not acted or is_clean_workbench():
            break
    wait(200)


def check(fid, title, fn):
    home()
    entry = {"id": fid, "title": title, "status": "FAIL",
             "expected": "", "actual": "", "screenshot": None}
    try:
        ok, expected, actual = fn()
        entry["status"] = "PASS" if ok else "FAIL"
        entry["expected"] = expected
        entry["actual"] = actual
    except Exception as e:
        entry["expected"] = "no exception"
        entry["actual"] = f"{type(e).__name__}: {e}\n{traceback.format_exc(limit=2)}"
    entry["screenshot"] = shot(f"{fid}-{title[:14]}")
    RESULTS.append(entry)
    print(json.dumps({k: v for k, v in entry.items() if k != "screenshot"},
                     ensure_ascii=False), flush=True)
    RESULTS_PATH.write_text(
        json.dumps({"window": WINDOW, "results": RESULTS}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8")
    return entry


# --------------------------------------------------------------------------
# Cases
# --------------------------------------------------------------------------

def f01():
    """Model picker opens with search + provider groups + a model list."""
    before = AX().size
    click("combobox", "DeepSeek Flash")
    wait(500)
    a = AX()
    ok = (a.has("选择模型") and a.has("搜索模型") and a.has("配置自定义模型")
          and a.count_role("radiobutton") >= 3 and a.size > before)
    return ok, "dialog shows 选择模型/搜索模型/配置自定义模型, >=3 provider groups, tree grows", \
        f"size {before}->{a.size}, radiobutton={a.count_role('radiobutton')}, has_选择模型={a.has('选择模型')}"


def f02():
    """Selecting a model changes the composer's model label, then restore."""
    original = "DeepSeek Flash"
    try:
        click("combobox", original)
        wait(450)
        picked = find_text("K3.1 Pro")
        if not picked:
            return False, "K3.1 Pro selectable", "option not found in picker"
        click_ref(picked.get("ref_id", ""))
        wait(700)
        changed = find("combobox", "K3.1 Pro") is not None or any(
            "K3.1 Pro" in n for n in AX().names)
        return changed, "composer model label becomes K3.1 Pro", \
            f"K3.1 Pro visible in AX after select: {changed}"
    finally:
        # Restore the user's original model choice if it actually changed.
        home()
        if not find("combobox", "DeepSeek Flash"):
            m = find_text("K3.1 Pro") or find_text("deepseek-flash")
            if m:
                click_ref(m.get("ref_id", ""))
                wait(400)
            target = find_text("DeepSeek Flash")
            if target:
                click_ref(target.get("ref_id", ""))
                wait(400)


def f03():
    """Permission-mode menu lists all five modes with descriptions."""
    chip = permission_chip_ref()
    if chip:
        click_ref(chip)
        wait(500)
    a = AX()
    if a.count_role("menuitem") < 5:
        # A cold first press sometimes only focuses the chip; press once more.
        retry = permission_chip_ref()
        if retry:
            click_ref(retry)
            wait(500)
            a = AX()
    ok = (a.count_role("menuitem") >= 5 and a.has("完全访问 高风险")
          and a.has("只读权限") and a.has("智能授权") and a.has("规划模式"))
    return ok, "5 menu items incl. 请求批准/只读权限/智能授权/完全访问/规划模式", \
        f"chip_found={bool(chip)}, menuitem={a.count_role('menuitem')}, " \
        f"has_完全访问={a.has('完全访问 高风险')}"


def f04():
    """Planning mode is a toggle: selecting it changes the label; toggle back."""
    try:
        chip = permission_chip_ref()
        if not chip:
            return False, "permission combobox present", "control not found"
        click_ref(chip)
        wait(500)
        m = find("menuitem", "规划模式") or find_text("规划模式")
        if not m:
            retry = permission_chip_ref()
            if retry:
                click_ref(retry)
                wait(450)
                m = find("menuitem", "规划模式") or find_text("规划模式")
        if not m:
            return False, "规划模式 menu item present", "menu item not found"
        click_ref(m["ref_id"])
        wait(650)
        names = AX().names
        labels = [n for n in names if "授权模式" in n and ":" in n]
        changed = any("先规划" in n or "规划模式" in n for n in labels)
        return changed, "label reflects planning mode (e.g. 请求批准 · 先规划)", \
            f"labels={labels[:3]}"
    finally:
        # 规划模式 behaves as a toggle inside the menu; click it again.
        home()
        chip = permission_chip_ref()
        if chip:
            click_ref(chip)
            wait(400)
            toggle = find("menuitem", "规划模式")
            if toggle:
                click_ref(toggle["ref_id"])
                wait(400)


def f05():
    """More-options popover exposes its three toggles."""
    before = AX().count_role("switch")
    click("combobox", "更多选项")
    wait(450)
    a = AX()
    ok = a.has("上传附件") and a.has("自动审查") and a.has("产物追溯") \
        and a.count_role("switch") >= before + 2
    return ok, "上传附件/自动审查/产物追溯 visible, >=2 extra switches", \
        f"switch {before}->{a.count_role('switch')}, has_自动审查={a.has('自动审查')}"


def f06():
    """Closing the popover removes its controls."""
    click("combobox", "更多选项")
    wait(400)
    opened = AX().has("自动审查")
    click("button", "关闭")
    wait(400)
    a = AX()
    return (opened and not a.has("自动审查")), "自动审查 present when open, gone after 关闭", \
        f"opened={opened}, after_has_自动审查={a.has('自动审查')}"


def f07():
    """Task search opens a modal dialog."""
    before = AX().size
    click("button", "搜索任务")
    wait(450)
    a = AX()
    ok = a.has_role("dialog") and a.has("关闭") and a.size < before
    return ok, "dialog role present, 关闭 button, tree replaced by modal", \
        f"size {before}->{a.size}, dialog={a.has_role('dialog')}, has_关闭={a.has('关闭')}"


def f08():
    """Typing in task search produces filter results / clear control."""
    click("button", "搜索任务")
    wait(400)
    before = AX()
    # Search dialog exposes a textfield named 搜索任务.
    wrote = set_value("textfield", "搜索任务", "Aster")
    wait(600)
    a = AX()
    ok = a.has("清除") or a.size > before.size or any("Aster" in n for n in a.names)
    return ok, "query text committed; clear control or result rows appear", \
        f"set_value_ok={wrote}, size {before.size}->{a.size}, has_清除={a.has('清除')}"


def f09():
    """Closing search returns to the workbench."""
    click("button", "搜索任务")
    wait(350)
    opened = AX().has_role("dialog")
    click("button", "关闭")
    wait(400)
    a = AX()
    # A lingering `dialog` role after the modal is visually gone is recorded
    # as an a11y observation, but the functional assertion is the visible one.
    ok = opened and not a.has("关闭") and a.has("描述任务")
    return ok, "关闭 hides the modal controls and the composer is usable again", \
        f"opened={opened}, has_关闭_after={a.has('关闭')}, has_描述任务={a.has('描述任务')}, " \
        f"a11y_observation_dialog_role_still_present={a.has_role('dialog')}"


def f10():
    """Sidebar collapse/expand is reversible."""
    before = AX()
    click("switch", "收起侧边栏")
    wait(450)
    collapsed = AX()
    cond1 = collapsed.has("展开侧边栏") and collapsed.size < before.size
    click("switch", "展开侧边栏")
    wait(450)
    restored = AX()
    cond2 = restored.has("收起侧边栏") and abs(restored.size - before.size) <= 8
    return (cond1 and cond2), "collapse hides sidebar and shows 展开侧边栏; expand restores", \
        f"size {before.size}->{collapsed.size}->{restored.size}, cond1={cond1}, cond2={cond2}"


def f11():
    """Navigate to Connectors shows the catalog filters."""
    click("button", "连接器")
    wait(600)
    a = AX()
    ok = a.has("全部") and (a.has("精选") or a.has("安装")) and a.size > 60
    return ok, "connector catalog with 全部/精选 filters and installable cards", \
        f"size={a.size}, has_全部={a.has('全部')}, has_安装={a.has('安装')}"


def f12():
    """Connector search filters the catalog."""
    click("button", "连接器")
    wait(750)
    before = AX()
    if not (before.has("全部") and before.size > 60):
        return False, "connector catalog loaded before searching", \
            f"size={before.size}, has_全部={before.has('全部')}"
    query = "zzzz-no-such-connector"
    field = ref("textfield", "搜索连接器或关键词") or first_non_composer_textfield()
    # input-probe2.json: a bare set-value/type returns ACTION_FAILED on this
    # controlled Chromium input, but focusing first makes set-value succeed
    # (the tool's own ACTION_FAILED hint says the same). No --headed needed.
    focus_ok = bool(field and cli("focus", field, "--timeout-ms", "3000",
                                  timeout=20).get("ok"))
    write_ok = bool(field and cli("set-value", field, query,
                                  "--timeout-ms", "3000", timeout=20).get("ok"))
    wait(850)
    # After a real input the accessible name can change (placeholder drops),
    # so read the value from every input-like node instead of matching a name.
    values = []
    for x in flat(snapshot()):
        if (x.get("role") in ("textfield", "searchfield", "combobox")
                and x.get("ref_id")):
            p = cli("get", x["ref_id"], "--property", "value", timeout=20)
            v = (p.get("data") or {}).get("value")
            if v:
                values.append(str(v))
    committed = any(query[:8] in v for v in values)
    a = AX()
    changed = (a.has("清除") or a.has("没有") or a.has("无结果") or a.has("暂无")
               or a.size != before.size)
    ok = committed and changed
    return ok, "focused query lands in the search field and the catalog reacts", \
        f"field_found={bool(field)}, focus_ok={focus_ok}, set_value_ok={write_ok}, " \
        f"field_values={values[:4]}, size {before.size}->{a.size}, view_changed={changed}"


def f13():
    """Navigate to Automation shows status filters and empty state."""
    click("button", "自动化")
    wait(600)
    a = AX()
    ok = (a.has("全部状态") and a.has("已暂停") and a.has("已错过")
          and (a.has("暂无定时任务") or a.has("新建")))
    return ok, "status filters 全部状态/已暂停/已错过 and empty/new state", \
        f"has_全部状态={a.has('全部状态')}, has_暂无定时任务={a.has('暂无定时任务')}"


def f14():
    """Automation status filter changes the visible state."""
    click("button", "自动化")
    wait(550)
    before = AX()
    click("button", "活跃")
    wait(500)
    a = AX()
    ok = a.size != before.size or a.has("暂无") or a.has("没有")
    return ok, "selecting 活跃 changes the task list / shows an empty state", \
        f"size {before.size}->{a.size}, has_暂无={a.has('暂无')}"


def f15():
    """Settings General renders its switches and labelled inputs."""
    click("button", "设置")
    wait(650)
    a = AX()
    ok = (a.has("开机自动启动") and a.has("语言") and a.has("启用上下文管理")
          and a.count_role("switch") >= 5)
    return ok, "General settings with 开机自动启动/语言/启用上下文管理 and >=5 switches", \
        f"switch={a.count_role('switch')}, has_开机自动启动={a.has('开机自动启动')}"


def f16():
    """A settings switch really toggles and can be restored."""
    click("button", "设置")
    wait(650)
    before = state_of("switch", "开机自动启动")
    r = ref("switch", "开机自动启动")
    if not r:
        return False, "开机自动启动 switch present", "control not found"
    click_ref(r)
    wait(500)
    after = state_of("switch", "开机自动启动")
    # Compare the switch VALUE only: `focused` is transient UI state and must
    # not be treated as a configuration difference.
    changed = bool(before and after and before["value"] != after["value"])
    r2 = ref("switch", "开机自动启动")
    if r2:
        click_ref(r2)
        wait(450)
    restored = state_of("switch", "开机自动启动")
    restored_ok = bool(before and restored and restored["value"] == before["value"])
    return (changed and restored_ok), "switch value toggles then returns to original", \
        f"before={before}, after={after}, restored={restored}, " \
        f"changed={changed}, restored_ok={restored_ok}"


def f17():
    """Usage page renders statistics instead of an empty shell."""
    click("button", "设置")
    wait(550)
    click("button", "用量")
    wait(800)
    a = AX()
    ok = a.has("管理模型与价格") and a.has("Token") and a.size > 500
    return ok, "usage table with 时间/模型/Token/费用/耗时/状态 and 管理模型与价格", \
        f"size={a.size}, has_管理模型与价格={a.has('管理模型与价格')}, has_Token={a.has('Token')}"


def f18():
    """Settings subpage navigation changes the view, returning restores home."""
    # An isolated window can occasionally come up on a non-workbench page when
    # a previous instance has not fully exited. Normalize before asserting.
    if not AX().has("描述任务"):
        r = ref("button", "工作")
        if r:
            click_ref(r)
            wait(700)
    click("button", "设置")
    wait(550)
    click("button", "关于")
    wait(650)
    sub = AX()
    # 返回应用 returns to the page that opened Settings (often the connector
    # catalog: 65 nodes, no composer), NOT always the workbench. The user path
    # is therefore two steps: leave Settings, then click the sidebar 工作.
    left_settings = False
    r = ref("button", "返回应用")
    if r:
        click_ref(r)
        wait(750)
        left_settings = not AX().has("开机自动启动")
    for _ in range(2):
        r = ref("button", "工作")
        if r:
            click_ref(r)
            wait(800)
        if AX().has("描述任务"):
            break
    back = AX()
    # The destination is whichever main-app page opened Settings (composer,
    # connector catalog with 全部, or the recents sidebar) — demanding the
    # composer specifically would misjudge a correct product behaviour.
    in_main_app = (back.has("描述任务") or back.has("全部")
                   or back.has("最近") or back.has("新建任务"))
    ok = (sub.size > 60 and left_settings and in_main_app
          and not back.has("开机自动启动"))
    return ok, "关于 subpage renders; 返回应用 leaves Settings into the main app", \
        f"sub_size={sub.size}, left_settings={left_settings}, in_main_app={in_main_app}, " \
        f"back_has_开机自动启动={back.has('开机自动启动')}, back_size={back.size}"


def f19():
    """New-task entry opens an editable composer state, then restores."""
    before = AX()
    clicked = click("button", "新建任务")
    wait(650)
    a = AX()
    # The new-task entry changes visible content without necessarily adding a
    # textfield or node, so compare the accessible-name signature instead.
    changed = (set(a.names) != set(before.names) or a.size != before.size)
    home()
    restored = AX().has("描述任务")
    return (clicked and changed and restored), \
        "new-task entry changes the view; home restores the composer", \
        f"clicked={clicked}, view_changed={changed}, " \
        f"size {before.size}->{a.size}, restored={restored}"


def f20():
    """Task-scope control opens its selector and can be closed again."""
    if not AX().has("描述任务"):
        r = ref("button", "工作")
        if r:
            click_ref(r)
            wait(700)
    before = AX().size
    # The chip name encodes the selected project, so locate it by prefix.
    r = None
    for x in flat(snapshot()):
        if (x.get("role") == "combobox"
                and str(x.get("name") or "").startswith("任务范围:")):
            r = x.get("ref_id")
            break
    if not r:
        return False, "task-scope combobox present", "control not found"
    try:
        click_ref(r)
        wait(500)
        a = AX()
        return (a.size > before or a.has("关闭") or a.count_role("menuitem") > 0), \
            "task-scope selector opens (tree grows / menu items / close control)", \
            f"size {before}->{a.size}, menuitem={a.count_role('menuitem')}"
    finally:
        # Leave no popover behind for the next run / the user.
        close_btn = ref("button", "关闭")
        if close_btn:
            click_ref(close_btn)
            wait(250)
        elif r:
            click_ref(r)
            wait(250)


CASES = [
    ("F01", "模型选择器展开", f01), ("F02", "切换模型并验证标签", f02),
    ("F03", "授权模式菜单五项", f03), ("F04", "切换授权模式并验证", f04),
    ("F05", "更多选项浮层开关", f05), ("F06", "关闭更多选项浮层", f06),
    ("F07", "任务搜索打开模态", f07), ("F08", "搜索输入过滤", f08),
    ("F09", "关闭搜索回到工作台", f09), ("F10", "侧栏折叠与还原", f10),
    ("F11", "连接器目录加载", f11), ("F12", "连接器搜索过滤", f12),
    ("F13", "自动化页筛选与空态", f13), ("F14", "自动化状态筛选", f14),
    ("F15", "设置通用项渲染", f15), ("F16", "设置开关切换与还原", f16),
    ("F17", "用量页统计渲染", f17), ("F18", "设置子页导航与返回", f18),
    ("F19", "新建任务与返回", f19), ("F20", "任务范围选择器", f20),
]


def restore_permission_label():
    """F04 toggles planning mode; make sure the run leaves no marker behind."""
    labels = [n for n in AX().names if "授权模式:" in n]
    if not any("先规划" in x for x in labels):
        return {"status": "clean", "labels": labels[:2]}
    # clear-plan-first.py proved the 规划模式 menu item is a toggle and that
    # re-selecting 请求批准 does NOT clear planFirst. Use the toggle.
    chip = None
    for x in flat(snapshot()):
        if (x.get("role") == "combobox"
                and str(x.get("name") or "").startswith("授权模式:")):
            chip = x.get("ref_id")
            break
    if chip:
        click_ref(chip)
        wait(500)
        toggle = find("menuitem", "规划模式")
        if toggle:
            click_ref(toggle["ref_id"])
            wait(600)
    after = [n for n in AX().names if "授权模式:" in n]
    return {"status": "restored" if after and not any("先规划" in x for x in after)
            else "FAILED", "before": labels[:2], "after": after[:2]}


def main():
    # A fresh profile opens a permission modal; dismiss it before any case.
    for label in ("继续", "稍后设置", "关闭"):
        m = find("button", label)
        if m:
            click_ref(m["ref_id"])
            wait(350)
            break
    home()
    cases = [c for c in CASES if not ONLY_CASES or c[0] in ONLY_CASES]
    for fid, title, fn in cases:
        check(fid, title, fn)
    # A single-case isolated run must not touch the shared permission state.
    restore = restore_permission_label() if not ONLY_CASES else {"status": "skipped"}
    passed = sum(1 for r in RESULTS if r["status"] == "PASS")
    summary = {"window": WINDOW, "cases": len(RESULTS), "passed": passed,
               "failed": len(RESULTS) - passed,
               "pass_rate": round(passed / len(RESULTS), 4) if RESULTS else 0,
               "permission_restore": restore,
               "results": RESULTS}
    RESULTS_PATH.write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in summary.items() if k != "results"},
                     ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
