# Remaining surface hermetic verification — 2026-11-08

After deleting remote workspace / advanced chicken-rib surfaces from `aster-core`.

## Commands and counts

### Compile

- `cargo test -p aster-core --lib --no-run` → pass
- `cargo test -p aster-core --features scientific --lib --no-run` → pass
- `cargo build -p aster -p aster-acp` → pass

### Focused remaining-surface lib suites (default features)

| Filter | Result |
| --- | --- |
| `sdk_capabilities` | 5 passed |
| `workspace::tests::` | 15 passed |
| `workspace::error::` | 8 passed |
| `workspace::conformance` | 3 passed |
| `workspace::retrieval::` | 188 passed |
| `workspace::manifest::` | 48 passed |
| `agent_protocol::` | 23 passed |
| `orchestration::` | 42 passed, 1 ignored |
| `security::tests` | 5 passed |
| `tools::builtin::bash` | 70 passed, 1 ignored |
| `harness_loop::` | 88 passed |
| `porcelain::tests::` | 18 passed |
| `run_control::` | 23 passed |
| `store::tests::` | 76 passed |
| `mcp::` | 216 passed |
| `skills::` | 68 passed |
| `planning::` | 57 passed |
| `permissions::` | 123 passed |
| `hooks::` | 78 passed |
| `config::` | 120 passed |
| `content_digest::` | 4 passed |
| `session_checkpoint::` | 20 passed |
| `goal_control::` | 58 passed |
| `moli_runtime` (`--features headless-search`) | 10 passed, 1 ignored |

### Broader suites with residual failures (same dirty tree)

- Full `cargo test -p aster-core --lib`: **5802 passed / 35 failed** (from `/tmp/aster-core-lib-default.log`)
- `agent_api::tests::`: 316 passed / 19 failed
- `fact_control::`: 258 passed / 3 failed
- `memory::` / `durable_memory::`: 1 golden-identity failure each
- `tools::builtin::` without `headless-search`: 1 fail (`headless_engine_request_does_not_spawn_moli`); under `headless-search` the moli filter passes

### ACP

- `cargo test -p aster-acp --test tui_913_hermetic` after building `aster`/`aster-acp`: **9 passed / 0 failed**

## Failure classes observed

1. Fixed golden digests / identity pins: `work_policy`, `durable_memory::context`, `capability` UI/Knowledge bindings, `tools::result_transform`, package binding suites.
2. `fact_control` nudge/cancel/duplicate guard (3).
3. LLM transport/effort pins: `codex_login` / `openai` split-CR, `effort` native model table.
4. Default-features headless web_search test without `headless-search`.

These digests/pins are outside the deleted S3/RemoteGit/apofasi/evaluation/state_graph/telemetry modules; treat as dirty-tree residue until separately repaired.

## Explicitly not claimed

- Live provider / Layer C / `deep_e2e_live` receipts
- Full `cargo test -p aster-core --tests` matrix (one ignored real-LLM attachment fixture was API-updated only)
- Studio / desktop GUI e2e
- Cloud `apps/cloud` aster-core mirror compile
- That the 35 full-lib failures were introduced solely by the chicken-rib deletion
