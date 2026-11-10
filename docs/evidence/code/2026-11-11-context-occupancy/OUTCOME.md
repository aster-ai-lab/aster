# Reported context occupancy

The native DeepSeek session displayed cumulative session usage as current
context occupancy. The retained provider input was 18515 tokens, but the modal
and header showed 116k, matching the cumulative total of 116366 tokens.
Unreported category counts were also rendered
as measured zeroes, the entire remainder was labelled reasoning/overhead, and
an unreported compaction threshold appeared as 85%. See
[native-before.json](native-before.json) and [the actual window](native-before.png).

ACP now reads the latest retained model-input report from the current session,
with runs visited in admission order from newest to oldest. Cumulative accounting
is unchanged. The session lookup releases its mutex before awaiting Core reads.

Usage availability and detailed-breakdown availability travel separately. A
reported zero is a measurement; an absent report is not. Existing full snapshots
retain their measurements. The fullscreen header, minimal prompt, running status,
goal projections, dashboard and session-info view use the same availability
predicate. An unknown numerator preserves a known window as `— / limit`.

The context modal shows reported input and the known remaining window without
inventing category measurements, tool/compaction counts or a compaction policy.
It preserves a reported turn count. An absent input report has no free-budget
percentage or progress bar. No model-specific formatting, estimates substituted
for provider reports, new dependency or configuration option was added.

## Verification

Commands ran at the Aster root with Rust 1.97.1,
`CARGO_TARGET_DIR=/Users/roylin/code/aster/target` and `CARGO_INCREMENTAL=0`.
[checks.json](checks.json) retains exact commands, counts and log digests.

- Context wire: **3 passed**
- Context modal: **38 passed**
- Context bar: **17 passed**
- Session-info missing usage: **1 passed**
- Minimal prompt information: **5 passed**
- ACP host methods, including the latest-report and reported-zero regression:
  **14 passed**
- The separate controlled provider-discovery fixture: **1 passed**. Its delayed
  response updates the advertised controls and actual runtime configuration;
  this is not a real upstream-model run
- Two initial host-method filters selected zero tests. They are retained and
  are not passes. The current-source test binary then executed all 14 tests
  under the actual module name
- Touched-file formatting and `git diff --check` passed. Workspace formatting
  reports exactly the same 18 unrelated paths as the preserved baseline
- All-target Clippy passed for ACP, Shell, Pager and Pager Minimal. Owned-file
  warnings are existing enum size, unused `remember_denies`, a redundant test
  conversion, and the pre-existing nested model-metadata conditional. These
  unrelated sites are unchanged; a separate cleanup would address them
- Both binaries built together and independently match current source fingerprint
  `4f545da722ae77266e8f9d3156627f34eb33817e`; see
  [source-reconciliation.json](source-reconciliation.json). This is a dirty-tree
  test candidate containing the parallel ACP work, not a release candidate
- The previously failing complete TUI tour passed alone: **1 passed, 0 failed**
  in 95.95 seconds. The original entire-screen assertions remain unchanged.
  The tour also exercised resume, local commands, YOLO and both render modes
- The complete shipped CLI/ACP target then passed on the final pair: **23 passed,
  0 failed, 1 ignored** in 184.19 seconds. The ignored real-model goal case was
  not run and is not counted as passing
- Additional unchanged model contracts: **8 discovery tests** and **5 reasoning
  wire tests passed**. Together with the selected-catalog fixture above, these
  are 14 controlled model-contract tests, not upstream-provider acceptance

## Reproductions and checker corrections

[reproductions.json](reproductions.json) retains the original failures and their
log digests. The latest-report regression first returned cumulative 408 where
the latest input was 131. Its original fixture assumed only two provider calls;
the canonical fixture made four. The corrected fixture preserves the exact
latest-report assertion and only requires the independent cumulative figure to
be larger. No production prompt or provider fixture response was changed.

The empty-session CLI/ACP tour later caught a missed fullscreen-header caller.
That failure reproduced alone. The entire-screen checks for fabricated zero
usage and 100% free remain intact. A zero notification is tested separately as
an explicitly reported measurement, while absence uses an explicit availability
flag. A temporary duplicate test-module name failed compilation and is not
counted as an executed test.

## Native replay

The original macOS Terminal observation used `deepseek/deepseek-flash` from the
supplied ACL and pair `2a0d00d9ae724ea12f83b7dc1fe523aff1796342`.
The source ACL hash remains unchanged. The fixed native scenario and its
unchanged prompt and deadlines are retained in [native-plan.json](native-plan.json).

The final native context and timeout-owner replays remain pending while macOS
reports a locked console. The primary-display screenshot contains only wallpaper,
and agent-desktop finds no addressable Terminal window. The lock flags and
observation are retained in [native-availability.json](native-availability.json).
The application accessibility API still names Terminal as frontmost; that alone
is not proof of an available desktop. No keystroke is sent to a locked desktop.
Native permission state and the previous agent-desktop delivery limits remain separate
from product results.

## Explicitly not claimed

This is focused logic and controlled-provider validation with retained native
reproductions. It is not native acceptance of this fix, a clean release candidate,
real Windows/Linux runtime acceptance, every provider or every TUI catalogue
case. The parallel ACP cancellation patch remains uncommitted and is excluded
from the owned change. No release was published.
