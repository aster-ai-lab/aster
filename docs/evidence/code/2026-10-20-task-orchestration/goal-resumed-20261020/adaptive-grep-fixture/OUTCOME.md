# Actual Work → ACP → Core adaptive grep regression

Status: fixture authored and source frozen; not compiled or executed here. The root agent owns the final paired native host / ACP build and execution.

Only two test files changed:

- `apps/work/src-tauri/src/agent_plane/local_acp_delivery_tests.rs`: two lines declaring the sibling test module. Removing exactly that declaration reproduces the saved baseline SHA-256.
- `apps/work/src-tauri/src/agent_plane/local_acp_search_tests.rs`: one ignored, serial fixture using the existing controlled loopback provider and native Work turn harness.

The original five `program` scenarios and provider implementation retain their baseline hashes. No production code, manifests, dependencies, or user configuration files changed.

## Frozen test contract

Exact owning filter:

```text
agent_plane::turns::local_acp_tests::delivery::search::real_acp_adaptive_grep_executes_without_embeddings_and_reaches_provider
```

Run the newly built Work libtest with the fresh companion selected by the existing `workspace_acp()` helper:

```text
<fresh-work-libtest> --ignored --exact agent_plane::turns::local_acp_tests::delivery::search::real_acp_adaptive_grep_executes_without_embeddings_and_reaches_provider --test-threads=1 --nocapture
```

The fixture temporarily points `A3S_CONFIG` at its own trusted ACL, containing adaptive tool presentation and disabled workspace retrieval. Its synthetic credentials select `openai::gpt-test` at the existing ephemeral loopback HTTP provider. It writes a unique marker in its own temporary workspace and scripts one `search(mode="grep")` call followed by final text. No user provider, real LLM, embedding model, LSP query, or VM is invoked.

Assertions cover the actual provider's search schema (`grep` offered; `semantic`/`hybrid` absent), actual current code-intelligence declaration names (`code_symbols`, `code_navigation`, `code_diagnostics`), the grep match reaching the next provider request, normal successful native settlement, persisted search output and final answer, preserved input, and unchanged companion digest. Code intelligence is checked for registration only; this is not an LSP end-to-end proof.

The test body catches assertion panics before cleanup. Cleanup aborts its owned turn, closes and waits/reaps captured ACP clients, closes any late runtime, then explicitly drops and joins the controlled provider before propagating an assertion or cleanup failure. The host ACL environment guard restores the previous path without reading or copying its contents.

## Checks performed

- Scoped `rustfmt --edition 2024 --check --config skip_children=true` for the two owned test files: exit 0, no output; see `format-check-result.json` and `format-check.log`.
- Scoped `git -C apps/work diff --check`: exit 0, no output; see `diff-check-result.json` and `diff-check.log`.
- `source-final.json` records the final hashes and unchanged protected fixtures; all hashes rechecked after the scoped formatting check.

No Cargo command or test execution was run by this agent. There is no passing runtime result yet. The final build/test must validate the current companion and host together.
