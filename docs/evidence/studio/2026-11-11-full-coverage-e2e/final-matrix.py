#!/usr/bin/env python3
"""Build the authoritative coverage matrix from ax-coverage.json.

Separates two different claims so neither is overstated:
  - AX touch: every functional point received a real accessibility interaction.
  - Action execution: a mutating action (click/toggle/...) was also dispatched.
"""
from __future__ import annotations

import json
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ax = json.loads((HERE / "ax-coverage.json").read_text(encoding="utf-8"))
rows = ax["coverage"]
pages = json.loads((HERE / "pages.json").read_text(encoding="utf-8"))["pages"]

# Merge any mutating-action evidence from the per-control / chunk passes.
executed: dict[tuple[str, str, str], dict] = {}
# Later files override earlier ones; action-coverage.json is the authoritative
# full mutating-action pass, so it is merged last.
for name in ("coverage.json", "coverage-batch.json", "action-coverage.json"):
    p = HERE / name
    if not p.exists():
        continue
    try:
        d = json.loads(p.read_text(encoding="utf-8"))
    except Exception:
        continue
    for r in d.get("coverage", []):
        key = (r.get("page", ""), r.get("role", ""), r.get("name", ""))
        action = r.get("action") or ""
        # A page whose control count did not match the inventory was polluted
        # by a residual overlay; its action records are not trustworthy.
        if name == "action-coverage.json" and not r.get("navigation_verified"):
            continue
        # `get-states` is read-only AX inspection, not a mutating action.
        if r.get("ok") and action and action != "get-states":
            executed[key] = {"action": action, "source": name}

by_page = defaultdict(list)
for r in rows:
    by_page[r["page"]].append(r)

total = len(rows)
touched = sum(1 for r in rows if r.get("ax_interaction_ok"))
verified_pages = sum(1 for p in pages
                     if all(r.get("navigation_verified") for r in by_page[p["page"]]))
with_action = sum(1 for r in rows
                  if (r["page"], r["role"], r["name"]) in executed)

summary = {
    "window": ax["window"],
    "pages_discovered": len(pages),
    "pages_navigation_verified": verified_pages,
    "functional_points": total,
    "ax_touched": touched,
    "ax_touch_rate": round(touched / total, 4) if total else 0,
    "mutating_actions_observed": with_action,
    "action_rate": round(with_action / total, 4) if total else 0,
    "role_counts": dict(Counter(r["role"] for r in rows).most_common()),
    "screenshots_page_discovery": len(list(HERE.glob("page-*.png"))),
    "screenshots_ax_pass": len(list(HERE.glob("ax-*.png"))),
    "pages": [],
}

lines = [
    "# 全功能触达覆盖矩阵（agent-desktop 真窗口）",
    "",
    f"- 窗口：`{ax['window']}`；页面 **{len(pages)}** 个，导航验证通过 **{verified_pages}/{len(pages)}**。",
    f"- 功能点 **{total}** 个；真实 AX 交互触达 **{touched}**（**{summary['ax_touch_rate']:.1%}**）。",
    f"- 其中另有 **{with_action}** 个功能点留下了变更型动作（click/toggle/expand/set-value 等）的成功证据（{summary['action_rate']:.1%}）。",
    "",
    "> **两个口径不可混淆**：`get --property states` 是对每个控件的真实无障碍交互（读取状态、可操作性），",
    "> 证明该控件在当前构建里可达且可操作；它**不改变界面**，因此不等价于「执行了该功能」。",
    "> 续作（`action-coverage.py`）用「同 snapshot + `click` 批量」把变更动作推到 119 条；",
    "> 全量动作执行仍未完成（详见 OUTCOME §9），**不宣称 100%**。",
    "",
    "## 按页面",
    "",
    "| 页面 | 预期控件 | 实测控件 | 导航验证 | AX 触达 | 触达率 | 变更动作证据 |",
    "| --- | ---: | ---: | :---: | ---: | ---: | ---: |",
]
for p in pages:
    name = p["page"]
    rs = by_page[name]
    ok = sum(1 for r in rs if r.get("ax_interaction_ok"))
    acts = sum(1 for r in rs if (name, r["role"], r["name"]) in executed)
    verified = all(r.get("navigation_verified") for r in rs)
    lines.append(
        f"| {name} | {p['unique_controls']} | {len(rs)} | {'✅' if verified else '❌'} | "
        f"{ok} | {ok / len(rs):.0%} | {acts} |"
    )
    summary["pages"].append({
        "page": name, "expected": p["unique_controls"], "observed": len(rs),
        "navigation_verified": verified, "ax_touched": ok, "mutating_actions": acts,
    })

lines += ["", "## 角色分布", "", "| 角色 | 功能点 |", "| --- | ---: |"]
for role, n in summary["role_counts"].items():
    lines.append(f"| `{role}` | {n} |")

lines += [
    "",
    "## 未触达功能点",
    "",
]
missing = [r for r in rows if not r.get("ax_interaction_ok")]
if missing:
    lines += ["| 页面 | 角色 | 名称 | 错误 |", "| --- | --- | --- | --- |"]
    for r in missing:
        lines.append(f"| {r['page']} | {r['role']} | {str(r['name'])[:48]} | `{r.get('error')}` |")
else:
    lines.append("**无。** 1193 个功能点全部完成真实 AX 交互，无遗漏。")

lines += [
    "",
    "## 仍未达成的口径（不隐藏）",
    "",
    "- 「执行每一个功能」尚未达成：119 个功能点有变更动作证据，集中在 5 个导航验证通过的页面；其余 9 页的全量动作执行因窗口状态污染与耗时（单次运行 40 分钟仅完成 6 页）而中止，见 OUTCOME §9。",
    "- 63 个静态路由中只有 14 个页面状态经真窗口 UI 可达并验证；其余需 deep link / 特定数据，未逐一验证。",
    "- 错误、加载、流式、故障注入、真模型任务、键盘焦点环、reduce 开启态未覆盖。",
    "",
    "## 截图证据",
    "",
    f"- 页面发现阶段 `page-*.png`：**{summary['screenshots_page_discovery']}** 张（每页一张）。",
    f"- AX 全量触达阶段 `ax-*.png`：**{summary['screenshots_ax_pass']}** 张（每页一张）。",
    "- 逐页 UX 分析见 `ux-analysis.md`。",
]

(HERE / "coverage-matrix.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
(HERE / "summary.json").write_text(
    json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps({k: v for k, v in summary.items() if k != "pages"},
                 ensure_ascii=False))
