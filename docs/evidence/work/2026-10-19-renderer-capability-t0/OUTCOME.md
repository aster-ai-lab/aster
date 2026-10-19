# T0 — renderer preference capability

Date: 2026-10-19. Status: development-origin fix verified; uncommitted.

## Result and scope correction

The existing Work change adds only `http://127.0.0.1:4320/*` to the
renderer capability and updates its description. That change was already
present when this task started; its authorship is not established, and its
bytes were preserved. This task adds a sibling regression module and its
`#[cfg(test)]` registration. Onboarding and other working-tree changes
were not edited or staged.

The v4 plan's packaged-build premise is disproven. There is no packaged
renderer server-port constant of 4320. Native `renderer_port()` in
`apps/work/src-tauri/src/lib.rs` reads `A3S_STUDIO_RENDERER_PORT`, falling
back to 4318. `apps/work/scripts/studio-shell.sh` supplies 4320 for Work
development. Release windows use `WebviewUrl::App` and load
`tauri://localhost/windows/main/index.html` on macOS. Adding a remote
4320 grant fixes the development origin, not the packaged origin.

## Historical evidence

The bounded extracts and counts are retained in `origin-observations.txt`.
The original logs remain under `/Users/roylin/Library/Logs/AsterDream/`.

| Existing launch log | Main document | Preference ACL denials |
| --- | --- | ---: |
| `aster-dream-20261019-201838-56466.log` | HTTP 4320 | 54 |
| `aster-dream-20261019-202105-56793.log` | HTTP 4320 | 0 |
| `aster-dream-20261019-211743-69231.log` | packaged `tauri:` | 0 |

The first log explicitly says `preference_read not allowed`, listing
only local and 4319 access. These are historical observations, not new
launches performed by this task. The controlled comparison below supplies
the causal proof independently of those log counts.

## Verification

All commands run from `/Users/roylin/code/aster` with Rust 1.97.1.

- `rustup run 1.97.1 cargo test --locked -p aster-dream --lib renderer_capability_tests --no-run`:
  passed; retained in `build.log`.
- `A3S_STUDIO_RENDERER_PORT=4320 rustup run 1.97.1 cargo test --locked -p aster-dream --lib renderer_capability_tests -- --test-threads=1`:
  2 passed; retained in `tests-4320.log`.
- Ran the same compiled test target's `renderer_capability_tests` with
  `A3S_STUDIO_RENDERER_PORT=4318` and `4319`: 2 passed per port.
- Ran that target's exact
  `preferences::tests::preference_ipc_reads_and_commits_after_a_child_webview_is_created`
  with `A3S_STUDIO_RENDERER_PORT=4320`: 1 passed. This exercises the
  registered native command handlers, host-token and child-webview checks,
  and an ACL preference store in a temporary directory. No real user
  preference file or credential was used.
- Targeted `rustfmt --edition 2024 --check --config skip_children=true`
  on `src-tauri/src/lib.rs` and `src-tauri/src/renderer_capability_tests.rs`:
  passed. `git diff --check`: passed.
- `rustup run 1.97.1 cargo fmt --all -- --check`: failed on unrelated
  existing formatting differences; final output retained in
  `workspace-format.log`. No workspace formatting was applied.

The executable reported by the successful compile was
`/Users/roylin/code/aster/target/debug/deps/a3s_studio_lib-d46b292e4f47e130`.
Each direct invocation used `--test-threads=1`; the IPC invocation also
used `--exact`.

## Controlled failure and adversarial checks

- Reconstruct the old policy in memory by removing only the 4320 remote
  URL, keeping the actual generated Tauri permission manifests. Both
  preference commands are denied on 4320. Restore the shipped policy:
  both are allowed. Existing 4319 access stays allowed in both policies.
  This compares the failing and corrected policies without overwriting
  someone else's uncommitted capability file.
- Packaged `Origin::Local` permits both commands even with 4320 removed,
  falsifying the claim that this remote grant repairs packaged access.
- The compiled full application context permits the renderer URL selected
  by the native `main_renderer_url(false)` function, rather than relying
  solely on a copied port value.
- Foreign HTTPS, `localhost:4320`, and loopback port 4321 remain denied.
  A separate applet window remains denied for both commands.
- The existing IPC test verifies that an embedded child webview cannot
  read preferences while its owning main renderer can read and commit.

## Alternatives not selected

- No wildcard-port grant: the fix needs one specific development origin.
- No new default-port constant or launcher redesign: 4320 is a launcher
  override, and the native selector already exists.
- No fresh DMG: historical packaged logs and the native URL-selection
  source contradict the packaged-4320 premise. Signed release delivery
  remains the plan's later release wave.
- No rewriting or committing the pre-existing capability change to claim
  ownership; the regression independently verifies it.

## Explicitly not claimed

- No new packaged-build or GUI reproduction, signing, notarization,
  installation, release publication, or live application restart.
- No full-workspace test pass or clean workspace-format pass.
- No claim that all eight issues are fixed or that the four pending
  product decisions are approved.
- No commit, staging, or submodule-pointer advancement. The independent
  `issues` Git repository described in the handoff is absent on this
  machine; no repository was initialized there.
