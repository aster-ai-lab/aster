# Exact source-line checks for structured output

## Result

`output_check` now has a bounded `source_lines` action that reads admitted workspace files and returns the exact text at caller-supplied 1-based citations. It distinguishes exact, whitespace-only, mismatched, unreadable, invalid-path, out-of-range and over-budget citations without guessing a repair. The generic structured-output directive asks the model to check every workspace source citation and copy `exactCode` verbatim.

## Root cause and changes

A 200-package static-analysis run had 44 terminal tasks with invalid source evidence. Among their invalid findings, 264 were whitespace-only differences at the correct line across 39 tasks; four tasks also used a path missing the extraction root. The previous structured-output fix removed second-model rewriting, but the primary model could still drop indentation or alter a citation.

- `source_lines` accepts 1–32 explicit file/line/code references, uses the configured workspace path resolver and filesystem, and retains workspace read boundaries and operation timeouts.
- Each result includes `exactCode` only for a readable source line within the 8192-byte output bound. Exact matches alone set `valid=true`; whitespace and other mismatches remain invalid.
- The model-facing tool schema and generic output-schema prompt describe the check. No schema field names are inspected to infer semantics, no path is guessed, no line is moved, and no final answer is rewritten.
- Regression coverage exercises exact and whitespace matches, mismatches, missing and parent-traversal paths, line bounds, citation/output budgets, governed tool-schema projection, and the structured-output directive.

## Verification

Environment: Linux x86_64, Rust 1.97.1, GCC 10, offline Cargo dependencies. The Studio submodule was initialized at the parent gitlink before Cargo commands.

- `cargo test --offline -p aster-acp --lib output_check:: -- --nocapture`: **11 passed, 0 failed** before the final budget test was added.
- `cargo test --offline -p aster-acp --lib source_lines_ -- --nocapture`: **2 passed, 0 failed**, including the final budget test.
- `cargo test --offline -p aster-acp --lib structured_output -- --nocapture`: **6 passed, 0 failed**.
- `cargo test --offline -p aster-acp --lib`: **558 passed, 1 failed, 10 ignored**. `plugin_mcp::tests::cancelling_an_actual_native_mcp_call_retires_its_process_and_descendant_writes` timed out before the native MCP call started; its isolated rerun passed (**1 passed, 0 failed**).
- `cargo clippy --locked --offline -p aster-acp --all-targets --no-deps`: passed with existing warnings and none pointing to the changed source-check code.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `cargo build --locked --offline -p aster -p aster-acp`: passed. The installed development pair reported matching `ASTER_PAIR_BUILD_ID` values; `aster --version` reported 0.4.1 and `aster-acp --version` reported 9.3.1.

Real static-analysis smoke used 10 tasks that had all failed source-evidence validation: **9 passed, 1 failed closed after a denied bash call**. All nine passing tasks used `source_lines`; 94 references produced 92 exact, one whitespace and one mismatch result. All nine original final texts matched their structured outputs.

The expanded run used all 44 tasks from the previous run whose terminal status contained `Finding code does not match source line` or `Finding file does not exist`. Model, task prompt, output schema, grader, timeout and attempt limits were unchanged; the intervention was the new Aster tool and generic directive.

- Result: **42/44 passed**, two failed, zero pending; the previous result for this targeted cohort was 0/44.
- 43 tasks used `source_lines`: 81 calls checked 402 references, producing 329 exact, 48 whitespace and 25 mismatch results.
- All 42 passing original final texts matched their structured outputs.
- Both remaining failures checked the affected citations but did not preserve the returned exact text. One retained a whitespace result; the other altered three previously exact citations after checking them.

Detailed artifacts are under `/storage/a3s/security-lab/runs/malregistry/kimi-evidence-fix-smoke-v2/` and `/storage/a3s/security-lab/runs/malregistry/kimi-evidence-fix-44/`.

## Explicitly not claimed

- This is not a new full-200 pass rate, detection accuracy, recall, or a representative benchmark estimate.
- The targeted 42/44 result does not prove that every improvement was caused by the tool; one passing task did not call it and model outputs vary between runs.
- The tool does not automatically rewrite final answers, validate arbitrary business semantics, or guarantee that a model follows `exactCode`.
- No Windows/macOS runtime, GUI, release packaging, or full workspace CI result is claimed.
- The parallel MCP timeout is reported as environment-sensitive only because its isolated rerun passed; no broader upstream root-cause claim is made.
