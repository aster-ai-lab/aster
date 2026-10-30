# Documentation editorial review

Date: 2026-10-26. Scope: the documentation portal, four Aster Code MDX articles, project summaries, figure labels, captions and shared page copy.

## Review and changes

The original copy used promotional triplets, unexplained internal terms and repeated qualification notices. The portal also described site-maintenance details that belonged in the website README.

- Replaced slogans such as “行动有边界 / 过程有记录 / 完成有依据” with descriptions of tool execution, saved records and completion checks.
- Explained Core, host applications and the fact log before discussing their behavior. Replaced “状态投影” with rebuilding state from the log and “子任务收口” with obtaining and processing child-task results.
- Used concrete examples: a write accepted before its receipt is saved, a test report becoming outdated after another edit, and a verification command changed by adding `|| true`.
- Removed redundant warnings and reader-facing site-maintenance details. Kept the relevant limitations: unresolved external operations, verification scope, explicit memory activation and absence of comparative benchmarks.
- Removed conflicting step numbers from the workflow diagram: the diagram depicts a cycle, while the article's numbered sections describe the task lifecycle.
- Clarified that child agents still pass permission checks; their permission sets need not be identical to the parent's. Clarified session creation to match the Rust example.

## Checks

| Check | Result |
| --- | --- |
| `npm --prefix apps/web run build` | 5 MDX pages rebuilt |
| `npm --prefix apps/web run check` | All generated pages match source |
| `npm --prefix apps/web test` | 3 focused tests passed; [log](node-checks.log) |
| `python3 -m unittest discover -s apps/web/tests -p 'test_*.py' -q` | 19 passed; [log](python-checks.log) |
| `python3 apps/web/scripts/check_site.py` | 25 public files, 5 documentation pages passed |
| `git diff --check -- apps/web` | Passed |
| A3S Test interactive browser review | Desktop workflow at 1440 × 1000 and mobile portal at 390 × 844 inspected; labels fit; [no page errors](browser-errors.json); [session finished](browser-review.json) without cleanup error |

Screenshots: [desktop workflow](workflow-revised-desktop.png), [mobile portal](portal-revised-mobile.png).

## Explicitly not claimed

This was an editorial review by the coding agent, not a study with independent readers. Readability is a judgment; no automatic “AI writing” score or word blacklist was used. Technical identifiers and necessary implementation limits remain. No runtime functionality, source-link availability or cross-framework performance was requalified, and no deployment was performed.
