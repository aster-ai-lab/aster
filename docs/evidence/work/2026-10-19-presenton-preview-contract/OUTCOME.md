# Work Presenton preview contract

Date: 2026-10-19

## Result

Work now describes `generate_deck` as parameter validation, which does not
start background generation or save a presentation. Explicit `.pptx` delivery
is directed immediately to the existing `office_file` save and readback route,
without waiting for a preview. Empty `read_deck` remains an MCP error, with an
explicit no-readable-snapshot reason and an instruction to stop repeated
polling and use native Office for file delivery.

Nonempty snapshot reads, tool names, input bounds, closed language values,
and validation remain available. No AIX protocol admission was relaxed.
No native Office API, Core behavior, Desktop source, deployment, GUI action,
formal task prompt, source document, or generated task artifact was changed.

## Scope and source review

The Work production references to the tool names are the MCP tool definitions
and handlers and the homepage task prompt. `homeComposerMode.tsx` appends the
homepage task prompt to the emitted live task text; its existing send test now
checks that the native-file-first guidance actually reaches that text.

Desktop has `PresentationGenerateBridge`, which observes successful tool parts
and stages a preview seed. Work has no corresponding observer. Its current
`AixViewBridge` accepts only typed `aix.view.store`; it does not run the legacy
Presenton `aix.agent.invoke` generation protocol. The ACK therefore cannot
truthfully claim to have started generation. This change corrects the contract
without reviving that protocol or rebuilding the preview integration.

The exact incremental diff relative to the pre-edit dirty-tree snapshot is
[change.patch](change.patch): four code/test files, **34 insertions and 18
deletions**. It excludes all preexisting shared-tree changes.

Baseline repository HEAD: `e4344fc07d51503f3e65eae8b1d29e18c2bcfdaf`.
Incremental patch SHA-256:
`1ad10e7386a3ab9b0c377569ddfd03ca26722d38799f366fb9c2ebc0e3f9e41b`.

| Final source | SHA-256 |
| --- | --- |
| `apps/work/src-tauri/src/agent_plane/presenton_mcp.rs` | `4eed436204510aa265e57cbfdaa06040597e2da5bf839869affa2aedfcfbac3d` |
| `apps/work/src-tauri/src/agent_plane/presenton_mcp_tests.rs` | `fc3f77c6c07346572ac3b33d04d368911bb6880bc1dda677f2d39153709e6225` |
| `apps/work/src/renderer/services/plugins/presentonHomepage.ts` | `8b9ac12e6ec84e41dc12cc6122f0e8a0cab1c3ff4f7b57a47ab4005abd14e8d2` |
| `apps/work/src/renderer/pages/agents/__tests__/presentonHomepage.test.tsx` | `322bf4ceb6fdfb9c9b446c3da4bdc0d738007288d676621751665dbf0d8d1565` |

## Validation

From the repository root:

```text
cargo test --locked --offline -p aster-dream --lib presenton_mcp
```

A single Cargo invocation waited for the shared target lock, freshly compiled
`aster-dream`, and passed **5 tests, 0 failed, 772 filtered out**. Cargo reported
2m 14s to finish the test profile. Test executable:
`target/debug/deps/a3s_desktop_lib-d46b292e4f47e130`.

Coverage includes the validation-only ACK, native-file-first tool descriptions,
empty snapshot refusal, nonempty snapshot readback, unchanged four tool names,
90 canonical languages, rejected invalid input, and the existing snapshot size
bound. Rust red was not claimed: the final implementation was updated while
this single invocation was waiting for the shared target lock.

From `apps/work`:

```text
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/agents/__tests__/presentonHomepage.test.tsx
```

Before the production prompt change: **2 passed, 1 failed**, at the old prompt
that did not direct native Office first. After the change: **3 passed, 0 failed**,
one file, 2.16s.

Formatting and whitespace:

```text
rustfmt --edition 2024 --check apps/work/src-tauri/src/agent_plane/presenton_mcp.rs apps/work/src-tauri/src/agent_plane/presenton_mcp_tests.rs
/opt/homebrew/bin/node node_modules/oxfmt/bin/oxfmt --check src/renderer/services/plugins/presentonHomepage.ts src/renderer/pages/agents/__tests__/presentonHomepage.test.tsx
git diff --check -- apps/work/src-tauri/src/agent_plane/presenton_mcp.rs apps/work/src-tauri/src/agent_plane/presenton_mcp_tests.rs apps/work/src/renderer/services/plugins/presentonHomepage.ts apps/work/src/renderer/pages/agents/__tests__/presentonHomepage.test.tsx
cargo fmt --all -- --check
```

The scoped Rust, frontend, and whitespace checks passed. The whole-workspace
format check exited 1 on unrelated existing ACP test formatting differences,
including `engineering_tests.rs`, `agent_tests.rs`, and
`run_observation_agent_tests.rs`; those files were left unchanged by this work.

## Explicitly not claimed

- No deployment or live model replay was performed. This does not establish a
  new original-task pass count or prove that a model stops polling.
- Preview generation handoff, view survival during task switches, global
  snapshot ownership/isolation, and preview edit application are not repaired.
- This does not independently validate any formal task's generated PPTX or
  change the existing Office tool's authority, file validation, or output limits.
- The whole workspace is not claimed formatted, built, or fully tested.
