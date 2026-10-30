# Implementation Plan: Rust SDK and explicit capability activation

**Branch**: Existing | **Spec**: [spec.md](spec.md) | **Input**: User-authorized review fixes
**Gate context**: Root product workspace and private Studio submodule.

## Summary

Clean active retired-SDK documentation, narrow ACP Office registration through existing host metadata, move browser search to an explicit build feature, defer Moli preparation through the existing shared future, and correct embedding guidance. Preserve existing execution authority and resource limits.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Invalid metadata refuses; tools still require workspace authority; unavailable search stays typed. |
| II. Evidence retained | PASS | Baseline and OUTCOME under the dated evidence directory; historical records retained. |
| III. Hermetic tests | PASS | Manual, metadata, session, host and lazy-runtime cases accompany behavior. |
| IV. Closed sets | PASS | Existing feature names and explicit boolean; no new free-form policy. |
| V. Workspaces / APIs | PASS | Existing members and public Rust APIs preserved. |
| VI. Local schema | PASS | Host metadata remains owned by ACP/Studio; no persistent schema change. |
| VII. Ratchets | PASS | No allowlist relaxation. |
| VIII. Transports | PASS | No credential or provider fields added to shared envelopes. |
| IX. ACL | PASS | No new product configuration format. |
| XIII. Repository | PASS | Private host files stay in its submodule; no new checkout. |
| XIV–XV. Names / meaning | PASS | Host explicitly declares Office; no prompt or filename classifier. |
| XVI–XVII. Product UI | PASS | No picker or animation change. |

## Project Structure

| Layer | Existing paths / patterns | Change |
| --- | --- | --- |
| SDK documentation | `crates/code/manual`, `core/src/sdk_capabilities.rs`, `core/tests/manual_index.rs` | Rust/ACP guidance; remove retired publishing guide; contract guard. |
| ACP Office selection | `acp/src/host_context.rs`, `agent/session_open.rs`, sibling tests | Strict optional host boolean; retain output checks and existing permission gates. |
| Studio caller | `apps/studio/src-tauri/src/agent_plane/turns.rs` and owning tests | Declare host-owned Office support in existing metadata. |
| Browser build | `acp/Cargo.toml`, root justfile and Studio build/development/package scripts | Explicit feature; maintain desktop capability. |
| Browser lifecycle | `core/src/tools/builtin/web_search.rs`, owning tests | Lazy `PendingHeadlessRuntime`; remove eager spawned preparation. |
| Embedding contract | `acp/src/agent/local_embedding.rs` and operation guides | Reflect registered-factory / embedded-light selection. |

## Research

- Reuse `sourceReview`'s strict boolean parsing and session-level host binding.
- Reuse `PendingHeadlessRuntime` shared futures and existing Moli resolver/deadlines; no new provisioner or cache.
- Preserve Core's existing local/scientific/server feature separation.
- Cargo features are additive: [official Cargo feature reference](https://doc.rust-lang.org/cargo/reference/features.html). Paired builds must be checked for unification.
- On-demand activation follows the existing explicit lifecycle pattern, consistent with [VS Code activation events](https://code.visualstudio.com/api/references/activation-events); no keyword-based activation is added.

## Verification Plan

1. Capture dirty baseline; inventory SDK remnants and all callers before editing.
2. Run manual/index/installer guards; inspect links and Rust examples.
3. Run ACP metadata/session setup and Studio metadata tests, including refusal and source-review cases.
4. Verify default and headless-enabled dependency graphs; run web-search tests for deferred preparation and existing refusal/cancellation behavior.
5. Run formatter, dependency guard, focused connected checks, paired terminal/ACP build and relevant Studio build. Record source drift and existing failures separately.
6. Record OUTCOME; no stage, commit or push.
