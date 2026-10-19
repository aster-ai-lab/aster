# MaaS built-in service retirement

Date: 2026-10-18. Aster working tree based on `c4be02b3`; this is local source evidence, not a published release.

The shared working tree already removed the MaaS controller, catalog module and automatic MCP injection. This slice removes the remaining product-specific portrait mapping, 26 images, 32 skill translation keys in each locale, service integration guide and opt-in Core live-service test. Generic catalog and connector fixtures now use independent service names. The former service display name `翻译` no longer replaces a user agent's saved avatar.

The existing upgrade migration removes retired pins, managed skill directories and owned `~/.agents/skills` links. The tests verify that unrelated skills remain. A new HTTP test proves both tool enumeration and a `maas_affairs` invocation at the old `/mcp/maas` route return 404. Login and account-backed knowledge access remain separate from the retired `maas_*` tool services.

Historical evidence remains retained. The old business test plans are marked retired; the five documentation links to deleted service modules were replaced.

## Verification

Commands run from the Aster root unless noted. Rust toolchain: `rustup run 1.97.1`.

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test --locked -p aster-dream --lib maas -- --test-threads=1` | 13 passed, 0 failed | [Host retirement](host-retirement.log) |
| `cargo test --locked -p aster-dream --lib session_launch_announces_ -- --test-threads=1` | 3 passed, 0 failed; schedule, mail, presentation and document parse remain | [Host servers](host-servers.log) |
| Work `pnpm test:renderer` for the seven edited catalog, avatar and connector test files | 50 passed across 7 files | [Renderer](renderer.log) |
| `cargo build --locked -p aster-dream --bin aster-dream` | Passed; existing warnings retained | [Build](build.log) |
| Work `pnpm install --frozen-lockfile`, `pnpm i18n:sync`, `pnpm i18n:check` | Passed; 19,334 translations checked | [i18n](i18n.log) |
| Rustfmt for the edited host module and `git diff --check` for this slice | Passed | Source check |
| Work `pnpm lint` | Failed outside this slice at Oxlint; later chained gates did not run | [Lint](lint.log) |
| ESLint on edited frontend files | Three existing reverse-layer imports remain in the catalog hook; the same three errors occur when checking its HEAD version via stdin | [Current](scoped-eslint.log), [HEAD baseline](baseline-eslint.log) |
| Work `pnpm docs:check` | 122 other broken links remain after fixing the five service-retirement links; subsequent chained docs checks did not run | [Docs](docs.log) |

The separate full Work typecheck also failed in existing application/E2E files; it reported no diagnostics in the edited frontend files. This is not a claim that the whole repository passes its release gates.

## Deployment boundary

The native debug host compiled successfully. The running desktop process was not replaced, and no installer or release was published. A deployed rebuild and a fresh session are required to remove capabilities cached by an older host/agent process. The macOS `native_process_lifetime_unqualified` defect remains open and is not fixed by this retirement.

[Source fingerprints](source-sha256.txt) bind the checked service-selection, migration and UI source files. Other sessions' in-flight changes were preserved and were not committed by this slice.
