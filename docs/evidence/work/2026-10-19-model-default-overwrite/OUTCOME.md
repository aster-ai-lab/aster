# Reconfiguring models updates the Work defaults used for inference

## Cause and repair

The model setup shell posted all three selected model references to
`POST /api/providers/defaults`. `ProviderStore::set_defaults` saved them in
`providers.json`, but left the native device preferences unchanged. The
workbench reads `chat.default_model_id` before considering the shell fallback,
so an existing preference kept the previous model active.

The shared save path now also writes `chat.default_model_id`,
`feature.quick_assistant.model_id`, and `feature.translate.model_id` with the
exact validated `providerId::modelId` references. It uses the existing native
ACL store and device lock, updating the three keys in one atomic ACL write.
Both setup and the workbench picker already use this endpoint. Setup then
navigates to the main document after login; the picker rereads native
preferences before its existing preference commit. No new IPC command or
frontend synchronization mechanism is needed for those callers.

The existing one-key host preference helper reuses the batch device writer.
The writer refuses member keys before touching storage. Existing model
eligibility checks, preference validation, revision handling, tenant paths,
credentials, unrelated preferences, and the earlier development-seed
preservation fix remain in place.

A provider-file failure restores the in-memory defaults without changing
preferences. A preference failure returns `preferences_unavailable` and
attempts to restore the previous provider defaults on disk and in memory.
The two stores cannot commit atomically across process termination or power
loss; rollback itself can also fail during a wider storage failure. No success
response is returned in either write-failure path.

## Regression reproduced before the production change

```text
cargo test --locked -p aster-dream --lib providers::default_tests -- --nocapture
exit 101: 0 passed, 1 failed
chat.default_model_id: openai::old-model, expected openai::new-model
```

## Validation

```text
cargo test --locked -p aster-dream --lib providers:: -- --nocapture
exit 0: 22 passed, 0 failed, 0 ignored

cargo test --locked -p aster-dream --lib preferences::tests -- --nocapture
exit 0: 16 passed, 0 failed, 1 ignored

cargo check --locked -p aster-dream --lib
exit 0: Work library and non-test development startup compile; existing warnings remain

cd apps/work
./node_modules/.bin/vitest run --project renderer \
  src/renderer/shell/desktopClient.test.ts \
  src/renderer/data/__tests__/desktopProviderPlane.test.ts \
  src/renderer/hooks/__tests__/useModel.test.ts \
  src/renderer/pages/agents/__tests__/WorkbenchModelPicker.test.ts \
  src/renderer/pages/agents/__tests__/useWorkbenchDefaultAgent.test.ts
exit 0: 5 files, 115 tests passed

python3 tests/just_work.py
exit 0: 7 tests passed
```

The four new provider regressions exercise overwriting distinct default,
quick, and translation slots; closing and reopening both stores; preserving
an unrelated member preference; unavailable preference storage; provider-file
write failure; and rejecting an unavailable model without changing either
default store. A fifth regression verifies that the host device writer cannot
write member preferences. All fixture stores are temporary directories.
The ignored preference test is the existing stdio transport entry point.

Changed-source `rustfmt --edition 2024 --config skip_children=true --check`
and scoped `git diff --check` passed. Full `cargo fmt --all -- --check` failed
on 20 other files, including `apps/runtime/src/auth.rs`; none of the four
changed Rust files appeared in that report.

`/opt/homebrew/bin/python3.13 tests/workspace_dependencies.py` ran 6 tests:
5 passed and 1 failed because the unrelated `apps/gateway/Cargo.toml` package
`a3s-gateway` reports `publish = None` instead of `[]`. The first attempt with the system Python 3.9
could not import `tomllib`; the run above used the installed Python 3.13.
`pnpm` was absent from the shell PATH, so the frontend run used the installed
Vitest executable directly, without installing or changing dependencies.

## Explicitly not claimed

- No full-workspace green result; the unrelated check failures were preserved.
- No live model-provider request, GUI campaign, or running app replacement.
- No real user configuration, credentials, preferences, or sessions were read
  or changed by the regression tests.
- No installer was rebuilt, signed, uploaded, or published by this repair.
- No claim that separate provider and preference stores are crash-atomic.
