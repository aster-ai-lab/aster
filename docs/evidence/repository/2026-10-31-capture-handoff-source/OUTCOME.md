# Capture handoff source integration

Merge preserved dirty-source checkpoint
`91f4ed454c3e703be5d000ae12c7190b08a3c86a` through actual ancestry.
Its 291 changed paths were compared with the canonical tree; 275 already
matched exactly. Fourteen conflicted files were reviewed, and the two other
differences merged without a source delta. The resulting complete source
tree equals the pre-merge HEAD before this evidence record is added.

Retain the later Capture/CaptureKit naming and legacy bundle compatibility,
macOS embedded library default, explicit standalone `ui` build commands,
platform-scoped audio dependency, shared root build profiles and removal of
the demo's extra workspace declaration. The current generated Cargo lock is
restored as a whole, without hand-editing or downgrading dependencies. The
current plan also retains the user's later commit/merge/push authorization.

## Verification actually completed

- `git diff --cached --name-only`: empty after all source resolutions.
- `git ls-files -u`: empty.
- `cargo metadata --locked --no-deps --format-version 1`: exit zero. Capture
  is a root-workspace member at `crates/capture/Cargo.toml`, and its default
  feature list remains exactly `["native-capture"]`.
- The owning source-provenance JSON parses successfully.

The older temporary consolidation directory disappeared during the parent
integration work. All six retained parent source checkpoint refs and the
corresponding Git objects were revalidated. The later registered full-access
head `c871987d8fd611d875e7d715521ee0fbeb287586` is additionally retained at
`refs/aster-preservation/20261031/late-aster-full-access`; its source still
requires reconciliation. No missing worktree was pruned or recreated.

## Explicitly not claimed

No new Capture behavior, Rust/Swift runtime campaign, native GUI acceptance,
all-workspace completion, directory cleanup, push, DMG or release is claimed.
Earlier temporary raw artifacts are not claimed present after their directory
disappeared; already committed evidence and preserved source refs remain.
