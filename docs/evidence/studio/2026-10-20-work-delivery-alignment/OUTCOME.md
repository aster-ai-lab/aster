# Studio delivery alignment

Date: 2026-10-20. Studio keeps its preview canvas and its source-review host. Two Work delivery facts now apply to ordinary Studio ACP sessions.

## Result

`generate_deck` still only validates parameters. Its description and success reply now say the host opens the right-hand preview from that input, and that the reply is not a PPTX file. A member who asks for `.pptx` is sent to `office_file` to save and read the file back. An empty `read_deck` tells the model to stop polling. Outline and slide edits still say not to write another file. The install check and the four tool names are unchanged. The slides task prompt in `presentonHomepage.ts` uses the same split. `presentationGenerate.tsx` still seeds the canvas from the tool input.

`acp_host` now puts `completionAttestation: disk_content` on every returned object, including a request with no host text and no assets. ACP installs `DiskContentAttestor` only when that field is set and the session is not `sourceReview`. The source-review `session/new` host stays `{"sourceReview": true}` and does not gain the field.

The Studio lib test included `apps/web/public/cases/acp-cloud-agent.aix`, which is not in this checkout. The include now reads `apps/api/vendor/web/public/cases/acp-cloud-agent.aix`. That archive still admits and streams two turns on one session.

## Validation

Commands ran from the repository root with the pinned toolchain, except the renderer command, whose working directory was `apps/studio`. `pnpm` was not on `PATH`; the renderer command used `apps/studio/node_modules/.bin/vitest`.

| Command | Result |
| --- | --- |
| `cargo test --locked --offline -p a3s-studio --lib presenton_mcp -- --test-threads=8` | 4 passed, 813 filtered out |
| `cargo test --locked --offline -p a3s-studio --lib host_meta_drops_nul_and_unknown_language -- --test-threads=8` | 1 passed, 816 filtered out |
| `./node_modules/.bin/vitest run src/renderer/pages/agents/__tests__/presentonHomepage.test.tsx src/renderer/pages/agents/__tests__/presentationGenerate.test.ts` | 2 files, 11 passed |
| `cargo test --locked --offline -p a3s-studio --lib an_installed_acp_cloud_agent_streams_two_turns_on_one_session -- --test-threads=1` | 1 passed |

`cargo fmt -p a3s-studio -- --check` passed for `presenton_mcp.rs` and `turns.rs` before this fresh run. A warm ACP process restarts once because the host object changed. That restart was not observed in these checks.

## Explicitly not claimed

- Full web typecheck, full lint, a desktop GUI pass, and a live model call.
- Shared crates, the delivery-contract text, and the `office_file` implementation were not changed in this landing.
- Studio team, mail, organization, schedule, plugin, knowledge-compile, and lineage surfaces were not changed.
