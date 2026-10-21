# Debug startup preserves saved model choices

Status: source repair and focused validation complete. No live
binary was built, replaced, deployed, or restarted by this slice.

## Repair and scope

The earlier [read-only restart audit](../2026-10-19-model-selector-restart-audit/OUTCOME.md)
identified two overwrite paths in Work's debug startup. `ProviderStore::open_at`
applies the development seed, whose `apply_seed` previously replaced all three
model-default slots. `publish_dev_chat_default` then replaced a differing
device `chat.default_model_id`, so a saved UI selection could be lost again.

Work `providers.rs` now fills only absent `default_model`, `quick_model`, and
`translate_model` slots. The startup preference publication produces an update
only when no preference value was stored. A saved value, including an explicit
empty/null preference, is preserved. A fresh store still receives the seed.
The small publication decision is private and used by production startup and
the hermetic persistence tests; no public API changed.

Provider installation, configured endpoint/key/model refresh, catalog ordering,
default validation, persistence error handling, startup ordering, and release
build behavior retain their existing code paths. `ProviderStore::open` still
runs before preference publication, preserving development credential refresh.
Desktop has no corresponding development-seed/publication branch and was not
changed. No real ACL, key, account/session state, preference, or provider store
was read or edited.

## Red before the repair

```text
cargo test -p aster-dream --lib providers::seed_tests -- --nocapture
exit 101: 1 passed, 3 failed, 0 ignored
```

Fresh initialization passed. Persisted model selection followed by reopen and
reseed reverted to `seed-model`; a persisted device chat choice still produced
an overwrite update; and an already filled default slot was replaced while
filling other absent slots. Before this run, the publication decision was only
extracted into a private helper with its original overwrite behavior; the
repair had not yet been applied.

## Validation

```text
cargo test -p aster-dream --lib providers::seed_tests -- --nocapture
exit 0: 4 passed, 0 failed, 0 ignored

target/debug/deps/a3s_desktop_lib-d46b292e4f47e130 providers:: --quiet --test-threads=1
exit 0: 18 passed, 0 failed, 0 ignored
```

The second command uses the fresh test executable produced by the first,
after the final source repair; it includes all existing provider regressions
and the four new cases. The four small tests use only declared fixture
ACL text, temporary provider storage, and a temporary native `PreferenceStore`.
They exercise the normal validated `set_defaults` write and preference commit,
close/reopen, then development reseeding/publication decisions. The partial
default matrix also verifies fixture credential/endpoint refresh remains
unchanged for all eight filled/missing combinations.

Owned-source `rustfmt --edition 2024 --config skip_children=true --check`
and `git diff --check` passed after the repair. The current exact two Rust
source identities are listed in [source-sha256.txt](source-sha256.txt), with
aggregate SHA-256 (sorted path, NUL, full bytes, NUL):

```text
8c420de6b4a44a61baca5d3323a12abad2e86322bf4d6c3dd234e98af1166caf
```

`shasum -a 256 -c source-sha256.txt` verified both files after the final tests.
No additional production-source edits followed that verification. Existing
unrelated host warnings did not fail the focused compile or tests. The root
agent retains the later product/debug-startup build and adoption decision;
this slice does not substitute unit evidence for a new app restart.

## Explicitly not claimed

- No live app restart/model-restoration PASS; the frozen v7 observation remains
  unchanged and requires a fresh reviewed binary plus UI verification.
- No supplier-credential policy change, model capability inference, provider
  name matching, new default selection, or configuration migration.
- No actual user ACL/keys/preferences/account/session data inspection or edit.
- No full-workspace green, application bundle build, deployment, or task score
  change.
