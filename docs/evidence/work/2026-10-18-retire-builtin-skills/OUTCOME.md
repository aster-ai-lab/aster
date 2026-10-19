# Retire built-in 校核 and 表格计算

Date: 2026-10-18. Local source evidence that AsterDream no longer ships the built-in skills `proofread` and `sheet-compute`. Not a published release.

The host catalog does not list those names. Startup removes a symlink whose target ends in `resources/skills/<name>` and leaves a real directory in place. An installed skill that still uses the name `proofread` keeps the Chinese label 校核 through the workbench copy keys.

## Verification

Rust toolchain: `rustup run 1.97.1`. The host command runs from the Aster root. The renderer command runs from `apps/work`.

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test -p aster-dream --lib --offline -- drops_retired_builtin_symlinks preserves_custom_skill_directories repairs_a_bundled_skill_link builtin_skills_link_without_replacing_a_directory ls_cb_01_and_ls_ctg_01_unwired_skills` | 5 passed, 0 failed, 0 ignored, 750 filtered out. Exit 0. Test run 0.32s after a 56.49s compile | `{SCRATCH}/builtin-skills.log` |
| `vitest run --project renderer src/renderer/pages/agents/__tests__/homeSlashCatalog.test.tsx` | 1 file passed, 14 tests passed, 0 failed. Exit 0. Duration 2.76s. Start 13:44:44 | `{SCRATCH}/slash-catalog-evidence.log` |

## Explicitly not claimed

No claim that a running AsterDream process already dropped the two cards. The desktop host was not rebuilt or replaced. MaaS historical fixtures and the catalog-owned `proofread` link in the MaaS retire test were not deleted. `workbench.composer.skills.proofread` and `workbench.composer.skills.sheet-compute` remain.
