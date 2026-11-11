# Test-debt cleanup after the Aster brand migration

Date: 2026-11-11 (Asia/Shanghai). Repository: `/Users/roylin/code/aster`.
Starting HEAD `49e0fd209`. Two local commits: `b98ed674d` (insta baseline
rename), `1a2981490` (stale test expectations). No product logic was changed.

## What was wrong

A previous answer listed 25 `aster-pager` lib failures and flagged the
status-line `+2` column shift as the most likely product defect. Re-reading the
code reversed that suspicion: the shift is a **brand-migration measurement
change**, not a rendering regression. The full 25 were mostly test debt.

| Group | Count | Root cause |
| --- | --- | --- |
| insta `+new results` | 11 | The crate was renamed `aster_code_pager` → `aster_pager`, but the 11 tracked `.snap` baselines kept the old `aster_code_pager__` prefix. insta looks for `aster_pager__` and finds nothing. |
| Stale brand expectations | 8 | `a3s` → `Aster` and `yolo` → `YOLO` lengthened visible text and changed labels; expectations and comments were not updated. |
| Intentional effort menu | 2 | `reset_item()` deliberately adds a "Use the model default" row to the shared `/model` / `/effort` dropdown; tests still expected five levels only. |
| Self-contradictory assertions | 2 | Brand-cleanup commit `0935a85a6` added `!text.contains("aster")` inside tests that also require `Run \`aster login\``. |

## Changes, and why they are not weakened assertions

- **11 snapshot baselines**: `git mv` to the new prefix only. A sha256 check
  against `HEAD` proved **11/11 byte-identical**, and a line-by-line comparison
  of every old baseline with the current output proved **11 MATCH / 0 DIFFER**.
  This repairs a migration omission; it does not accept new output.
- **Brand measurement updates** (product output is correct and matches the
  requirement "YOLO / no grok visible"):
  - `[a3s] ` 6 columns → `[Aster] ` 8 columns; `a3s` 3 columns → `aster`
    5 columns. Updated `views/status_line/sanitize_tests.rs` (four cases),
    `views/status_line/tests.rs`, and their comments.
  - `scrollback/text_selection.rs` URL head 33 → 40 (the URL itself grew).
  - `views/announcements.rs` `[Get Aster Code]` hit rect 14 → 16.
  - `views/dashboard/peek.rs` `yolo` → `YOLO` in four assertions.
  - `doctor_cmd/tests.rs` theme fixture `groknight, grokday` →
    `aster-night, aster-day`.
- **Effort menu**: expectations now assert the exact six-row list
  `["Default","low","medium","high","xhigh","max"]` and selected index 3
  (high). The assertions remain exact, just aligned with the intentional row.
- **Two contradictory assertions**: removed, because they could never hold;
  the brand-cleanup commit introduced them with the defect. Each test keeps
  3–5 meaningful assertions (auth method, env marker, account copy, legacy
  domain). Renamed `..._suggests_grok_login` → `..._suggests_aster_login`.

## Verification (commands and counts)

| Command | Result |
| --- | --- |
| `cargo test --locked -p aster-pager --lib -- --test-threads=1` (baseline) | 10347 passed; 25 failed; 4 ignored |
| same, after `b98ed674d` | 10358 passed; 14 failed; 4 ignored; `new failures: []`, 11 fixed |
| same, after the brand batch | 10368 passed; 4 failed; 4 ignored; `new failures: []`, 10 more fixed |
| same, after the YOLO case alignment | **10369 passed; 3 failed; 4 ignored; `new failures: []`** |
| `cargo clippy --locked -p aster-pager --all-targets --no-deps` | exit 0 |
| `cargo fmt --all -- --check` | exit 1, 18 pre-existing files / 33 hunks; none of the changed files |
| `cargo test --locked -p aster-pager --lib input_recovery -- --test-threads=1` | 16 passed |
| `python3 tests/release_governance.py` | 10 tests OK |
| `cargo build --locked -p aster -p aster-acp --bin aster --bin aster-acp` | exit 0 |

Rebuilt pair: `aster-cli-pair-v1:58adbc450c5f3d094591dcbfe417c788cb0cdbd3:aarch64-apple-darwin`;
`source_pair.py` recomputed `58adbc450c…` over 13,465 files with
`source_matches_pair: true`. Rust 1.97.1, macOS aarch64, shared `target`,
`CARGO_INCREMENTAL=0`. Every full-suite run had an explicit failure-set diff;
no failure disappeared through relaxation. The `.snap.new` files produced by
these runs were removed so they could not pollute the source hash.

## Three remaining failures, with exact follow-ups

1. **`short_terminal_leaves_the_row_four_of_its_five_rows`** — product layout
   decision, not a stale expectation. At 16 rows the borderless prompt draws
   upper/lower half-block vpad rows (`views/prompt_widget/mod.rs:3172`, an
   intentional visual design), so the status script keeps only `row-1..row-3`;
   the test asserts `row-4` survives. **Follow-up**: decide whether short
   terminals should suppress vpad. Changing the assertion to 3 rows would
   weaken the short-terminal usability contract, so it was left failing.
2. **`shell_aliases_expand_to_exact_argv_and_bypass_is_explicit`** — test
   harness / environment defect with a network side effect. The fixture runs
   `bash -ic "source rc; source rc; eval 'ssh -p 2222 host'"`, but `-c`
   parses the whole command before the alias exists, so the alias never
   expands: the real `ssh` runs, connects to the sandbox proxy
   `198.18.4.254:2222` and exits 255. Reproduced manually, including with
   `shopt -s expand_aliases`. **Follow-up**: redesign the fixture (e.g.
   `bash --rcfile` with a separate command line) so the capture assertion
   actually runs; do not relax `status.success()`.
3. **`rendered_user_and_streaming_markdown_commands_keep_their_text_and_layout`**
   — suspected product defect, not yet localized. At `width=18` the magenta
   highlighted text is only `/acme:review`; `/model` is not highlighted even
   though `slash/highlight.rs` explicitly rejoins soft wraps to keep one
   command identity. **Follow-up**: inspect `ResolvedSelectionModel`
   ranges/lines at width 18 and find why the first token is dropped.

## Release gate — still BLOCKED

Worktree still has the parallel ACP cancellation patch (five files), its
untracked `docs/evidence/code/2026-11-10-acp-cancel-terminal/`, a dirty
`apps/studio` submodule and a modified Stage-3 OUTCOME from the parallel
task: 8 non-empty `git status --porcelain` entries. `publish-aster-cli.sh`
refuses a non-empty porcelain; two local commits are unpushed; tags
`aster-acp-v9.2.2` and `aster-cli-v0.3.4` do not exist; `release-code.yml`
also needs a successful `qualify-code.yml` run. No tag, push, workflow
dispatch, `cargo publish` or channel update was performed. Parallel work was
preserved and never staged into these commits.

## Explicitly not claimed

- A fix for the three remaining lib failures, the 18 pre-existing
  `cargo fmt` files, or the parallel ACP cancellation patch.
- Any change to product rendering, link-column computation, prompt layout or
  slash highlighting.
- The full 508-case TUI campaign (452 NOT_RUN / 56 PARTIAL), Windows/Linux
  runtime parity, or a fully green package/workspace suite.
- Native re-verification on this pair; the native recovery and gesture-matrix
  evidence remains valid for its own recorded BUILD_ID.
- Any release, push, tag, or acceptance of parallel work.
