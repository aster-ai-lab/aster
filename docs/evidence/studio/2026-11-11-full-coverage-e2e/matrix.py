#!/usr/bin/env python3
"""Turn coverage.json into a readable, auditable coverage matrix."""
from __future__ import annotations

import json
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
data = json.loads((HERE / "coverage.json").read_text(encoding="utf-8"))
rows = data["coverage"]
pages = json.loads((HERE / "pages.json").read_text(encoding="utf-8"))["pages"]

by_page: dict[str, list[dict]] = defaultdict(list)
for r in rows:
    by_page[r["page"]].append(r)

total = len(rows)
touched = [r for r in rows if r.get("touched")]
ok = [r for r in rows if r.get("touched") and r.get("ok")]
misses = [r for r in rows if not (r.get("touched") and r.get("ok"))]

summary = {
    "window": data["window"],
    "pages_discovered": len(pages),
    "functional_points": total,
    "touched": len(touched),
    "action_delivered": len(ok),
    "touch_rate": round(len(touched) / total, 4) if total else 0,
    "delivery_rate": round(len(ok) / total, 4) if total else 0,
    "misses": len(misses),
    "action_counts": dict(Counter(r.get("action") or "not-touched" for r in rows).most_common()),
    "error_counts": dict(Counter(r.get("error") or r.get("reason") or "none"
                                 for r in misses).most_common()),
    "screenshots": [s["file"] for s in data.get("screenshots", []) if s.get("ok")],
    "pages": [],
}

lines = [
    "# 全功能触达覆盖矩阵（agent-desktop 真窗口）",
    "",
    f"- 窗口：`{data['window']}`；发现页面 **{len(pages)}** 个；功能点 **{total}** 个。",
    f"- 已触达 **{len(touched)}** / {total}（{summary['touch_rate']:.1%}），动作成功投递 **{len(ok)}**（{summary['delivery_rate']:.1%}）。",
    "- 口径：功能点 = 每个页面 AX 树中唯一的 `(role, name)` 可交互控件；页面内同名重复实例视为同一功能点。",
    "- 导航类控件在页面发现阶段被真实点击（记为 `navigate-click`），页面循环里不重复点击以免离开该页。",
    "- 开关类用 toggle 两次还原，避免改写用户配置；下拉类用 expand/collapse 或 click 回退。",
    "",
    "## 按页面",
    "",
    "| 页面 | AX 节点 | 功能点 | 已触达 | 动作成功 | 未成功 | 截图 |",
    "| --- | ---: | ---: | ---: | ---: | ---: | --- |",
]

for p in pages:
    name = p["page"]
    rs = by_page.get(name, [])
    t = sum(1 for r in rs if r.get("touched"))
    o = sum(1 for r in rs if r.get("touched") and r.get("ok"))
    shot_name = f"page-{p['index']:02d}-{name}".replace("/", "-").replace(" ", "_") + ".png"
    lines.append(
        f"| {name} | {p['ax_nodes']} | {len(rs)} | {t} | {o} | {len(rs) - o} | `{shot_name}` |"
    )
    summary["pages"].append({
        "page": name, "ax_nodes": p["ax_nodes"], "functional_points": len(rs),
        "touched": t, "delivered": o, "missed": len(rs) - o,
    })

lines += ["", "## 动作分布", "", "| 动作 | 次数 |", "| --- | ---: |"]
for action, count in summary["action_counts"].items():
    lines.append(f"| `{action}` | {count} |")

lines += ["", "## 未成功功能点（逐项，不隐藏）", ""]
if not misses:
    lines.append("无。全部功能点均已触达且动作成功投递。")
else:
    lines += ["| 页面 | 角色 | 名称 | 动作 | 错误 | 原因 |", "| --- | --- | --- | --- | --- | --- |"]
    for r in misses:
        lines.append(
            f"| {r['page']} | {r['role']} | {str(r['name'])[:48]} | "
            f"`{r.get('action') or '-'}` | `{r.get('error') or '-'}` | "
            f"`{r.get('reason') or '-'}` |"
        )

lines += ["", "## 截图证据", "", "每个可导航页面在发现阶段与触达阶段各留有截图（见目录内 `page-*.png` / `touch-*.png`），浮层另有 `overlay-*.png`。"]
for s in summary["screenshots"]:
    lines.append(f"- `{s}`")

(HERE / "coverage-matrix.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
(HERE / "summary.json").write_text(
    json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
)
print(json.dumps({k: v for k, v in summary.items()
                  if k not in ("pages", "screenshots")}, ensure_ascii=False))
