# Explicit model output caps remain hard caps

An explicit output-token cap was being increased by reasoning effort. The
Anthropic adapter always applied the 65,536-token default floor for adaptive
or GLM effort. The OpenAI adapter used `max(explicit_cap, effort_floor)`.
Consequently a model's declared `limit.output` and the MCP sampling
generation cap could be exceeded on the actual HTTP request. Applying a
second, larger sampling cap could also widen an existing smaller cap.

The shared effort resolver now gives an explicit cap precedence. Only an
unset cap receives the existing default output-window floor. Anthropic
tracks whether its numeric window came from `with_max_tokens`; OpenAI
already represents the distinction with `Option<usize>`. Sampling caps take
the minimum of the existing effective wire window and the requested bound.
A zero effective
sampling cap returns `None`, so the existing sampling caller refuses before
generation. Zhipu delegates its capped view to the shared OpenAI adapter.

The public builder/trait signatures and all existing provider effort
translations are retained. Legacy Anthropic thinking budgets still require
the existing 1,024-token answer reserve; the cap is never raised to fit the
budget. An adaptive effort parameter does not itself promise a separate
fixed thinking-token quota.

## Scope and review

All `lifted_max_tokens`/`output_token_floor`, provider `with_max_tokens`, and
`with_output_cap` callers were read before the change, including
`LlmConfig` factory assembly, model capability application, the Zhipu
wrapper, and `LlmSamplingHandler`. Existing lower configured caps remain
authoritative when a sampling ceiling is applied. No new dependency,
configuration format, model-name matcher, task classifier, or task-specific
instruction was introduced.

Changed source files are `llm/{anthropic,openai,zhipu,effort,mod}.rs` and the
new sibling `llm/output_cap_tests.rs` under `crates/code/core/src`.

## Validation

Before production code changed:

```sh
cargo test -p a3s-code-core --lib llm::output_cap_tests -- --nocapture
```

The initial regression run exited 101: **2 passed, 2 failed**. The captured
high-effort request replaced the initial zero-boundary diagnostic with
65,536 tokens, and a positive 1,024-token sampling cap was widened to 2,048
on recapping. The zero diagnostic was then corrected: the final positive
wire matrix starts at one token; a separate test requires a zero sampling
cap to return `None` without an HTTP request. No claim is made that
`max_tokens: 0` is a valid vendor request.

After the initial fix, the same Cargo command exited zero: **5 passed,
0 failed, 0 ignored**. Independent read-only review then found that applying
a larger cap could still widen an implicit default window. This separate
positive-boundary regression was added before the follow-up production
change:

```sh
cargo test -p a3s-code-core --lib llm::output_cap_tests::sampling_output_caps_cannot_expand_default_wire_windows -- --nocapture
```

It exited 101: **0 passed, 1 failed**, capturing the actual request changing
from 8,192 to 16,384 tokens. Each adapter now uses its effective-window
helper for both request construction and sampling-cap application.
Default effort/GLM floors remain 65,536, and DeepSeek still omits that floor.

The final complete `llm::output_cap_tests` Cargo run passed: **6 passed,
0 failed, 0 ignored**. Its mocked HTTP boundary records the real serialized request
body; no provider request is sent. The matrix covers:

- Explicit caps 1, 2,048, 8,192, 65,535, 65,536, and 65,537 with high,
  omitted/default, and none effort.
- Anthropic Claude, Anthropic GLM, OpenAI GPT, the Zhipu wrapper, and
  OpenAI-compatible DeepSeek without a default output floor.
- Sampling caps only tightening a lower configured cap and repeated cap.
- Zero effective sampling caps refusing rather than producing a zero-token
  vendor request or becoming an unlimited/default generation.
- Legacy thinking-budget minimum and answer-reserve boundaries.
- Identical existing defaults when no explicit cap is configured.
- Larger caps of 16,384, 65,537, and 131,072 never expanding a known
  effective default wire window or changing legacy thinking mode.

The freshly rebuilt Core test executable reported by that Cargo run was
then reused for the directly affected existing test families:

```sh
target/debug/deps/a3s_code_core-cffccb74a0f2d50b llm::anthropic::tests --quiet --test-threads=1
target/debug/deps/a3s_code_core-cffccb74a0f2d50b llm::openai::tests --quiet --test-threads=1
target/debug/deps/a3s_code_core-cffccb74a0f2d50b mcp::client::sampling_gate_tests --quiet --test-threads=1
```

These test families were re-run after the final production change. Results:
**21 passed**, **47 passed**, and **6 passed**, respectively; none
failed or were ignored. This executable was built from the fixed source,
not a pre-change test artifact.

```sh
rustc --edition 2021 --test crates/code/core/src/llm/effort.rs -o target/tmp/output_effort_tests
target/tmp/output_effort_tests --nocapture
```

The current shared helper and its existing tests also passed independently:
**4 passed, 0 failed**.

Owned-source `rustfmt --edition 2021 --config skip_children=true --check`
and `git diff --check --` passed. `cargo fmt --all -- --check` exited one on
concurrent changes outside this slice: six Cloud plugin files,
`crates/code/acp/src/document_read.rs`, and the Work and Desktop
`agent_plane/execution_status_tests.rs` files. Those files were not edited
to make this slice's format check pass.

The Cargo test builds emitted the existing macOS linker warning about the
large `__eh_frame` section. It did not prevent compilation or the tests.

## Final source identity

The six changed Rust files are frozen after the final six-case green run.
Their individual hashes are in `source-sha256.txt`. Aggregating them in
sorted relative-path order as `path UTF-8`, NUL, complete file bytes, NUL
produces SHA-256:

`9126e4b5a9b1b073f8b8489aa10928c14bf4265c6294444ae25446b46571629e`

This identifies source, not a claimed installed/live executable. A product
build must include this final source after the review fix.

## Read-only efficiency findings retained

Interactive ACP still starts at `high`, with a 16,384-token host thinking
budget and 128 tool rounds. Anthropic adaptive requests omit fixed
`budget_tokens`, so that host number is not a hard thinking quota on this
wire. Without an explicit output cap, the existing 65,536-token effort
window remains unchanged by this fix.

The Work renderer emits `reasoningEffort`, but the production Rust local
turn path does not read or forward it to `session/set_model`. The existing
local-ACP test helper performs that request manually. ACP session creation
uses its stored default effort, and unsupported reset values in
`set_session_model` retain the prior selection. Merely displaying a
renderer default/off label therefore does not prove that the local request
uses that mode.

The existing model-preview UI has an effort slider only for configured
models with more than one projected choice. Published choices are
authoritative, including an empty list. Generic assistant defaults are
`reasoning_effort: default`, `maxTokens: 4096`, and
`enableMaxTokens: false`; those values do not establish a local-ACP wire cap.

The frontend already has closed `ReasoningWireProfile` off/auto/effort/budget
contracts. Core's `ModelConfig` carries only a reasoning boolean and token
limits; it cannot explicitly distinguish budget from adaptive reasoning.
A separate integration should project the trusted endpoint/model contract
across the Rust public boundary, validate the user's existing selection,
apply it to the retained runtime, and make reset/off explicit. Protocol or
model-name inference must not substitute for that capability declaration.

Streaming deliberately has no provider wall-clock deadline while bytes
continue arriving. Its default transport idle timeout is 300 seconds, and
the default run `max_execution_time_ms` is unset. An explicit run deadline
is the existing mechanism for bounding an actively streaming call; tool
round ceilings do not interrupt a single reasoning-only generation.

## Explicitly not claimed

No live provider request was sent, and no account/provider configuration or
saved ACL was read. No live ACP or desktop executable was built, replaced,
or restarted.
No default effort or model capability policy changed. No completed task
corpus rerun, faster office-task latency, fixed model reasoning behavior,
or complete workspace build is claimed. Very small explicit output caps
can still truncate an answer; they remain ceilings rather than permission
to silently spend more tokens.
