# Remove search_skills and find-skills

Date: 2026-10-19. Local source evidence. Not a published release.

The session tool `search_skills` is no longer registered. The bundled skill `find-skills` (查找技能) is gone from AsterDream and Former. `web_search`, `web_fetch`, and `Skill` stay. The catalog lists each discoverable local skill as a name and a description, and tells the model to call `Skill`.

Startup still removes a symlink whose target ends in `resources/skills/find-skills`. A real directory with that name stays. `prepare_builtin_skill("find-skills")` returns `unknown_builtin`.

## Verification

Rust toolchain: the workspace pin `1.97.1`. Commands run from the repository root unless noted. Host packages were tested separately because `aster-dream` and `a3s-desktop` share the library output name `a3s_desktop_lib`.

| Check | Result |
| --- | --- |
| `cargo test --offline --locked -p a3s-code-core --lib -- test_all_prompts_loaded active_run_keeps_n_skill_search_registry_and_use_lease_across_cutover live_checkpoint_keeps_source_run_knowledge_across_concurrent_cutover test_to_system_prompt test_skill_tool_schema_enforces_canonical_shape` | 5 passed, 0 failed, 5108 filtered out. Exit 0. Tests 0.44s after a 53.31s compile |
| `cargo test --offline --locked -p a3s-code-core --test live_skill_lifecycle --test test_skill_capability_integration --test test_skill_mcp_decisions_real_llm -- live_skill_add_remove_is_visible_and_restores_the_session_shadow closed_session_rejects_live_skill_mutation session_registers_skill_tool_and_loads_session_skill_dirs probe_naming_and_skill_tools_admit_offline` | 4 passed, 0 failed, 6 filtered out. Exit 0. Three binaries: 2, 1, and 1 passed |
| `cargo test --offline --locked -p a3s-code-acp --lib -- empty_registry_still_states_the_product_boundary native_skill_and_real_stdio_mcp_stay_frozen_and_ready_processes_retain_their_package` | 2 passed, 0 failed, 316 filtered out. Exit 0. Tests 3.79s |
| `cargo test --offline --locked -p aster-dream --lib -- drops_retired_builtin_symlinks_and_keeps_a_directory builtin_skills_link_without_replacing_a_directory ls_sc_01_a_user_directory_is_kept_over_http` | 3 passed, 0 failed, 791 filtered out. Exit 0. Tests 0.07s |
| `cargo test --offline --locked -p a3s-desktop --lib -- builtin_skills_link_without_replacing_a_directory drops_a_retired_find_skills_bundle_link_and_keeps_a_directory` | 2 passed, 0 failed, 815 filtered out. Exit 0. The first run failed on ` ```aix-ui `; the bundled fence is ` ```openui `. The rerun passed |
| `apps/work` `vitest run --project renderer` on homeSlashCatalog, CapabilitiesPage, capabilitiesData, harnessEvolution, harnessDetailCatalog, TasksSettings | 6 files, 172 tests passed, 0 failed. Exit 0. Duration 10.81s |
| `apps/desktop` the same six renderer files | 6 files, 172 tests passed, 0 failed. Exit 0. Duration 11.90s |

## Explicitly not claimed

No claim that a running AsterDream or Former process has dropped the menu. The desktop hosts were not rebuilt or launched. Marketplace install, shell enablement, and `office_file` visual restyle were not added. The renderer skills.sh dialog and `cherry-tool-guide` stay. `web_search` and `web_fetch` were not removed. Historical evidence files were not rewritten. Two unrelated `a3s-code-core` integration targets, `agent_portable_checkpoint_recovery_v1` and `agent_protocol_harness`, still fail to compile and were not part of this change.
