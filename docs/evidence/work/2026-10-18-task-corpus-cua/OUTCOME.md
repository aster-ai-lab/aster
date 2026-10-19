# Task corpus: background CUA replay

Status: **HANDOFF READY — stopped at the user's request; v9 formal 0/22 pending**.
V9 is deployed with all three CUA functional gates PASS and a new frozen baseline;
no v9 original task has been submitted or collected. The app and owned renderer
remain available for the next agent. The completed, independent v8 first-attempt
strict matrix remains **13 PASS / 9 FAIL (59.1%)**, with zero retries; all **20
successful v8 turns have independent source/artifact QA** and A02/A09 retain
original error diagnostics. QA findings do not change frozen strict results.

The next agent should read [the complete handoff prompt](HANDOFF-PROMPT.txt)
before acting. It includes the original prompts and allowed attachments,
frozen build identity, collection workflow, known defects and cleanup steps.

## Current v8 independent batch

- Root deployed v8 after native build session 90802 exited 0 and rechecked the
  38 scoped source hashes and paired binaries. Build UUID:
  5CCF2AC1-36FF-3FBF-9EF2-FDA690A9DD16. Baseline host was PID 69840;
  after root's v8 background quit/relaunch the last observed v8 host was
  PID **82537**, normally stopped before the separate v9 deployment. Owned preview
  session 52718 serves the frozen renderer on port 4319, reused by v9.
  The frozen baseline PID remains unchanged. [Build evidence](/tmp/aster-task-corpus-cua-20261018-2245/evidence/build-identity-v8.json).
- SH-01/02/03 all have actual CUA functional PASS records. SH-02 includes a real
  completed read tool; SH-03 restores the attachment/read/first sentence and
  full-access/Qwen/planning-off/idle state, then the separate 08:57 42 task.
  [Gate evidence](/tmp/aster-task-corpus-cua-20261018-2245/evidence/v8-qualification-gates.json)
  remains separate from corpus outcomes.
- The independent [formal baseline](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-baseline.json)
  freezes cutoff 2026-10-19T00:59:48.700832+00:00, excludes 60 older sessions,
  and records 52 allowed input hashes plus the 38 source bindings. The
  [Qwen v8 ledger](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-ledger.json)
  contains only this batch. Prior GLM and v5 Qwen outcomes are retained below
  without importing their scores. Raw submitted prompt hashes retain recorded
  attachment trailing spaces; replies and artifacts are unchanged.
- Root actually opened and verified the UI terminal for all 22 collected cases.
  Every existing-collector invocation exited 0; collected reply/artifact byte
  counts and SHA-256 were independently matched. Deliveries are under
  /tmp/aster-task-corpus-cua-20261018-2245/run-qwen-v8/TC-Axx/.
  The frozen scorer SHA-256 remains
  9f1f61f9a089f7a820482bb92ee80ff1ca544fd74b7e178796a58d2e6fa542a7.
  Per-case receipts, logs and scores are linked from the ledger.

| Case | Frozen strict | Original notes |
| --- | --- | --- |
| A01 | FAIL | 缺少「报表系统升级」 |
| A02 | FAIL | 图表文件 0 个，需要趋势、柱状、散点各一; 缺少「产品A」; 缺少「产品B」; 缺少「订单」; 缺少「大客户部」; 缺少「销售三部」; 缺少「广告」; 缺少数字 268; 缺少数字 98 |
| A03 | PASS | 要点都在 |
| A04 | PASS | 要点都在 |
| A06 | PASS | 要点都在 |
| A07 | FAIL | R003 出现 2 次，应为 1 次; R004 出现 2 次，应为 1 次; R007 出现 2 次，应为 1 次; 缺少日期 2026-01-08; 缺少日期 2026-01-09; 缺少日期 2026-01-10; 缺少日期 2026-01-12; 缺少日期 2026-01-13 |
| A08 | PASS | 要点都在 |
| A09 | FAIL | 图表文件 0 个，需要趋势图和区域对比图; 缺少数字 124 |
| A10 | PASS | 要点都在 |
| A12 | FAIL | 缺少「风险中」 |
| A23 | PASS | 要点都在 |
| A24 | PASS | 要点都在 |
| A25 | PASS | 要点都在 |
| A26 | PASS | 要点都在 |
| A28 | FAIL | 缺少「未完成」 |
| A29 | PASS | 要点都在 |
| A30 | PASS | 要点都在 |
| A35 | FAIL | 汇总页缺少 225; 汇总页缺少 240 |
| A36 | FAIL | 缺少第 5 周增长 50%（增长50% / +50%）; 缺少第 6 周下降 50%（下降50% / -50%） |
| A40 | PASS | 要点都在 |
| A49 | PASS | 要点都在 |
| A51 | FAIL | 缺少「印证材料」; 缺少「自发布之日起执行」; 缺少一级标题缺「三」（缺三 / 缺少三 / 缺失三 / 直接四 / 跳到四） |

[Complete first-attempt scores JSON](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-attempt1-scores.json),
[standalone matrix](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-attempt1-matrix.md),
and [final receipt/hash/anchor validation](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-attempt1-final-validation.json)
freeze only the original 22 cases. The final single authorized execution-index
projection at 2026-10-19T01:49:01.373967+00:00 finds 20 done / 2 error with every
original execution and anchor unchanged. All 22 receipt JSON objects match their
collector logs and ledger entries; all 22 frozen score files match identities,
status and notes. All 32 collected files match their receipts or A07 source-hash
sidecar, with no unexpected files or case directories. No session messages or
configuration were read for this final verification.

- A07 has one traced XLSX (6,498 bytes) and 3,223 reply bytes. Post-turn hashing
  of the original task source and the exact session's sole tobot-inbox upload
  both matches preflight SHA-256
  2141a61466d714e2d95aa0fc48aa204a3f0203f2635a43d767feb467b525fcdc,
  unchanged across collection. Only immutable receipts were collected; no other
  workspace output was copied. [Source integrity](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A07-attempt1-source-integrity.json),
  [receipt](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A07-attempt1-collection-receipt.json),
  [strict score](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A07-attempt1-strict-score.json).
  [Independent artifact QA](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A07-attempt1-independent-qa.json)
  confirms the core cleaned sheet's ten unique rows, real Excel dates, missing
  markers, package integrity and formula caches are correct. A missing-items
  supplement repeats three IDs; source-description counts and quoted date
  spellings are wrong. The two authorized parser outputs retain slash spellings,
  so the explanation's hyphen substitution is not parser normalization. Full
  Excel application rendering was not performed; strict FAIL remains frozen.
- A10's [receipt](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A10-attempt1-collection-receipt.json)
  retains one 30,633-byte PPTX. A35's [receipt](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A35-attempt1-collection-receipt.json)
  retains one 6,721-byte XLSX. A36's [receipt](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A36-attempt1-collection-receipt.json)
  retains all six traced artifacts: three PNG charts and all three DOCX versions,
  including two drafts. No final-version selection or artifact correction was
  performed. [A10 independent QA](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A10-attempt1-independent-qa.json)
  finds the core facts and 90/82/95 chart correct, but the cover turns a Q1
  go-live target into an actual event. Strict PASS is not full semantic accuracy.
  [A35 independent QA](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A35-attempt1-independent-qa.json)
  confirms four sheets, 30 unique records and correct 225/240/465 values/caches.
  Frozen scoring concatenates neighboring numeric cells into 15225/15240 and
  misses their digit boundaries; strict FAIL remains unchanged.
  [A36 independent QA](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A36-attempt1-independent-qa.json)
  confirms all three versions' tables and all three original PNGs, with 19
  calculated rates and two unavailable rates. Draft descriptions contain errors;
  the final DOCX fixes the counts but retains the incorrect arithmetic statement
  (22-20)/20 = 9.09%, whose actual value is 10%. Source-required two-decimal
  percentage spellings separately explain the two strict word-form failures.
  The original embedded PPTX PNG and three original chart PNGs were viewed;
  full presentation, Excel and DOCX page rendering were not performed.
- Independent semantic QA is linked for A26/A29/A30/A40 in the ledger and
  evidence/original22-qwen-v8-Axx-attempt1-independent-qa.json. A29's join and
  dates recalculate correctly. A26 has excessive speculative questions without
  asserting them as confirmed issues. A30 mixes author risk inferences into a
  facts section and missing/incomparable items into conflicts. A40's capability
  boundaries are correct, with an excessive missing-information list. Root also
  reports unsupported price/cause claims in A08; this concern is not resolved by
  strict PASS. These findings remain separate from strict scoring.
- All remaining original cases, including A12/A03/A23/A24/A25, were collected
  only after root's exact actual UI terminal confirmations. A04/A06/A12 use the
  existing frozen external OCR oracle through the unchanged scorer; no answer
  was rewritten. A23/A24's rejected CUA clicks were guard events and did not
  create a new submission or retry.
- Independent source/artifact QA now has evidence for **20 of 22 cases**, linked
  from the ledger. [A04 QA](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A04-attempt1-independent-qa.json)
  verifies the 272-character summary, five conclusions and three recommendations
  against all 12 original PDF pages. [A06 QA](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A06-attempt1-independent-qa.json)
  verifies all three original pages and the contract fields/calculations.
  [A12 QA](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A12-attempt1-independent-qa.json)
  confirms the core comparison values but finds unsupported causal links,
  inferred prepayment/acceptance protection and unconditional best-choice claims.
- The additional A01/A03/A08/A23/A24/A25/A49/A51 source QA records are additive
  references, with no strict-result change. A01 omits the project name and
  invents a Word heading-level change. A08 has material derived arithmetic and
  comparison-population errors as well as unsupported price/cause claims. A51
  misses the source's 佐证材料/印证材料 terminology mismatch; the missing-three
  numbering issue was nevertheless found semantically. A03/A23 preserve the
  requested source facts; A24/A25/A49 have mostly correct core results with
  qualified speculation, cautious classification or minor overstatement.
  These eight reviews explicitly disclose an accidental TEST-CASES acceptance-
  column exposure during prompt-source extraction and **are not blind reviews**.
  Findings are tied to original source locations/independent calculations; no
  oracle or answer PNG was read and tested model inputs/artifacts were unchanged.
- [A28's new source-only QA](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A28-attempt1-independent-source-qa.json)
  verifies all three rows/seven fields and ID-specific separation. It confirms
  the 100%, missing progress, 12-of-20/60% and pending-two facts. The missing
  explicit 未完成 wording is a clarity note, while the original strict FAIL is
  retained. The older separate QA document was preserved and not used for this
  source-only review. All 20 successful original turns now have independent QA;
  A02/A09 have error diagnostics rather than completed-delivery semantic approval.
  Office package/geometry checks and viewed embedded/chart PNGs do not establish
  full application/page rendering. No single semantic pass rate is inferred from
  varied QA classifications or from 13 strict PASS results.
- Root selected **all original22** for a separate v9 retry1 batch. Its independent
  [prepared ledger](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v9-retry1-ledger.json)
  records **22 selected / zero submissions**, one planned retry relative to each
  v8 first attempt. V9 was initially launched as PID 98039 and, after the normal
  gate-3 quit/restart, the current exact-name process is PID **98622**. Build UUID:
  766FF8E2-09F0-3EFD-ABD4-0736B3A933A2, with frozen paired ACP/Office installed as
  application MacOS siblings to isolate shared-target rebuilds. The actual smoke
  ACP child 98127 uses the installed sibling path, as recorded without argv/env
  inspection. [V9 build identity](/tmp/aster-task-corpus-cua-20261018-2245/evidence/build-identity-v9.json),
  [runtime process projection](/tmp/aster-task-corpus-cua-20261018-2245/evidence/v9-smoke-runtime-process-projection.json).
  Root's actual 10:01 no-attachment 42 smoke is SH-01 functional PASS. SH-02's
  actual 10:03 attachment smoke preserves the exact first sentence; metadata
  independently matches its copied source URI and allowed-input SHA, typed Qwen
  snapshot and host done. Raw prompt hashes retain leading/trailing attachment
  ASCII spaces. SH-03 confirms history, attachment, exact reply, Qwen, full access,
  planning off and idle restoration after a normal quit/relaunch. All three
  [gate records](/tmp/aster-task-corpus-cua-20261018-2245/evidence/v9-qualification-gates.json)
  are functional PASS, separate from original-case outcomes. The independent
  [formal baseline](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v9-retry1-baseline.json)
  freezes cutoff **2026-10-19T02:06:58.754488+00:00**, excludes all **84 existing
  sessions**, and verifies **52 allowed input / 49 scoped source / 3 installed
  binary hashes**. Baseline SHA-256:
  b1b1b006582a3ecd3181676b97ab4bb9571a928cbcb4ec7936b01a3ed7a73b81.
  No original task has been retried; the selected 22 remain pending for handoff. The
  [runbook](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v9-retry1-runbook.md)
  prepares the same exact metadata binding, existing collector and frozen
  per-case scorer. Evidence-only preparation submitted and collected no original
  task; no v8 session or cutoff will be reused for v9. No further formal task is
  authorized by this freeze; the user requested handoff to another agent.
- The single fresh projection authorized after the earlier report update, at
  **01:21:41.634813 UTC**, finds **20 done / 2 error**, zero streaming, and all 22
  original anchors unchanged. The earlier 01:16:18 projection remains historical
  evidence. [Latest projection](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-status-snapshot-after-report-update.json).
  A02's exact typed error is max_tokens; A09's is completion-gate refusal because
  the mutation containing two TC08 chart PNGs and two probe PNGs lacks bound
  Passed verification. [Error evidence](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-two-terminal-errors.json).
  Root subsequently verified both original error turns in the UI. Both collector
  receipts preserve only reply.txt, with zero partial artifacts; the A09 trace
  cards were not copied. Later bridge repair cannot upgrade these original errors.
- From v8 A35 onward, actions used complete fresh AX via
  getAXState({disableDiffing:true,emit:false}); unique label/URI guards emitted
  matched targets before indexed actions. Each action was followed by fresh AX,
  with full trees emitted after critical sends and terminal checks. No stale/diff
  numeric index was reused. The missing-window failure occurred during querying,
  before a click; the v7 historical procedure remains a fact about that earlier run.
- After all original anchors were terminal, root normally quit PID 69840 with
  targeted background Super-Q and confirmed exact ps exit 1/gone. Two background
  getApp relaunches still returned cgWindowNotFound; process inspection found the
  same v8 bundle main at PID 82537. [Failed recovery evidence](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-ui-recovery-attempt.json)
  remains unchanged. After the user answered the desktop-recovery question,
  root's fresh getApp succeeded with v8/4319 and A07 history/full Qwen/planning
  off/idle restored. [Window-restored evidence](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-ui-window-restored.json).
  The block is resolved for further UI checks; its cause remains unknown. No
  foreground/Raise/global mouse/shell-kill fallback or product repair is claimed.
- The authorized [post-batch identity check](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-post-batch-build-identity.json)
  matches all **4 binaries** and **34 Work-related source files**. Of the full
  frozen 38-source scope, Studio turns.rs and workbench_turns.rs differ. The raw
  snapshot remains DRIFT_DETECTED; those changes are unreviewed, outside Work's
  compile paths, and were not reverted. This checks measured before/after file
  identities, not mapped process memory or outstanding UI outcomes. Post-replay
  candidate source changes below are excluded from original-v8 attribution.

## Post-replay source follow-ups, not deployed

After the frozen v8 post-batch identity check, the Work delivery contract and
two existing test modules were updated to constrain supplemental worksheets,
probe files, source-fact labels and final-answer scope. Three host tests and
one real-ACP/loopback-provider transport test pass; no original-case result is
upgraded. [Source-check evidence](../2026-10-19-delivery-scope-and-facts/OUTCOME.md).

A02's [narrow output-cap diagnosis](/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-qwen-v8-A02-attempt1-output-cap-diagnostic.json)
confirms genuine `max_tokens` termination but cannot establish the effective
request cap or usage from the persisted metadata. A separate real-Harness
regression reproduces A09's missing proof handoff across `program` calls;
repair and rejection-case checks are in progress. These later source edits do
not alter the installed frozen v8 binary or its original collected deliveries.

## Historical pre-v8 status checkpoint

Status: **IN PROGRESS — original 22 in full-access mode; GLM batch closed**.
The combined historical ledger has fifteen distinct cases / sixteen formal
attempts: six cases have frozen strict results (1 PASS / 5 FAIL), nine original
error turns remain uncollected, and seven cases have never been submitted.
All fifteen latest original anchors are terminal (6 assistant success / 9 error).
This aggregate includes historical v5 Qwen evidence and is not a single-model
matrix. The separate **v7 GLM-only matrix** has 14 cases: 5 assistant success /
9 error; 5 strict scores (1 PASS / 4 FAIL) and 9 unscored. A new independent
`run-qwen-v8` is prepared with **zero submissions**; deployment/baseline is not
yet claimed. All six scored first attempts failed; A03's earlier v5
FAIL is preserved beside its new v7 retry PASS. A07's cleaned data is independently
correct but its four workbooks and unrequested source-copy sheets repeat records.
V7 is installed,
frozen, and launched; SH-01/02 have functional full-access/GLM 5.3 PASS results.
SH-03 history/attachment/permission recovery passes, while GLM model preference
restoration fails separately and is not claimed as covered.
The earlier v5 SH-01,
SH-02, and SH-03 passes remain historical evidence. Two earlier
v4 A03 full-access requests were downgraded by the product to per-call
confirmation and are retained only as diagnostics.
The earlier A07 confirmation-mode submission is retained
as diagnostic history; it is not a formal full-access attempt and does not use
that run's retry allowance.
The current user goal is to rerun the original 22 `../tasks` cases through CUA
without taking the mouse, fix actual failures, and rerun affected cases. Shell
qualification, earlier sessions, and controlled ACP tests are separate evidence
and do not count as original-case completion.

## Current v7 deployment and bounded qualification

- Root's final v7 `just build-work` exited 0; frozen renderer preview build
  exited 0 in 12.6s. The busy same-task-ID fixes in both sources, two reviewed
  permission drifts, delivery contract, and effort forwarding were bound to
  stable source baselines before and after the build. Evidence:
  `evidence/build-v7-20261019.log`, `build-v7-source-baseline.json`,
  `renderer-v7-build.log`, and `renderer-v7-source-baseline.json`.
- V5 PID 43431 normally quit before the source main was installed, signed, and
  registered. `evidence/build-identity-v7.json` records a pre-signing copy match,
  verified signature, and UUID `9D7C1C07-4126-326A-9BDA-F2F84EE35BF8`.
  Paired ACP SHA-256 is
  `8ce93b4693cae437970675f5ce77afd712be0f657a639b51d623da9d5481f6b3`;
  Office SHA-256 is
  `8fc8689a26dfab0898d4262df19e1d00f5ef5efc7d80c230b3cc4fec44961613`.
  This development bundle has no bundled companions; resolution uses current
  `target/debug` paths. Exact process metadata confirmed initial installed host
  PID 54654 and ACP child PID 55104. The raw child comm spelling is
  `/Users/roylin/code/aster/apps/work/src-tauri/../../../target/debug/a3s-code-acp`;
  canonical path resolution confirms the current paired debug path, with the
  raw spelling retained. This checks executable paths, not mapped-process bytes.
- V7's owned preview execution session 12541 serves the same isolated 4319
  origin. Prior owned session 8077 ended after Ctrl-C with exit 130; shared
  development port 4318 was not touched. The v7 manifest has 2,639 files and ten
  renderer source bindings; SHA-256
  `0cfa2824893eaa3193ae09f1154ef41441eed99ba24f45b4a63746a5a9f68fa2`.
  `evidence/renderer-v7-http.json` reports HTTP 200 and an exact frozen-file
  match with HTML SHA-256
  `22c09e8238c58b033bbd665eb6acbb0ebad491d23e7ff96fb87eeeb9341e3e26`.
- V6 was never adopted: its build started after a failed source-preflight
  assertion. Its rejected status remains in
  `evidence/renderer-v6-not-adopted.json`; the later reviewed/tested permission
  changes are bound by v7 instead. No unrelated hunk was rolled back.
- V7 baseline `evidence/original22-v7-full-access-baseline.json` freezes cutoff
  `2026-10-19T00:17:45.252747+00:00` and 40 old session IDs. It explicitly retains
  A03/A07's completed strict failures and does not reset formal retries or the
  2 submitted / 2 failed / 20 not-run progress. Root selected **GLM 5.3** in the
  UI. V7 host/companion paths are observed. SH-01/02 qualification and SH-03's
  bounded history/permission recovery are complete; model preference recovery is
  a separate defect. Provider payload inspection was not performed here.
- V7 SH-01 was sent through a new homepage after explicitly selecting GLM 5.3;
  after Send the UI showed GLM 5.3 and full access. Baseline-40 exclusion found
  exactly one new execution: session `01a15187-2822-7567-a7b5-926622986293`,
  anchor/execution `01a15187-28fc-7245-a227-28f357d89a9f`, created
  `2026-10-19T00:19:34.563Z`, metadata done `2026-10-19T00:19:40.934Z`.
  Its whitelisted actual agent mode is `bypassPermissions`. The operator then
  confirmed reply `42`, Pause gone, Send disabled, and retained full-access /
  GLM 5.3: functional SH-01 PASS. Record:
  `evidence/sh01-v7-initial-metadata-20261019.json`. An old A07 task's Qwen model
  display is expected historical state and is not used to identify this new gate.
- V7 SH-02: an independent new task submitted exact question
  `请说出所附文件的第一句。` plus the sole original A03 TXT. Before/after Send,
  full access, GLM 5.3, trace on, and auto-review off were confirmed. The read
  tool completed automatically; UI returned
  `这周订单模块联调差不多搞定了，目前完成率85%。`, Pause disappeared and Send was
  disabled: functional SH-02 PASS. Session
  `01a15188-c26a-71e1-a441-b9f24dff66a8`, anchor/execution
  `01a15188-c36f-76f5-8d0a-b5abda47ff10`. Exact read-only execution metadata
  independently confirms `done` at `2026-10-19T00:21:29.341Z`; record
  `evidence/sh02-v7-execution-metadata-20261019.json`.
- For SH-03, root normally quit PID 54654 using the targeted application
  shortcut, asserted it gone, and launched in background again. Read-only comm
  inspection confirms 54654 is gone and the unique same development bundle is
  now PID **55394**. Root subsequently observed the same 08:21 question, TC02
  chip / uploaded UUID suffix `72bdffb8`, correct first sentence, full access,
  and idle composer: history/attachment/permission recovery functional PASS.
  The model chip returned to Qwen; no GLM preference restoration PASS is claimed.
- [Model selector/restart audit](../2026-10-19-model-selector-restart-audit/OUTCOME.md)
  independently verifies `glm::glm-5.3` in both successful gate turns' typed
  assistant/model snapshots. The debug startup development-default reseed and
  preference publishing path accounts for the later Qwen selection; this is
  recorded separately from successful history recovery. Root explicitly selected
  GLM 5.3 again before A10. No selector/restart fix was made during this replay.
- The operator now emits a complete live AX tree and derives action indices in
  that same call. Silent/full and diff persistent numeric-index reuse are no
  longer used, avoiding stale native mappings without a foreground fallback.
  The native file picker is now in List view; exact Go To selection still
  requires an observed activation of the selected file before Open, without a
  product change.
- Root reports 185 focused checks green for the reviewed fix set, plus effort
  suites of 3 + 6 + 6 for its eleven source files. Final delivery-contract
  SHA-256 is
  `e05b5d3408dcb38a8d824ad8540dcd64b128978a22be023e9949edadf931f844`.
  These source/build checks are not v7 UI gates or new corpus outcomes. Previous
  broader baseline failures remain disclosed below.

## Historical deployed v5 and qualification

- Final `just build-work` exited 0; log:
  `evidence/final-build-20261019.log`. ACP plus Office built in 50.52s and Work
  in 10.47s. The earlier process 43727 was only a precompile; this later build,
  freeze, and installation is the deployment record.
- `evidence/build-identity-v5.json` records an exact copy match before signing,
  verified installed signature, application stopped for installation, and source
  / installed UUID `4683DA5E-B228-3C8D-B4EE-538A3FB14934`. Signing changes the
  installed whole-file digest; no post-signing whole-file equality is claimed.
  ACP SHA-256 is
  `0860cb7c5dfe1dcec878dd99271342e75f173fc3be246bd33141d42d8cd37ee9`;
  Office SHA-256 is
  `8fc8689a26dfab0898d4262df19e1d00f5ef5efc7d80c230b3cc4fec44961613`.
- V4 PID 36372 normally quit and was confirmed gone. V5 background launch
  returned PID 42418. Its frozen preview is on 4319, owned execution session
  8077; the former owned session 7058 stopped. Shared development port 4318
  was not changed. SH-03 normally quit PID 42418 and confirmed it gone, then
  background launch returned then-current PID 43431 on the same 4319/v5 renderer.
- V5 contains 2,639 files / 76,775,878 bytes and binds ten source hashes.
  `evidence/renderer-v5-manifest.json` SHA-256 is
  `87baad2bac453af0f70140a6d6c62e83c1b05bdce5f7058fb8be933b27781d89`.
  HTTP returned 200 with exact HTML SHA-256
  `beabe830e5890a7a98f107394307aa3d5779668930710b2df45fc50d4bef40dc`.
  The old HomeExtras v2 hash drift was recorded as an existing pre-build
  TaskControls change; no parallel hunk was rolled back.
- Formal v5 epoch: `evidence/original22-v5-full-access-baseline.json`, cutoff
  `2026-10-18T23:52:39.671591+00:00`, 36 pre-existing session IDs excluded,
  required backend mode `bypassPermissions`. Qualification is separate from
  the original 22. Its initial zero-case baseline is preserved; later formal
  progress is recorded below rather than rewriting the baseline.
- SH-01: the actual new homepage selected full access; post-Send and completion
  retained it. UI replied `42`, composer was usable, host ended `done`.
  Session `01a1516f-7e64-74af-bc15-f19b0c7149c6`, anchor/execution
  `01a1516f-7f4c-7383-ac5b-807028cc06b9`, created
  `2026-10-18T23:53:43.781Z`, done `2026-10-18T23:53:51.623Z`.
  Root's whitelisted agent check found `permission_mode=bypassPermissions`:
  functional mode-gate PASS using the existing signed-in profile.
- SH-02: an independent new task submitted the original first-sentence question
  and sole A03 TXT; the expected attachment chip and full-access mode were
  confirmed before/after Send and at completion. The read tool finished
  automatically without a per-call approval card, the UI returned the correct
  first sentence `这周订单模块联调差不多搞定了，目前完成率85%。`, and the composer
  recovered. Session `01a15171-247d-7747-97c1-7c5b025bc02a`, anchor/execution
  `01a15171-25a1-7623-a06a-d53c4a4a0e47`, created
  `2026-10-18T23:55:31.838Z`, host done `2026-10-18T23:55:40.337Z`;
  workspace `/Users/roylin/.a3s/sessions/2026-10-19/01a15171-247d-7747-97c1-7c5b025bc02a`:
  functional full-access attachment PASS. Independent inspection of only this
  exact execution metadata confirms matching IDs, `done`, and exclusion from
  the old baseline; record `evidence/sh02-v5-execution-metadata-20261019.json`.
- SH-03: the normal background restart automatically restored SH-02's TXT,
  correct first sentence, and full-access mode. The operator explicitly opened
  today's 07:53 recent task and confirmed the `42` bubble/time/full mode, then
  the 07:55 attachment task and confirmed its bubble/time, attachment UUID
  suffix `d53af16215ac`, correct first sentence, and full mode. Old same-name
  06:46 / 06:55 rows were not counted. Functional restart persistence PASS.
- Root observed host 42418's ACP children 42784 / 43068 executing the exact
  current `target/debug/a3s-code-acp` path, rather than an old bundle companion.
  Root also reconfirmed the latest companion path after the restart. Formal
  full-access submissions use Qwen3.8Flash, with A03 and A07 now settled.

V5 repair checks remain distinct from corpus outcomes: mode tests have 285
passed and six baseline sidebar failures; the 213 permission-related checks are
green and the 26 member-default checks are included. Output-cap focused sets
6 + 21 + 47 + 6 are green; schema terminal events 4 passed; orphan terminals
across both hosts 24 passed / 2 ignored; reader 40 passed / 2 ignored. Bridge
temporary-log cleanup has 32/32 across three files, with that source included
in v5. Workspace dependency checks passed 6 with Python 3.13. These figures
overlap and are not summed. The 273 baseline TypeScript diagnostics, two old
lint expressions, and six out-of-scope cloud formatting gaps remain disclosed;
no whole-tree green claim is made.

## Formal original-case results, v5 and v7

The two completed v5 outcomes below remain counted after the v7 deployment.
V7 A10 formal attempt 1 was submitted at 08:26 with the exact original prompt,
sole original `TC09_项目总结报告.docx`, trace on, and auto-review off. Root
confirmed GLM 5.3 and full access before/after Send. Session
`01a1518d-2a83-75e7-9789-62909ddc3162`, anchor/execution
`01a1518d-2b76-7116-92ab-f7e3c1c36bed`, created
`2026-10-19T00:26:08.387Z`; independent workspace
`/Users/roylin/.a3s/sessions/2026-10-19/01a1518d-2a83-75e7-9789-62909ddc3162`.
Its backend settled `done` at `2026-10-19T00:30:16.846Z`. Whitelisted current agent fields
show `permission_mode=bypassPermissions`, `model=glm::glm-5.3`, and
`modelName=glm-5.3`; this is agent metadata, not provider-payload inspection.
Root subsequently revisited the exact upload workspace and observed its original
08:26 bubble / sole DOCX, six-page final answer, plan 4/4 done, full access / GLM,
and idle composer with Send disabled. The ignored collector exited 0 and retained
both immutable PPTX versions: 20,152 and 10,185 bytes, plus 2,960 reply bytes
(SHA-256 `717710efd68e9cc7ab8127e18d476678f1065f04fdc6a6705ed86af281bc0ecc`).
All collected sizes and SHA-256 digests independently match the receipt. The
unchanged strict scorer returns **FAIL**: `幻灯片 11 页，超过 10 页`. Both the earlier
five-page deck and final six-page deck are preserved and counted by the existing
scorer; neither was excluded to improve the score. The UI text link displayed
`[blocked]` while one artifact card could open; both traced versions were collected.
Evidence: `evidence/original22-v7-A10-attempt1-{metadata,collection-receipt,strict-score}.json`
and `evidence/original22-v7-A10-attempt1-collector.log`. Independent QA confirms
both packages/native reopening and geometry checks pass with zero findings, and
the required core source facts are covered. It also finds an unsupported assertion:
the source's Q1 target becomes an actual project/reporting period on the covers.
The six-page deck's embedded PNG was actually viewed: the 82/90/95 labels,
0–100 scale and bar heights match. Full-slide visual QA was not performed.
Result: `CORE_FACTS_COMPLETE_WITH_UNSUPPORTED_PERIOD_ASSERTION_AND_EXTRA_DRAFT`,
separate from strict FAIL. Evidence:
`evidence/original22-v7-A10-attempt1-independent-qa.json`, SHA-256
`5ce8ef848206bb97aecf83a607954a2e646d7c902febff0e5d736eabfcdd70a3`.
The earlier safely stopped A10
preparation was not a submission and does not count as a retry.

A35 formal attempt 1: the original prompt and four allowed inputs were submitted
through actual CUA at 08:27, with full access and GLM selected. Session
`01a1518e-a8cf-72a2-b1aa-67920adc4137`, anchor/execution
`01a1518e-a9b4-7349-8838-79dadc031fb1`, created
`2026-10-19T00:27:46.256Z`, backend done `2026-10-19T00:29:33.097Z`. Root observed
the exact four-file user bubble, completed `office_file`, and final reply claiming
four sheets / 30 rows / 225 / 240 / 465, with full access / GLM and idle composer.
The collector exited 0 and captured one real XLSX (5,267 bytes, SHA-256
`a35a9ef62374ba714a7071856f7320a9dcd66f82d9d44c01ea54ee519f3c33b4`) and 2,045 reply
bytes (SHA-256 `6f9bda5b5dd1d600d3d6da04ec04d55097eeb8ce20a1e8055f8706084ca36a84`).
All sizes/digests independently match the receipt. The unchanged strict scorer
returns **FAIL** with three notes: `汇总页缺少 225`, `汇总页缺少 240`, and
`汇总页缺少 465`. Independent artifact QA establishes a separate **semantic
CORRECT** result: all three sources map row-for-row, exactly four sheets / 30
unique IDs are present, and the summary formulas plus cached values are correct
(15/225, 15/240, 30/465). The strict failure is an unchanged scorer parsing
limitation: flattening removes cell separators, creating `咨询15225办理15240合计30465`,
so the numeric-boundary predicates cannot match 225/240/465. Neither the workbook
nor the scoring predicate was changed to obtain a different score; strict FAIL
remains frozen. Visual application QA was not performed. Independent evidence:
`evidence/original22-v7-A35-attempt1-independent-qa.json`, SHA-256
`a844c7dca24751f729783cafa03499102518e644a1f7b6a2b97910ec25a8f3aa`.
Both cases' allowed source hashes match preflight, and every run-file digest is
unchanged before/after QA. Collection and strict-score evidence:
`evidence/original22-v7-A35-attempt1-{metadata,collection-receipt,strict-score}.json`
and `evidence/original22-v7-A35-attempt1-collector.log`.

A03 formal attempt 2 (retry 1, v7): actual CUA submitted the original instruction
and sole TC02 TXT at 08:33, with GLM / full access. The persisted prompt has one
additional trailing ASCII space; this byte difference is recorded rather than
silently called exact. Session `01a15193-7512-70ff-a2da-3ca0fbcfc62f`,
anchor/execution `01a15193-75f2-7477-952b-9b8e62c6c002`, created
`2026-10-19T00:33:00.690Z`, done `2026-10-19T00:33:30.240Z`. Root confirmed the
completed UI answer with `2个`, 85%, 9月12日, 120万元, 15%, and 星河. Typed assistant
model ID/snapshot is `glm::glm-5.3`; current agent mode is `bypassPermissions`.
The collector exited 0 into a separate `run-retry1/TC-A03`, preserving v5
`run/TC-A03` unchanged. Its 510 reply bytes match SHA-256
`698ef95eee9b0ca3904a74854c09679db1b8ef7b80bf8a36939a8be1b01e2443`; no artifact
was requested or collected. Unchanged strict scoring is **PASS**, `要点都在`.
Evidence: `evidence/original22-v7-A03-retry1-{metadata,collection-receipt,strict-score}.json`
and `evidence/original22-v7-A03-retry1-collector.log`. This retry does not add a
distinct case or erase the first-attempt FAIL.

Further v7 submissions and their exact original metadata are recorded below.
A08/A36 have frozen strict results; the other nine original anchors ended in
error and remain uncollected. Later same-session continuations are tracked
separately from these original submissions:

| Case | Session | Anchor / execution | Created UTC | Sole / allowed attachments |
| --- | --- | --- | --- | --- |
| A36 | `01a15194-1333-751f-ade2-a20e0d453069` | `01a15194-140a-7415-a58b-6db880e471c6` | 00:33:41.171 | 口径.txt, 八周使用数据.xlsx |
| A09 | `01a15194-86a1-71b7-8209-8ef7653b1a58` | `01a15194-8787-7682-94ab-3868c4276ea8` | 00:34:10.721 | TC08_月度经营数据.xlsx |
| A02 | `01a15195-8f12-7550-8cc2-18b4914d2167` | `01a15195-9003-77e6-be6d-3f43b307b856` | 00:35:18.418 | A02数据可视化测试样例.xlsx |
| A08 | `01a15196-6923-75f2-b6cc-ff6b5ca9d44f` | `01a15196-69ff-7719-8627-5af55cef3815` | 00:36:14.243 | TC07_区域销售_2024_2025.xlsx |
| A01 | `01a15197-f18b-7282-b785-a7be7b03a629` | `01a15197-f26c-76e0-af2a-a5dc118668bb` | 00:37:54.699 | 方案V2.0.docx, 方案V1.0.docx |
| A04 | `01a15199-1de8-77ab-8962-9f16e51f3c39` | `01a15199-1ec2-72a0-af69-b56a7336be5d` | 00:39:11.592 | TC03_运营改善长报告.pdf |
| A06 | `01a15199-98a2-7355-b4f8-429b04488c1e` | `01a15199-9999-75cb-a0ab-a9dd642e5205` | 00:39:43.010 | TC05_设备采购合同_扫描版.pdf |
| A12 | `01a1519a-6a58-7480-9f5a-3176dfe78fe3` | `01a1519a-6b3b-75dc-bc13-46e1976e3f5e` | 00:40:36.696 | TC14_供应商C方案.pdf, TC14_供应商B方案.docx, TC14_供应商A方案.pdf |
| A23 | `01a1519b-6357-7344-8125-1f4607447baf` | `01a1519b-6442-70fa-8825-50230a878c7a` | 00:41:40.440 | 政策通知.txt |
| A24 | `01a1519c-1bae-74d5-b2fd-2dee02667ddc` | `01a1519c-1c91-76d3-ad59-1428aa56fd86` | 00:42:27.630 | 旧版.txt, 新版.txt |
| A25 | `01a1519c-bb6d-70d1-833a-210a65dbb748` | `01a1519c-bc43-73aa-bb1f-ca52ee72f3c2` | 00:43:08.525 | 预算表说明.txt, 项目说明.txt, 目录.txt, 登记资料.txt, 申请表.txt |

The earlier initial backend metadata was streaming; A25 was already error on its
first bounded inspection. Exact allowed user-message probes
verify the source prompt content with only trailing ASCII spaces (A36/A01/A24: two;
A09/A02/A08/A04/A06/A23: one; A12: three; A25: five), expected filenames, typed assistant model ID/snapshot
`glm::glm-5.3`, and current agent mode `bypassPermissions`. This is local typed
metadata, not an upstream network payload assertion. Each has a separate
`evidence/original22-v7-Axx-attempt1-metadata.json`; no source/answer text was copied.
One root-requested, bounded read of only A36/A09/A02/A01 execution records found
A36 backend `done` at `2026-10-19T00:38:54.519Z`; the other three remained
streaming. This does not replace root's UI terminal confirmation or authorize
collection. No background polling loop was started.
After A24, one further root-requested snapshot covered exactly
A01/A02/A09/A04/A06/A12/A23/A24. It found backend **error** for A01 at
00:42:54.479, A09 at 00:42:17.495, A04 at 00:42:04.856, and A06 at
00:42:33.741 UTC. Their assistant part/tool counts are respectively 11/4,
15/7, 3/0, and 3/0. The other four remain execution `streaming` with assistant
`pending`, each containing only one `data-workbench-proof` part and zero tools.
Last assistant updates for A02/A12/A23/A24 are respectively 00:35:18.659,
00:40:36.923, 00:41:40.674, and 00:42:27.857 UTC. No error bodies, tool arguments,
environment, sensitive configuration, or other session bodies were inspected.
These backend errors are ready for root's UI review but remain uncollected and
unscored. The immutable snapshot is
`evidence/original22-v7-status-snapshot-after-A24.json`; no polling loop was used.
The external ledger counts latest formal case results and separately retains
first-attempt results and every prior diagnostic or retry.

The final one-shot snapshot covers all fifteen known original case sessions,
exporting only exact original/current assistant identities, terminal states,
part counts, and typed model IDs/snapshots. A01/A02/A04/A06/A09/A12/A23/A24/A25
original assistants are error; the six other latest original assistants are
success. Fourteen originals are typed `glm::glm-5.3`; A07 remains historical
v5 `anthropic::qwen3.8-flash` and is excluded from the GLM matrix. A25's current
Agent metadata has already switched to Qwen, while its exact submitted assistant
snapshot remains GLM; the snapshot is used for model attribution. A26 was never
pasted or sent and remains `not_run`.

Root actually observed the original A02/A04 UI errors as HTTP 429 / provider
code 1113, insufficient balance. A04 later acquired Qwen continuation anchor
`01a1519d-c8ac-71b4-9bcf-2431b2228256`; root reports its 08:44 `继续` was not a
root-operated original submission and ended at `max_turn_requests`. A02 also
acquired Qwen anchor `01a1519f-856f-777e-b737-1f38dab22a9b`. Both current turns
are error, but neither represents the original GLM execution or a new independent
Qwen case. Their tool arguments, error bodies, and environment were not read.

The authorized A02 collection was attempted with its exact original GLM anchor.
The existing collector correctly refused with exit 101:
`explicit anchor does not match the current execution`. No `run/TC-A02` was
created and no strict scorer was invoked. A04 was not collected after its
changed anchor was confirmed. Neither was manually reconstructed or allowed
to borrow a later turn's terminal state. Original UI observations and both
earlier metadata snapshots remain preserved. Evidence:
`evidence/original22-v7-A02-attempt1-{collector.log,collection-result.json}` and
`evidence/original22-v7-glm-batch-final-metadata-snapshot.json`.

The GLM-only frozen matrix is `evidence/original22-v7-glm-batch-closed.json`.
Its 14 original GLM submissions have 1 strict PASS, 4 strict FAIL, and 9
unscored execution errors; no missing collection is counted as PASS. Historical
v5 failures remain in the combined ledger. The empty independent Qwen directory
and preparation receipt `evidence/original22-qwen-v8-prepared.json` record zero
formal submissions, no frozen deployment, and no cutoff baseline. The `v8`
directory name is a planned identity, not evidence that a v8 build is deployed.
No same-session continuation is included in that future matrix.

A08 formal attempt 1: root observed the exact 08:36 source-file bubble and final
answer, idle composer / Send disabled, full access and GLM. The collector exited
0, captured 3,164 reply bytes and no artifact, and independently matched SHA-256
`071c19b0f524e83826292e48380a605633abf9566851e20f88263738152b2e48`. The unchanged
strict scorer returns **FAIL** for missing 2024 华东 1200, 2024 华南 1000,
2025 华东 1320, and 2025 华北 1120. Root separately reports semantic defects:
the answer uses the full August 华北 300 as incremental contribution 93.75%,
compares mismatched periods after excluding months, and invents demand/stocking
causes. This manual QA issue is preserved independently of the strict result;
no semantic correctness is claimed. Evidence:
`evidence/original22-v7-A08-attempt1-{collection-receipt,strict-score}.json` and
`evidence/original22-v7-A08-attempt1-collector.log`.

A36 formal attempt 1: root confirmed the exact 08:33 user bubble / two sources,
four final files, plan 5/5 done, full access / GLM, and idle composer. Backend
done is `2026-10-19T00:38:54.519Z`. The collector exited 0 and retained three
PNG files (23,453 / 23,651 / 25,313 bytes), one DOCX (4,303 bytes), and 2,645
reply bytes; all collected byte counts and SHA-256 digests match independently.
The unchanged strict scorer returns **FAIL** only for the missing literal
fifth-week `增长50% / +50%` and sixth-week `下降50% / -50%`. Root reports that the
source explicitly requires two decimal places, and the actual reply uses
`+50.00% / -50.00%`. That source format takes priority; neither reply nor files
were rewritten for a score. Independent QA recalculated all 24 week rows and
19 computable week-over-week rates using decimal arithmetic; every rate matches.
Analysis weeks 4/5 are uncomputable for the correct current/prior missing-value
reasons. The source's two-decimal format is confirmed. All three original PNGs
were actually viewed and passed visual QA, including coordinates, values, and
the missing fourth-week gap; the DOCX passed native validation. The report's
30% fluctuation threshold is explicitly report-defined, and no causal reason
was invented. This is a separate semantic/chart CORRECT result; strict FAIL
remains unchanged. Evidence:
`evidence/original22-v7-A36-attempt1-independent-qa.json`, SHA-256
`d8f04eeb9364ce0419acca689e472d72cca8e3f50da5e5870351c06dfa70f1c2`.
Evidence: `evidence/original22-v7-A36-attempt1-{collection-receipt,strict-score}.json`
and `evidence/original22-v7-A36-attempt1-collector.log`.

A03 formal attempt 1: full original prompt and the sole original TXT were
submitted through the UI with tracing on and auto-review off; full-access mode
remained selected after Send. Session `01a15177-8178-73cf-b53b-94542ebcd8df`,
anchor/execution `01a15177-8252-70dd-8c10-ac71d632c2eb`, created
`2026-10-19T00:02:28.856Z`, host done `2026-10-19T00:02:49.387Z`; independent
workspace `/Users/roylin/.a3s/sessions/2026-10-19/01a15177-8178-73cf-b53b-94542ebcd8df`.
The exact session's agent was whitelisted for a single permission-mode check,
which returned `bypassPermissions`. Root observed the UI terminal, no approval
card, and the expected data. The existing ignored collector then exited 0 and
wrote only the immutable delivery to formal `run/TC-A03` (954 reply bytes,
SHA-256 `d7ab063c6a5ded263ac93136d4981ed261eb1e55b8504ecadb1be43df7c017b4`).
The unchanged strict scorer returns **FAIL** solely for
`缺少两处高风险的数量（2个 / 两个）`; the response's `2项` is an operator-reported
correct unit paraphrase. Manual semantic verification is separate and does not
promote this strict FAIL to PASS. Frozen score/identity receipt:
`evidence/original22-v5-A03-attempt1-strict-score.json`; collector log:
`evidence/original22-v5-A03-attempt1-collector.log`.

A07 formal attempt 1: the exact original prompt and sole original XLSX chip were
confirmed; trace on, auto-review off, UI still full access after Send. Exact
selected-file background AX click enabled Open and uploaded the XLSX; no product
change for a file-extension filter was required. Session
`01a15178-6397-733f-bd21-6475330800f7`, anchor/execution
`01a15178-6493-7558-a156-3814fbd9c9e8`, created
`2026-10-19T00:03:26.743Z`, workspace
`/Users/roylin/.a3s/sessions/2026-10-19/01a15178-6397-733f-bd21-6475330800f7`.
Its initial exact metadata was streaming and its actual agent mode was
`bypassPermissions`; record `evidence/original22-v5-A07-attempt1-metadata.json`.
Root subsequently confirmed UI completion without Pause and backend `done` at
`2026-10-19T00:09:43.321Z`, 376.578 seconds after creation (about 6m16s).
The unchanged collector exited 0, captured four real XLSX files and 2,574 reply
bytes, and reported host `done`, assistant `success`, producer `completed`.
All artifact byte sizes/digests and the reply digest independently match that
receipt. No run file was modified during QA.

All four packages pass ZIP CRC, XML parsing, openpyxl readback and native
`a3s-office validate --json`. Each has `清洗结果`, `原始数据_未修改` and `清洗说明`.
The reply selects `delivery-3-TC06_客户跟进记录_清洗结果_交付版.xlsx` as the delivery.
Its cleaned page, and those in the other three files, have exactly ten unique
records matching the supplied source's six fields after full-row deduplication.
R001/R006 each remain once in the cleaned page; R005 remains distinct. All ten
dates are actual Excel numeric date values with `yyyy-mm-dd` display format,
spanning 2026-01-05 through 2026-01-14. Source serials 46030/46035 correctly
decode to 01-08/01-13. R003/R007 owners and R004 amount are explicitly marked
missing; no value was invented. The nine known amounts total 134700; the sum
and mean formulas have correct cached values 134700 and 14966.67, with no cached
formula-error cells. This is a separate **cleaned-data semantic CORRECT** result,
not an overall task or strict PASS. No visual application review was performed.

The complete immutable delivery still gets **strict FAIL** from the unchanged
scorer SHA-256 `9f1f61f9a089f7a820482bb92ee80ff1ca544fd74b7e178796a58d2e6fa542a7`.
All four books and every sheet are included: R001/R006 and amounts 12000/23000
each occur 12 times; the other IDs and amount 5000 each occur eight times,
producing 13 strict failure notes. No draft was deleted, excluded or renamed to
selectively obtain a score. Each individual book also carries an unrequested
12-row source-copy sheet beside its ten cleaned rows, so the delivery ambiguity
is not only a count of files. The first draft retains a placeholder source note;
the second has date-distribution totals that do not reconcile.

The final workbook and reply still misdescribe the original date distribution.
The actual source's 12 rows contain five ISO text dates, three slash text dates,
two Chinese text dates and two native serial dates; after deduplication the
counts are 3/3/2/2. The purported `原始数据_未修改` copy converts the two serials to
ISO strings and changes R005's `2026/01/09` to `2026-01-09`. Its 8/2/2 and 6/2/2
claims describe that altered copy, not the original. The original file itself
remains unchanged at SHA-256
`2141a61466d714e2d95aa0fc48aa204a3f0203f2635a43d767feb467b525fcdc`;
the uploaded-copy equality is separately reported by root. This provenance and
explanation defect does not invalidate the correctly decoded cleaned dates.

The UI's zero artifact badge versus four trace artifact buttons is an
operator-reported display discrepancy: the collector independently captured
all four files. It was recorded, not repaired during QA. Evidence:
`evidence/original22-v5-A07-attempt1-collector.log`,
`evidence/original22-v5-A07-attempt1-strict-score.json`, and
`evidence/original22-v5-A07-attempt1-independent-qa.json`.

While A07 was running, New Task displayed the homepage heading but its composer
inherited the busy queue/Pause state. The preparation helper stopped safely:
there is no A10 draft or submission, and no A10 attempt is counted. Root will
investigate the new-task busy scope separately.

## Earlier v4 diagnostic attempts

At the earlier checkpoint, the operator reported frozen v4 loaded through port
4319 in Work PID 36372. PID 30925 had normally exited through the application
menu and was confirmed gone before that restart.
The development bundle still has its
temporary renderer-port setting; restore the original Info.plist only after the
test application normally exits during final cleanup. Earlier v2/v3 processes
and launch observations below are history, not the current application identity.

A07's earlier diagnostic attempt was submitted in the old per-call confirmation
mode with the complete original prompt and its `.xlsx` attachment chip confirmed
in the UI both before and after Send:

- Session: `01a1514d-14dc-70fb-985d-45da596e248f`.
- Assistant anchor / execution: `01a1514d-15fd-724a-9e56-447d30687dd7`.
- Created: `2026-10-18T23:16:08.540Z` (2026-10-19 Asia/Shanghai).
- Independent workspace:
  `/Users/roylin/.a3s/sessions/2026-10-19/01a1514d-14dc-70fb-985d-45da596e248f`.
- Original submitted state: streaming. Parsing later succeeded, but the operator
  observed about 8.5 minutes before the first `office_file` call. Core fact seq
  40 strictly rejected its `rows2`/`rows3` schema; the missing ToolEnd left the
  UI waiting for confirmation. The corrected call
  `toolu_30238d22ee30479f8cfe8af0` reached preparation/approval at seq 41.
- The backend then remained `awaiting-approval`. An apparently idle UI recovered
  its approval card after switching SH-01 → A07. Two approvals using fresh AX
  state both displayed `发送您的决定失败，请重试`; no new confirmation fact was
  reported. There were zero files, zero final reply, and no terminal then.
- After the normal restart, the operator observed both `office_file` cards as
  failed, zero work artifacts, and the UI interruption message
  `执行已中断，运行进程已不存在。请重新运行任务。` The trace failure has the same
  text. This is retained diagnostic failure evidence, not a corpus PASS.
- Exact read-only persisted-state verification found a further mismatch:
  assistant status is `error`, both Office calls are `output-error`, the proof
  producer is `failed` with zero artifacts and that trace error, but the exact
  execution summary still says `awaiting-approval`. There are zero assistant
  `type=text` reply characters. The UI message is not a text delivery, and no
  authoritative host terminal is inferred from it.
- The existing collector binary was invoked for only this session/anchor with
  output directed to `evidence/diagnostic-confirmation-run/TC-A07`. It correctly
  refused `the UI turn has no authoritative terminal state` (exit 101; one
  failed collector test). Neither that directory nor formal `run/TC-A07` was
  created. The refusal is not bypassed. Logs and metadata are retained as
  `evidence/diagnostic-confirmation-run-{collector,terminal,collection-result}-20261019.*`.
- The post-turn source SHA-256 still equals the preflight:
  `2141a61466d714e2d95aa0fc48aa204a3f0203f2635a43d767feb467b525fcdc`.
  The operator also reported its uploaded copy unchanged.

At the time of these v4 diagnostics, pending repairs were not deployed: reader UTF-8/typed errors had an
operator-reported 40 passed and 2 ignored checks; schema terminal events had
4 passed; orphan-terminal handling across both hosts had 24 passed and 2
ignored. Explicit output-hard-cap boundary retesting and the mode-handoff
frontend repair were still in progress. Root's `just build-work` process 43727
was a precompile, not the final deployment. The later v5 deployment and its
qualification above supersede that pending state; these historical checks did
not establish a corrected desktop outcome at that time.

A03 full-access request: after the PID 36372 restart, the actual homepage mode
was selected as **完全访问** and displayed that value. The original A03 prompt
and TXT were submitted, but the resulting session changed to **逐次确认** and
still showed that mode at completion. The operator reports correct content in
about 30 seconds. Session
`01a1515a-e24d-7441-84bf-90fccee2da8e`, anchor/execution
`01a1515a-e396-7417-b402-23bab4d58371`, created
`2026-10-18T23:31:13.101Z`, workspace
`/Users/roylin/.a3s/sessions/2026-10-19/01a1515a-e24d-7441-84bf-90fccee2da8e`.
The host ended `done` at `2026-10-18T23:31:26.169Z`. Root's collector exported
670 reply bytes with SHA-256
`954efe2cec547c1d18a9ccf1c2f102a15d4095cb36d78ad7d30e2bfcd363f1f7`
to `evidence/diagnostic-confirmation-run/TC-A03`; its log is
`evidence/diagnostic-A03-mode-collector.log`. Root's strict score is FAIL solely
because `2项` does not match the accepted `2个 / 两个` quantity wording; root's
separate source-based semantic check found the numbers and facts correct.

A03 request 2 retained the same failure independently: first selecting full
access on the old task stored that task's choice by design; a later Send would
be responsible for updating the agent. A new homepage displayed full access,
and its full original prompt and sole A03 TXT were confirmed before Send. The
new session nevertheless used default/per-call confirmation after Send and at
completion. Session `01a1515e-e302-73e0-8a07-b4c23b0c1e50`, anchor/execution
`01a1515e-e434-70c1-963e-a55bf6f27525`, created
`2026-10-18T23:35:35.426Z`, host `done` at `2026-10-18T23:35:49.012Z`;
workspace
`/Users/roylin/.a3s/sessions/2026-10-19/01a1515e-e302-73e0-8a07-b4c23b0c1e50`.
Root collected 571 reply bytes, SHA-256
`f6c4065ded7f7dd36a881d2746081b44adde926543a628654a4a76462148ec14`,
under `evidence/diagnostic-mode-retry-run/TC-A03`, log
`evidence/diagnostic-A03-mode-retry-collector.log`.

Independent checks of only these two collected replies and collector metadata
confirmed their exact bytes, digests, identities, host `done`, assistant
`success`, and zero artifacts. Calling the unchanged `score_dir` / `score_a03`
on request 2 gives **FAIL**, with the sole note
`缺少两处高风险的数量（2个 / 两个）`. The original score and integrity metadata
are retained in `evidence/diagnostic-A03-mode-retry-strict-score-20261019.json`.
This strict lexical result is separate from the full-access mode failure and
from root's semantic adjudication; no score tolerance was changed.

Root inspected only the whitelisted agent record for these sessions:
`agentId=01a14deb-5f83-75b0-8d92-25be3c3589fb`, whose
`configuration.permission_mode` remained `default`. This evidence updater did
not read that configuration or either new session body. The mode-handoff defect
had to be repaired before a valid full-access replay could be claimed. Both requests
are retained diagnostics, not full-access PASS results; valid formal
full-access submissions and original-case passes remained zero at that earlier
diagnostic checkpoint. Formal `run/` had no case delivery then; later v5 results
are recorded separately above.

The external case ledger is
`/tmp/aster-task-corpus-cua-20261018-2245/evidence/original22-rerun-ledger-20261019.json`.
It uses the preflight's exact 22 case IDs and records only new operator-reported
attempts and outcomes. No account configuration or actual session body was read
to prepare this status update.

| Original case | Formal full-access replay state | Final outcome |
| --- | --- | --- |
| TC-A01 | NOT RUN | — |
| TC-A02 | NOT RUN | — |
| TC-A03 | DONE — formal v5 attempt 1 | FAIL strict; semantic facts correct separately |
| TC-A04 | NOT RUN | — |
| TC-A06 | NOT RUN | — |
| TC-A07 | DONE — formal v5 attempt 1 | FAIL strict; cleaned data correct separately, delivery/provenance defects remain |
| TC-A08 | NOT RUN | — |
| TC-A09 | NOT RUN | — |
| TC-A10 | STREAMING — formal v7 attempt 1 | Pending |
| TC-A12 | NOT RUN | — |
| TC-A23 | NOT RUN | — |
| TC-A24 | NOT RUN | — |
| TC-A25 | NOT RUN | — |
| TC-A26 | NOT RUN | — |
| TC-A28 | NOT RUN | — |
| TC-A29 | NOT RUN | — |
| TC-A30 | NOT RUN | — |
| TC-A35 | NOT RUN | — |
| TC-A36 | NOT RUN | — |
| TC-A40 | NOT RUN | — |
| TC-A49 | NOT RUN | — |
| TC-A51 | NOT RUN | — |

## Resumed original-22 replay, 2026-10-19

- Fresh native CUA binding recovered the old process's window. Its actual menu
  Quit action normally exited PID 55876; no force-stop or foreground fallback
  was used.
- Launch on frozen port 4320 exposed a test configuration defect: native
  preferences are not granted to that remote origin. Onboarding's existing
  preference read/write failed; no successful privacy consent change occurred.
  The application was normally exited again. The current run instead uses the
  already declared renderer capability for exact origin
  `http://127.0.0.1:4319`, without changing permissions or trust rules.
- An earlier resumed background launch was PID 82375, with
  `self_activation_suppressed: true`. Native AX confirms its actual main
  document on 4319. The development bundle's temporary `LSEnvironment` now sets
  only `A3S_STUDIO_RENDERER_PORT=4319`; restore the original Info.plist after
  the test application is normally exited at final cleanup.
- At that earlier check, frozen `renderer-v2` matched all 2,638 recorded file hashes and all
  five relevant source hashes. Its independent 4319 preview serves matching
  HTML. The shared 4318 development service was not changed.
- Preflight: original 22 cases, 52 explicitly allowed inputs, zero missing.
  The new submission cutoff is `2026-10-18T22:45:37Z`, with pre-existing session
  IDs recorded before any new prompt. Old corpus tasks remain excluded.
- SH-01 was repeated through the actual fixed frontend: UI prompt
  `只回复数字 42`, reply `42`, usable composer, exact new host execution `done`.
  Session `01a15131-cb93-778f-98b9-ec5eb5b05c54`, assistant anchor/execution
  `01a15131-cc4f-7631-b051-2f1a934124d2`. Its exact workspace is
  `/Users/roylin/.a3s/sessions/2026-10-19/01a15131-cb93-778f-98b9-ec5eb5b05c54`.
  This is functional SH-01 evidence using the existing signed-in profile,
  not an empty-profile login test.
- SH-02 attempt 1: the intended A03 TXT was selected by exact URI and the native
  Open button. The fixed frontend displayed the attachment chip. The question
  `请说出所附文件的第一句。` was then visibly submitted with that attachment;
  the host ended `done`, but the reply repeated `42` and did not answer the
  question. This is a content FAIL, retained in `sh02-before-fix-20261019.json`.
  Native `typeText` lost its
  Chinese text except punctuation; that draft was corrected and independently
  checked before submission using the documented native paste operation.
- The exact persisted user message and Core user fact both contain the complete
  current question and attachment bytes. A separate offline two-turn public
  streaming capture preserves `[user, assistant, user]` and the exact final
  input. This does not prove the live provider payload, exclude live PrePrompt
  hooks, or explain the repeated `42`; no speculative production fix was made.
- SH-02 attempt 2 used an independent new task, as the case permits. Native paste
  supplied the question; native Right/Return selected and opened the exact TXT.
  UI displayed the chip and question before Send. The model read the file and
  returned its actual first sentence. Exact session
  `01a15139-e6cb-739b-89c2-3dd412b87e7b`, anchor/execution
  `01a15139-e78d-707b-9bd2-0d890fcc837d`, host `done`: functional PASS.
- SH-03: normally exited PID 82375 and confirmed it was gone, then background
  launch returned PID 93807 with activation suppressed. From the actual recent
  task list, opened today's SH-01 and SH-02 tasks and observed the saved `42`,
  the saved TXT attachment and the correct first-sentence answer: functional
  persistence PASS. The earlier same-session SH-02 failure is not erased.
- Artifact tracing was visibly enabled. The later v4 A07 submission above is
  retained as a confirmation-mode diagnostic. The user's subsequent full-access
  requirement starts formal counting at zero; no new corpus pass is claimed.

Resumed-run identity, preflight and launch records are under the same external
run root's `evidence/`, with `20261019` names. The historical observations below
are retained as history; later resumed observations supersede their obstruction
and renderer-port state.

## Build and desktop identity

- `just build-work`: passed, including ACP, Office, and the Work host.
- Updated the existing local development application bundle from the new debug
  binary, retaining the old executable under the external run root. No login or
  provider state was copied.
- Source and installed Mach-O UUID:
  `D9428EF1-EE9D-3158-B7F6-D429A54D49B1`. All 23 initialized Mach-O sections
  match. Whole-file hashes differ because the bundle executable was re-signed;
  removing signatures does not produce equal whole-file hashes either, so no
  byte-identical executable claim is made.
- Background quit settled the old process. Background launch returned PID 55876,
  `self_activation_suppressed: true`; the exact live main window is 113580.
- External build identity record:
  `/tmp/aster-task-corpus-cua-20261018-2245/evidence/build-identity.json`.

## CUA qualification and first actual turn

The installed `cua-driver` is 0.23.2. All driver input calls explicitly used
`delivery_mode: background`; no foreground fallback, global pointer movement,
drag, or desktop-scoped input was requested. AX reads normally omit screenshots;
the few grounding screenshots are kept outside scored case directories.

Driver AX clicks could open a new task, but AX setters refused the WebKit editor
with `element_outside_target_window`. Background token input and background pixel
input returned `unverifiable`; fresh screenshots still showed the placeholder.
These attempts are not successful submissions. The pixel implementation was
checked against the exact release commit before use: it posts to the PID and
does not warp the physical cursor. Independent cursor readings changed while
the user was active, and are not claimed as proof of unchanged cursor position.

Native `cua_repl` AX setters displayed the text and enabled Send. Its stale
element handles also caused `-10005` failures. Resetting the REPL, rebinding the
application, and using complete fresh AX states allowed a real submission:

| Check | Observation | Result |
| --- | --- | --- |
| SH-01 reply | UI user message `只回复数字 42`, assistant reply `42`; composer usable again | Functional PASS |
| SH-01 host settlement | New session `01a14f8d-f554-7375-a4ca-2f3eab7726f6`; anchor/execution `01a14f8d-f674-779c-b9c7-f78646caaf6b`; stored status `done` at `2026-10-18T15:07:51.422Z` | PASS |
| SH-02 attachment entry | Initial `+` offered only review/trace; repaired menu now exposes Upload and opens the native picker | Entry repair observed in desktop; full SH-02 still FAIL/pending diagnosis |
| SH-02 selected attachment | Exact A03 TXT URI selected, native Open enabled and pressed; returning composer has no attachment chip | Not passed; no model prompt submitted |
| SH-03 restart persistence | Not run after the actual SH-01 turn yet | NOT RUN |
| Original 22 office cases, 3 prompt cases, 2 terminal cases, error cases | Not submitted in this replay | NOT RUN |

Scope deviation: the existing signed-in development profile is used with new
tasks. Empty-profile login has not been tested; the original plan's fresh-user
login requirement is not claimed as covered. The submission cutoff is frozen at
`2026-10-18T15:05:06Z`; the later new-session identity was read only from host
metadata and matched to the observed UI action. Old completed tasks are excluded.

## Collection and independent source criteria

- [Read-only collector](../2026-10-18-task-corpus-collector/OUTCOME.md): three
  self-checks pass after adding the three explicitly listed prompt-only cases
  (MAIL/COPY/CONFLICT). ERR-01/02 remain separate UI checks. It requires exact
  session/anchor IDs and settled typed host
  state, and validates snapshots through the existing ArtifactStore. This is
  collection infrastructure, not an E2E pass.
- Collection success, content score, review status, and actual E2E outcome must
  remain separate. An error/aborted partial response may pass lexical content
  checks; that does not establish a successful user task. `Traced` establishes
  snapshot provenance/integrity, not Office format or content correctness.
- Four scanned-PDF oracles were frozen independently from the source PDFs, before
  inspecting a new model answer: A04, A06, A12-A, A12-C. All 17 rendered pages
  were checked by the independent agent; root separately inspected A06 pages
  1/2, A12 A/C, and A04 pages 1/9/11/12.
- External facts, source/render/oracle hashes, page provenance, and lexical
  limitations are retained under
  `/tmp/aster-task-corpus-cua-20261018-2245/oracle-evidence/`.
  The four oracle files remain outside the repository and are not attachments.
- No scoring predicate or required task prompt is relaxed. Strict lexical
  outcomes and a source-based semantic adjudication must be reported separately.
- The scorer now accepts `--oracle-root <external directory>` and fixes its
  old positional-argument indexing bug. Root reran its synthetic self-check:
  22 positive, 3 blocked, 5 negative and 4 external-oracle checks passed.
  This is not a corpus result. Details:
  [external oracle evidence](../../execution/2026-10-18-task-corpus-external-oracle/OUTCOME.md).

External run root: `/tmp/aster-task-corpus-cua-20261018-2245/`.
Only delivered reply/file bytes belong in `run/TC-*`; screenshots, tool errors,
timing, identity metadata, and collection logs belong in `evidence/`.

## Upload repair and current desktop obstruction

[Upload entry repair](../2026-10-18-composer-upload-entry/OUTCOME.md) reuses the
existing launcher/picker and restores its runtime on the agentless homepage.
Four relevant hermetic files passed 160 tests; scoped lint/format passed. The
whole renderer typecheck failed with 273 diagnostics; none referred to the six
changed files. No renderer-wide typecheck pass is claimed.

A subsequent controlled test runs the actual Bridge, attachment runtime,
ComposerSurface runtime and Tiptap document. Its successful selection, cancel,
and empty-change cases pass (3/3). Successful controlled input keeps the file
token and enables Send, so reconciliation has not been established as the cause
of the actual picker failure. Native `Open Finder item` was useful for folder
navigation; the second file attempt explicitly used a selected file plus the
enabled Open button. Neither attempt produced a visible chip. No unexpected
file was uploaded or sent to a model.

Two controlled failures justified two further production repairs: a rejected
file selection/upload now reports the existing localized error and clears the
selection lock for retry; the shared editor input adapter skips delayed focus
after its editor is destroyed. Their four focused regression files pass 15/15
with zero unhandled errors. Scoped lint, formatting, diff checks and TypeScript
diagnostics for the four changed source/test files pass. This does not establish
that either finding caused the desktop's missing attachment chip.

The broader five-file run has 163/181 passing and 18 failures in the existing
ComposerSurface mock suite. An exact baseline comparison, temporarily removing
only these two production fixes, reproduces the identical 18 failing test names
in the standalone 166-test Surface suite (148 pass). Both production files were
then restored with byte-identical SHA-256 values. The final combined run after
fixture-listener/RAF cleanup has the same failure set. These wider checks are
not reported as passing; details and logs are retained in the
[picker error recovery evidence](../2026-10-18-composer-picker-error-recovery/OUTCOME.md).

The desktop was loading another agent's shared Rsbuild dev server on 4318.
Its UI changed during the picker attempts. HMR destroying a pending input is a
hypothesis, not an established root cause. A static build completed in 6.38s
under the external `renderer/` directory. It used `--no-env` and blanked the two
explicit renderer secret defines. A dedicated loopback preview serves it on
4320; HTTP returned 200. The 2,638-file digest manifest is
`evidence/renderer-manifest.json`. The shared development server was not stopped.

After the two error/lifecycle repairs, a second static build completed in 5.71s
under `renderer-v2/`, again with `--no-env` and the two explicit renderer secret
defines blanked. The owned preview was replaced with this build on port 4320;
HTTP returned 200 and its main HTML bytes matched the built file. All 2,638
files are recorded in `evidence/renderer-v2-manifest.json` (manifest SHA-256
`f29d01815cb3a80f1434c010754701cf4d84c7fb8da61cfb978f144e021027ec`).
Source hashes, build output and HTTP verification are retained separately.
The still-running old application has not been switched to this renderer.

For the planned switch, a temporary bundle `LSEnvironment` port was added and
the development bundle re-signed/re-registered while the old process was still
running. That sequencing should be avoided. The original Info.plist was then
restored and re-signed; its digest is again
`89b93c5b74a8bc2db7d2f45a893317b90f9b5296b8bcc53d589db16b4f12229d`, and
the executable digest is exactly the original installed
`b21a3448909dc2376bfcf21a62d18619ef1a5c832893c2a44d5e00265eb595a4`.
There is no outstanding test-port environment setting in the installed bundle.

Both CUA channels subsequently failed to bind the old main window: the driver
sees its WindowServer ID but zero AXWindows and refuses background input;
native CUA reports `cgWindowNotFound`. Cooperative quit/menu calls were refused.
The driver's force-stop call was also refused with
`foreign_process_termination_denied` because this runtime cannot prove launch
ownership. No shell kill or foreground fallback was used. A fresh metadata
check found only the completed SH-01 execution since the cutoff. A bounded
symbol-only sample found the main thread idle in the normal AppKit event loop,
not a demonstrated main-thread deadlock. This does not establish that the WebKit
renderer is healthy or explain the AX failure.

The user has been asked to exit AsterDream normally so that the fixed renderer
can be used for continued background testing. This is an outstanding external
step, not a completed restart. Only after the process is confirmed exited
should the isolated port be configured and the background launch retried.

Recovery follow-up at `2026-10-18T15:48:27Z`: PID 55876 remains live; a fresh
native binding again returned `cgWindowNotFound`. Native inventory reports the
matching bundle as running without a window entry. The owned preview process
is still live, and port 4320 returns HTTP 200 with HTML matching `renderer-v2`.
No normal application exit, new launch, attachment or model submission occurred
in this follow-up. The same external step remains pending.

At `2026-10-18T15:49:38Z`, the third consecutive goal turn with this same
external obstruction again confirmed live PID 55876 and a fresh native
`cgWindowNotFound` result. The preceding turn was a verified wait against the
live process and owned preview handle. No user report or observed process exit
has arrived. Cooperative CUA exit is unavailable, force-stop was refused by the
driver's ownership check, and no bypass was used. Independent repair, fixed
renderer, collector and source-criteria preparation are complete for the next
UI step; actual replay cannot proceed without recovery of the native surface.
The goal is therefore blocked rather than complete. After normal exit, confirm
the old process is gone, launch the frozen frontend in background mode and
resume SH-02/03 before the corpus cases.

Root additionally reran `cargo fmt --all -- --check` (pass),
`tests/workspace_dependencies.py` (6 pass), and the fresh compiled collector
tests (2 pass, 1 parameterized collector ignored). These checks do not establish
any new office-case pass.

## Pending

Recover the CUA surface after normal application exit and rerun SH-02/03 with
the frozen repaired renderer. Then replay the file tasks first
(A07/A10/A35/A09/A02/A36), followed by the remaining corpus and negative cases.
No new corpus completion rate is claimed.

Before the first file-generating turn, verify the exact new Work session's
`workspace.path` from `sessions.json` metadata. Do not alter the user's global
workspace setting or assume the default path is empty. Enable artifact tracing
through the real composer menu and match its typed proof's session, anchor and
execution identities before collecting immutable snapshot bytes. Runtime ACL
configuration and account/provider state are not needed for these checks.
