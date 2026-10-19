# Task-corpus external oracle directory

Date: 2026-10-18

## Result

`apps/work/tests/e2e/task-corpus/score_office.py` accepts
`--oracle-root PATH`. The default remains its adjacent `oracle` directory.
The parameter flows through `main` → `score_dir` → `Ctx` → `oracle_rules`;
the existing must/forbid parsing and every task-specific scoring predicate
and expected phrase are unchanged. No corpus source or answer was copied
into this repository.

The prior entry point passed `sys.argv[1:]` but expected two entries and
read index 1. Standard `argparse` now accepts the documented single run
directory, provides `--help`, and rejects missing/unknown arguments.

Self-check oracle access is confined to temporary synthetic directories,
including when `--oracle-root` is also supplied. It does not inspect the
external or repository-default oracle. Four synthetic checks cover missing
oracle, matching must/absent forbid, missing must, and present forbid.

## Changes and callers

- Only the scoring script and this evidence file were edited in this slice.
- Added an optional `oracle_root` argument with the original default to
  `Ctx`, `oracle_rules`, and `score_dir`; `apply_oracle` forwards the context.
- Added the CLI parser and four synthetic oracle self-checks. Existing
  self-check calls now explicitly use a temporary missing-oracle directory.
- Repository caller search found only this script's own `score_dir` and
  `oracle_rules` calls. Documentation still uses the valid default command.
- The task-corpus directory was already untracked; unrelated files and
  concurrent edits were retained. No GUI, task execution, or system
  permission change was performed.

## Validation

Executed from the repository root with `PYTHONDONTWRITEBYTECODE=1`:

| Check | Result |
| --- | --- |
| `python3 apps/work/tests/e2e/task-corpus/score_office.py --self-check` | `SELF-CHECK PASS (22 pass, 3 blocked, 5 negative, 4 external oracle)` |
| `python3 apps/work/tests/e2e/task-corpus/score_office.py --help` | Exit 0; exposes optional run, `--oracle-root`, and synthetic `--self-check` |
| Imported CLI probe with one synthetic case | 3/3 pass: default oracle, flag before run, flag after run all reach `Ctx` with the expected path |
| Imported CLI invalid-argument probe | 2/2 pass: no arguments and unknown option both exit 2 |
| `main(['--self-check', '--oracle-root', external])` with intercepted `oracle_rules` | Pass; all 8 oracle accesses were in temporary synthetic directories; neither the external nor default oracle was read |
| External input availability | The four expected files under `/tmp/aster-task-corpus-cua-20261018-2245/oracle` exist and are readable; contents were not printed |

The CLI routing probe replaced `CASES` only inside its temporary imported
module with a function that recorded `ctx.oracle_root`. It used a temporary
`TC-PROBE/reply.txt`, never scored or displayed real oracle facts, and did
not alter production predicates.

The combined-mode probe wrapped the real `oracle_rules`, asserted every
root was named `missing-oracle` or `external-oracle` under the self-check's
temporary directory, and then ran the actual complete self-check.

## Explicitly not claimed

- These checks do not establish any task completion rate or GUI E2E result.
- External OCR facts were not independently reviewed in this slice.
- No scoring tolerance, file-type rule, acceptance phrase, or oracle rule
  was changed.
- Self-check still uses the existing task source workbooks for its original
  A07/A08 fixtures; only oracle answers are synthetic.
