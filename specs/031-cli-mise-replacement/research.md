# Research and decisions

## Baseline

- Latest release observed through the GitHub API: `v2026.10.25`, published
  2026-10-31T10:24:21Z.
- Upstream main observed: `d9d94fa49a1800fa449ca748efd279f8adc0af18`, dated
  2026-10-31T22:02:42Z. This is context, not a substitute for the release pin.
- Official feature source: <https://mise.jdx.dev/cli/> and the domain pages
  linked by `contracts/capability-matrix.md`.
- Functional parity preserves Aster namespaces and ACL; no mise subprocess,
  binary bundle, or service may provide the implementation.

## Existing implementation

The starting checkout has uncommitted tool/env/exec/activate additions. Retain
them, inspect every caller, and extend the existing owners. The imported
`codegen` directory name is upstream layout, not a generator contract for the
handwritten CLI source (`docs/migration.md`, product/dependency boundaries).

## Upstream reuse boundary

The pinned upstream `src/lib.rs` explicitly says that its own CLI is the only
intended consumer and its Rust API is unstable. The config implementation
exposes a `ConfigFile` trait, but discovery, mutation, settings, global config,
and bootstrap still depend directly on `MiseToml`. Calling its CLI unchanged
would retain another product configuration language and the upstream updater.
Therefore a whole-CLI delegate is not a compliant replacement. Reuse reviewed
algorithms and backend contracts inside Aster's owners where appropriate,
retaining provenance and licenses, while keeping ACL and Aster's updater.

Reviewed source at release commit `3c78bbb2d58c0b668fe8d5bebf4a8cb66194e1aa`:
`src/lib.rs`, `src/config/config_file/mod.rs`, `src/config/mod.rs`,
`src/config/config_file/mise_toml.rs`, and
`crates/mise-config/src/mise_toml.rs`.

## Version resolution contracts

Additional pinned review: `src/toolset/tool_request.rs`, `src/lockfile.rs`,
`src/semver.rs`, and `crates/mise-util/src/semver.rs` at the same release.
Do not substitute an npm range parser or a generic version comparator for
these behaviors without differential witnesses:

- Actual version selection uses separator-aware matching for both bare and
  explicit prefix requests. Bare requests prefer an exact version, whereas
  `prefix:` selects the newest match. The looser bare-prefix behavior in the
  old lock lookup is a separate contract; the earlier research conflated it
  with actual selection. Both `0.8` and `prefix:0.8` select `0.8.9` rather than
  `0.81.0` in the frozen executable witness. A leading `v` has context-specific
  behavior retained by the adapted matcher.
- Request kinds include version, prefix, reference (`ref/tag/branch/rev`),
  subtraction, path and system. Legacy reference hyphen spellings normalize
  at admission. An admitted scheme is not evidence that its backend works.
- Subtraction resolves its base first, truncates to the subtraction precision,
  and borrows from the previous nonzero component on underflow. For example,
  `2.79.0 - 0.0.1` produces prefix `2.78`, rather than a wrapped integer.
- Upstream uses `versions::Versioning` for general versions, a separate npm
  range contract for package.json, and SemVer precedence for complete SemVer
  comparisons. The currently installed generic `version-compare` crate is
  not automatically an equivalent replacement.

The current implementation now resolves exact/bare/prefix requests, provider
aliases and subtraction, including local selection and cached metadata. Ten
fixed cases match actual reference executions. Native path/system selection is
implemented in the tool owner and exec/install/use/which/activation. Actual CLI
comparisons against seven context references pass. Reference execution, user-defined aliases,
complete request grammar, multi-version declarations, backend-specific behavior
and the remaining lifecycle work keep T004 open.

`versions 8.0.1` is pinned to the exact upstream lockfile version and checksum.
The dependency already shares its `nom` and `itertools` versions with this
workspace. No external dependency version was upgraded. The tool crate now
inherits the root workspace Rust 1.97 baseline rather than advertising an
unverified standalone Rust 1.82 minimum.

## External and system selection

Seven additional native reference executions live in
`contracts/mise-context-reference.json`, captured by
`tests/cli_mise_context_reference.py` with the same pinned binary, isolated
state, paranoid mode and a fixed 25-second deadline. They cover absolute and
relative arguments, ancestor `.mise/config.toml`, home expansion, system
selection with a managed shim earlier in PATH, missing paths, and global
relative paths with an explicit global config root. The original ten-case
version reference is unchanged. The first comparison uses equivalent ACL
declarations. The updated CLI comparison also uses explicit temporary arguments.
`aster exec --tool TOOL@REQUEST` selects overrides without changing declarations;
all 18 updated CLI cases pass with the original deadlines. Refreshed pager tests
(15 cases) and the CLI/ACP paired build pass too.

Pinned review of `src/toolset/tool_request.rs` and
`src/config/config_file/mod.rs` establishes that relative paths use the declaring
workspace or explicit global root, while command arguments use cwd. Aster keeps
these origins while merging ACL layers; its user root is the native user home,
independent of ASTER_HOME. External/system selections create no managed receipts
and do not fetch. System PATH excludes the store's emitted shim/install roots.

Default layout discovery adapts `src/backend/github.rs`, `src/backend/npm.rs`
and `src/plugins/core/node.rs`, retaining the MIT notice and exact source blobs
in `docs/source-manifest.json`. Native executable lookup reuses workspace
`which 8.0.6`; GitHub directory traversal shares the existing 8192-entry bound.
Aqua registry-described layouts refuse explicitly until T008 supplies metadata.
Reference kinds, path/system locks, native Windows/Linux behavior and final
packaged acceptance remain unverified. The offline workspace resolver locked zero new
packages while adding the already installed `which` dependency edge; unrelated
concurrent Cargo.lock changes are not attributed to this feature.

The original CLI suite initially timed out all 17 cases at their unchanged
20-second command deadline. Every case passed alone with the same compiled test
and CLI binaries, followed by all 17 passing with the original parallel runner
in 58.20 seconds. This preserves the failed run and proves functional behavior;
it does not establish the cause of the initial timeout or release efficiency.

`tests/cli_mise_context_benchmark.py` measures five fresh-state/repeated-state
pairs for native path selection with identical external programs and unchanged
20/25-second command deadlines. The first debug Aster medians are 573.354 ms
with fresh owned state and 152.608 ms repeated; the optimized reference medians
are 443.656 ms and 16.361 ms. Host load and OS caches are uncontrolled, and the
build profiles differ. This is diagnostic evidence, not release acceptance.
The standard release workflow's paired build passed for a comparable profile.
Its Aster medians are 627.316 ms fresh and 210.642 ms repeated; mise medians
are 393.927 ms and 15.197 ms. This does not meet efficiency acceptance.
Before changing startup, an isolated native probe also verified that
one environment command writes 27 agent guide files. Environment commands now
skip this extraction and agent HTTP prewarming, retaining policy/sandbox
admission. Its 18 actual CLI cases, debug/release paired builds and owning Aster
Clippy pass. Guide-file output falls from 27 to zero. The first recapture hit
the unchanged mise deadline and is retained; the complete retry and release
measurements do not demonstrate a speedup.

The release archive contract ships `aster` and `aster-acp`, but the old default
package command required a separate `aster-use` executable. A sanitized-PATH
release probe reproduced this failure. The CLI now reuses the existing local
SDK parser and shared fallible output writer in process, retaining an explicit
`ASTER_USE_BIN` override. Six SDK tests and all 20 isolated actual CLI cases
pass. A copied CLI without an SDK sibling also checks the unchanged 14-file,
6262-byte publication archive. These checks precede the latest runtime-pool
optimization; current CLI/pair/owning-Clippy processes remain pending.
Environment commands now request one async worker by default and prewarm one
blocking worker, with the existing pool capacity and explicit override intact.
SDK worker tasks retain the standalone command's 8 MiB stack requirement.
No current performance improvement or final packaged acceptance is claimed.

## Native runtime archive and Node verification contracts

The frozen `src/cli/uninstall.rs` blob is
`3ee303ab6be92e4e874a932c6ec8bd3929a4dd24`. It gives an exact stored label
priority over fuzzy candidates, refuses ambiguous selection without `--all`,
and uses dry-run exit status to report pending removal. The native tool owner
now implements this candidate boundary and the CLI exposes named-tool all and
preview behavior. Its owning suite passes 114 cases with two ignored network
cases; CLI acceptance remains open. Eight completed native reference cases are
assembled only from passed independent cases with matching input/binary/hook
identities. Initial captures retain batch-setup and executable-start timeouts.
A completed exact case also shows
that mise recreates a compatibility symlink after deleting a short-label
installation; the collector must distinguish that link from a real payload
directory. The native explicit-prefix result corrected a second collector
assumption: `prefix:1.2` resolves one provider version and is not an ambiguous
bare fragment. The implementation and new tests now use the same distinction,
and preserve an older installation if the resolved version is not installed.
The original version/context reference inputs remain unchanged.

The complete-archive path is implemented in the existing tool crate. It retains
source-declared native paths rather than flattening a runtime to one binary.
SDK tests and the actual CLI execute fixture programs that read sibling data,
including linked entrypoints. This establishes the storage/execution boundary;
the official Node prebuilt provider now uses it, and archive-aware v3 locks
bind both compressed bytes and the complete v1 layout record.

Additional pinned source review: `src/plugins/core/node.rs` (blob
`fbb5ca44fbf99b50c9dd5be2a1c9fd897807899e`) and
`crates/mise-util/src/gpg.rs` (blob
`cb84aee7816e4ae448372925e37487363d16278a`). Node's binary layout is `bin/node`
on Unix and `node.exe` at the root on Windows. Source compilation, flavors,
patches, npm/corepack behavior and their configuration remain required too.

The baseline verifies `SHASUMS256.txt.sig` on every release line by default,
before using `SHASUMS256.txt` to pin the archive. It uses in-process OpenPGP,
with bundled trusted primary keys and subkeys. Its verifier handles binary and
armored signatures, tries issuer-matching keys first, and limits total content
hashing attempts by the trusted keyring. Issuerless signatures may use a bounded
fallback; a signature naming an untrusted issuer does not trigger that fallback.
The Node adapter now implements this verification contract, including genuine
signed historical data and rejection before downloading an invalid archive.

The exact public key bundle and signed Node v20.11.0 regression data are now
local assets, with Git blob and SHA-256 identities in docs/source-manifest.json.
The reference uses `pgp 0.21.0`, checksum
`ae70f4d9325a391db30d115d6191a7bb67cec856aa0bba131290f0c4e09a532a`;
that exact dependency now provides the in-process verifier. Genuine Node
20.11.0 installation, stdlib effects and npm execution passed on macOS arm64;
the first run exposed prerelease metadata that has a retained regression.
No build or runtime source dependency on the mise repository is required. Preserve
upstream notices for adapted algorithms and keep configuration native ACL.

## Confirmed starting failures

- Existing debug `aster tool install hermes` panics (exit 101): blocking
  reqwest client construction runs inside Tokio before `spawn_blocking`.
- An installed CLI at `~/.local/bin/aster` lacks the new environment commands;
  help output is not evidence that the final product has shipped them.
- Nested project execution does not load ancestor `env.acl`.
- Shims select a global current pointer; activation exports one static snapshot.
- The existing catalog lacks most core language runtimes.
- `cargo test --locked -p aster-use-tool-acquire` initially refuses because the
  root lockfile does not include current manifest changes. Let Cargo update
  the root lockfile offline, inspect the exact delta, then restore locked checks.

## Phase checks

- Clarify: no unresolved factual decision is needed for T001–T003. Preserve
  all baseline capabilities; incompatible configuration spelling uses ACL.
- Analyze (before implementation): no constitutional blocker or uncovered
  requirement found in the spec/plan/task mapping. FR-001–004 map to T001/003/004/008;
  FR-005–006 to T002/005; FR-007 to T006/007; FR-008 to T008; FR-009 to T009/010;
  FR-010 to T011; FR-011 and all completion gates to T012/013; FR-012 to T014;
  FR-013 to T008/011/012/013.
- Native verification beyond the available host remains a required completion
  gate, not an assumed compatibility claim.

## Tracked configuration and running-process retention

Pinned source review of `src/cli/prune.rs`, `src/config/tracking.rs`,
`src/toolset/mod.rs` and `src/toolset/running.rs` at the unchanged release
commit establishes that pruning resolves every tracked configuration offline,
retains its selected installed versions, and also retains versions used by
live native executables or absolute interpreted-tool paths. A current pointer
alone does not protect an unused version. Tool stubs, lock/snapshot variants,
exclusions and retargeted runtime links are additional retained requirements.

`tests/cli_mise_prune_reference.py` captures twelve fixed native cases using
the existing reference binary hash and 25-second per-command deadline.
`contracts/mise-prune-reference.json` retains the accepted capture. macOS
terminates an unsigned copy of the system sleep executable with signal 9;
the isolated fixture copy therefore receives an ad hoc signature before
installation. A separate native probe preserves that failure and the signed
copy's successful startup. Case inputs, expected retained versions and
deadlines did not change to make the running-version case pass.

Existing configuration loaders explicitly promise no writes. Tracking belongs
in an explicit CLI/store operation after a successful load; it must not change
those published read-only APIs. Aster's current `ToolStore::prune` removes all
non-current version directories, and the actual CLI does not admit `tool prune`.
Neither behavior satisfies the frozen lifecycle contract.
