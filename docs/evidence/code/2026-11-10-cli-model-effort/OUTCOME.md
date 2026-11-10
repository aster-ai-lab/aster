# Preserve configured effort during CLI model initialization

A model-only CLI override no longer resets an authoritative session's effort
when ACP has already selected that model. The deferred request retains its
intent until the final session catalog is available. Explicit CLI effort,
explicit default selection, manual model picks, and a different model restored
from disk still follow the existing switch path. No provider name or effort
vocabulary is added to the initialization logic.

## Reproduction

The native preflight used a freshly built `aster` / `aster-acp` pair, the selected
`deepseek/deepseek-flash`, and a private byte-identical copy of the supplied ACL.
The source ACL had a different default model and an explicit `max` effort.

Launch command, from the owned test workspace with isolated Aster state:

```sh
/Users/roylin/code/aster/target/debug/aster --model deepseek/deepseek-flash --no-plan --fullscreen
```

[Startup](preflight-01-startup.png) displayed `max`. Opening
[`/effort`](preflight-02-effort-before-task.png) created the session, sent a
redundant model switch, displayed `Default (high)`, and changed the private ACL
to default effort before any model task. The original ACL was unchanged.
[Profile fingerprints](preflight-profile-mutation.json) retain that observation.

## Verification

All Cargo commands ran from the repository root with Rust 1.97.1,
`CARGO_TARGET_DIR=/Users/roylin/code/aster/target`, and `CARGO_INCREMENTAL=0`.

```sh
cargo test --locked -p aster-pager --lib app::dispatch::tests -- --test-threads=1
cargo clippy --locked -p aster-pager --all-targets --no-deps
cargo fmt --all -- --check
```

- Final dispatch tests: **1780 passed, 0 failed, 0 ignored**. Four new cases
  cover preserving current effort, switching a different restored model,
  explicit default/remapped effort, and retaining manual default reset.
- All-target Clippy passed with existing warnings. Its only diagnostic in a
  touched file concerns the unchanged switch-completion handler.
- The source pair was rebuilt and both BUILD_ID values agreed. The real
  DeepSeek metadata regression also passed in a headless PTY: `max` remained
  active, `Default (high)` stayed separate, the private ACL digest was unchanged,
  and the owned process exited zero. This submitted no inference task and is
  not a native desktop pass. See [headless-effort.json](headless-effort.json).
- The shipped-pair suite passed 22 tests with one ignored live test and one
  outdated theme assertion. The product correctly persisted `aster-day`; the
  fixture still expected `grokday`. After updating that fixture, an initial
  rerun correctly refused a mixed pair because Cargo had rebuilt ACP alone.
  Rebuilding both binaries and rerunning the affected tour passed: **1 passed,
  0 failed**. Thus 23 distinct eligible tests passed across the two valid runs.
- Changed files were formatted. Workspace format checking still fails in
  unrelated files listed in [verification.json](verification.json).
- The first dispatch run had 1779 passes and one failure. That failure also
  failed alone. The old fixture expected a default selection to preserve a
  stale explicit catalog effort, contradicting the handler committed in
  `b542eaffa3`. The handler is unchanged by this fix. The corrected fixture
  checks explicit `None`, the separate advertised `high` default, and persisted
  `None`. [The unchanged-handler fingerprint](model-effort-fixture.json) and
  verification metadata retain the failure and correction.

## Explicitly not claimed

The desktop was locked during the checks above and was then unlocked by the
user. Post-fix agent-desktop verification is still pending; no complete 508-case
campaign is claimed here. The candidate also contains pre-existing, uncommitted
ACP cancellation work. The user authorized publishing the separately verified
normal-sandbox cancellation patch; its integration and native verification are
a separate delivery. This evidence is not a clean release qualification,
a packaged-build pass, or native Linux/Windows acceptance.
