# A35 formula-cache hypothesis disproved

Read-only diagnosis. No product source, scorer, workbook, configuration,
account, live process, or deployment was changed.

The frozen artifact is
`/tmp/aster-task-corpus-cua-20261018-2245/run/TC-A35/delivery-1-合并汇总.xlsx`:
5,267 bytes, SHA-256
`a35a9ef62374ba714a7071856f7320a9dcd66f82d9d44c01ea54ee519f3c33b4`.
Python standard-library `zipfile` and ElementTree inspection of only workbook
metadata and formula cells found all six persisted formula caches:

| Summary cell | Formula kind | Saved `<v>` |
| --- | --- | --- |
| B2 | Cross-sheet COUNTIF sum | 15 |
| C2 | Cross-sheet SUMIF sum | 225 |
| B3 | Cross-sheet COUNTIF sum | 15 |
| C3 | Cross-sheet SUMIF sum | 240 |
| B4 | SUM(B2:B3) | 30 |
| C4 | SUM(C2:C3) | 465 |

All six `<f>` expressions have the corresponding `<v>` above.
`calcPr` records `calcCompleted=1`, `calcMode=auto`, `forceFullCalc=0`, and
`fullCalcOnLoad=0`. The independent artifact QA agent also reopened the same
bytes with `openpyxl(data_only=True)` and the frozen scorer's `load_xlsx`,
obtaining these same values. The workbook did not lack cached totals.

Source inspection confirms that explicit formula objects enter the native
engine, not a caller-supplied cache. `office_file::build_office` calls
`recalculate_spreadsheet_formulas`, checks formula count, refuses error values
and spill-limit breaches, saves, reopens, and returns text readback. Ordinary
string cells remain literal; the explicit formula input rejects a supplied
`value` field. The native editor recalculation uses the atomic mutation path;
its write planner writes both the formula and typed cached `<v>` value.
`SUMIF` and `COUNTIF` operate on resolved in-workbook range sheet identities,
with bounded rectangle visits and existing fail-closed unsupported criteria.
There is no evidence here for a writer or calculation repair.

The frozen strict receipt remains **FAIL** with missing-total notes. Its
unchanged scorer SHA is
`9f1f61f9a089f7a820482bb92ee80ff1ca544fd74b7e178796a58d2e6fa542a7`.
The predicate at `score_office.py::score_a35` flattens all summary cells into
text. `flat` removes whitespace separators; the independent preceding counts
therefore join the totals as `15225`, `15240`, and `30465`. The existing
numeric-boundary predicate cannot find 225/240/465 inside those longer digit
runs. A pure Python reproduction on the three rows returns false for all
three boundary matches. This is a scorer flattening defect, not absent output.

Commands: standard-library ZIP/XML inspection of the exact path above;
bounded source reads of `office_file.rs`, native formula recalculation/writer,
`sum_if.rs`, `count_if.rs`, and scorer `flat`/`score_a35`; a three-row pure Python
reproduction of whitespace flattening plus the existing digit-boundary regex.
No build or test run was needed because no behavior was changed.

## Explicitly not claimed

- No change from the frozen strict FAIL to PASS or an extra passing task.
- No repair, additional task output, hand-filled totals, or scorer relaxation.
- No claim that structural readback proves arbitrary task semantics or that
  every supported formula is independently re-evaluated after reopening.
- No live session/configuration/account inspection, model call, app restart,
  product rebuild, installation, or deployment.
