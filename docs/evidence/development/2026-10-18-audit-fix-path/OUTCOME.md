# Desktop lock, search update refusal, and unpublished update checks

## Result

Both desktop shells refuse a second lock, a quiet claim, and a lock that
cannot be opened. The production lock parent is the tenant data directory.
`a3s-search update` stops before any HTTP request. Desktop and AsterDream
treat a missing published update as unpublished. A real update failure still
surfaces the update error.

## Commands and counts

```
cargo test -p aster-dream --lib single_instance --offline
```

4 passed, 0 failed:
`second_claim_is_busy_and_asks_the_first_window_to_show_itself`,
`quiet_claim_does_not_notify`, `unopenable_lock_path_is_not_owned`, and
`a_missing_tenant_directory_does_not_fall_back_to_temp`.
Rerun after the last `apps/work/src-tauri/src/lib.rs` edit: 4 passed, 0 failed, exit 0.

```
cargo test -p a3s-desktop --lib single_instance --offline
```

4 passed, 0 failed. Same four cases, including the second claim busy, the
quiet claim not notifying, and the unopenable lock path not owned.

```
cargo test -p aster-dream --test platform_roots --offline single_instance_second_claim_activates_the_first
```

1 passed, 0 failed. Rerun after the last `apps/work/src-tauri/src/lib.rs` edit: 1 passed, 0 failed, exit 0.

```
cargo test -p a3s-search --offline
```

758 passed, 0 failed, 3 ignored. Per target: 554, 53, 11, 2, 1, 11, 1,
41 passed with 1 ignored, 3, 9, 20, 12, 35 passed with 2 ignored, and 5
doc tests. Rerun after the prompt files were restored to HEAD: 758 passed, 0 failed, 3 ignored, exit 0.

```
cargo run -p a3s-search --offline -- update
```

Ran twice. Both exited 1 with `Aster is built from source; rebuild this workspace to update it`.
Neither run performed an HTTP request.

A search of source, manifests, and the lockfile for `a3s_updater`,
`a3s-updater`, and `crates/updater` found no call and no provenance path.

```
cargo test -p a3s-desktop --lib --offline app_updater
cargo test -p a3s-desktop --lib --offline host_family
```

7 passed and 4 passed, 0 failed. The connect-failure check is a pure
function over an `std::io::Error`. Linux and Windows updates stay unsupported.

```
apps/desktop/node_modules/.bin/vitest run --project renderer src/renderer/windows/main/hooks/__tests__/useAppUpdateHandler.test.ts
```

6 passed, 0 failed. `apps/desktop/node_modules` was already present, so
nothing was installed. The unpublished manual check does not open the
update-error popup. A real update error still does.

```
cargo test -p aster-dream --lib --offline app_updater
```

15 passed, 0 failed. Gate rerun: 15 passed, 0 failed, exit 0. `signature_download_and_install_failures_stay_failed` passed. The plugin download error `Download request failed with status: 404 Not Found` stays failed. The log names the cases:
`unsupported_architecture_is_not_published`,
`one_missing_platform_is_not_published`,
`a_missing_fallback_set_is_not_published`, `network_down_stays_failed`,
`signature_download_and_install_failures_stay_failed`,
`linux_x86_64_accepts_an_empty_windows_signature`,
`an_a3s_lab_url_on_another_os_rejects_the_feed`,
`an_empty_signature_on_the_current_key_rejects_the_feed`,
`an_empty_platforms_object_fails`,
`a_feed_with_no_entry_for_this_machine_still_parses`,
`kylin_packages_use_the_linux_deb_and_rpm_keys`, and
`bundle_version_matches_the_package_the_page_shows`.

```
cargo fmt -p aster-dream -p a3s-desktop -p a3s-search -- --check
```

Exited 0. Rerun after the download-404 classification fix: exited 0.

```
cargo fmt --all -- --check
```

Exited 1. 156 files differ, in `apps/api`, `apps/auth`, `apps/billing`, and `apps/config`. `crates/flow/src/store/migrations/tests.rs` is clean. This command did not pass.

`crates/code/core/src/prompts.rs`, `crates/code/core/prompts/common/skills_catalog_header.md`, and `crates/code/core/src/prompts/tests.rs` were restored to HEAD. Those edits are outside the four approved slices.

```
cargo test -p a3s-code-core --lib --offline prompts::tests::
```

46 passed, 0 failed, exit 0. The log includes `test_all_prompts_loaded` and `test_slots_output_language_pins_reply_language`.

## Explicitly not claimed

The CLI self-update gate and the installer behind it were not changed.
Desktop stays at 2.1.3, is not wired to the GitHub feed, and gained no Linux
installer. Native selects, the two Office editor trees, and the DESIGN.md
sentence were left as they were. The updater plugin was not forked, and
`loongarch64` is not a supported install. No release was published, no
commit was created, and no i18n, config, or shared lock crate was added.
AsterDream and Desktop were not launched, About was not opened, and `aster-dream` and rsbuild were not killed.
The running debug app is unchanged until the next build.

Two older compiler lines in `docs/evidence/terminal/2026-10-18-native-forms/` still name `a3s-updater`.
Those logs were not rewritten. They are not a call site or provenance.
