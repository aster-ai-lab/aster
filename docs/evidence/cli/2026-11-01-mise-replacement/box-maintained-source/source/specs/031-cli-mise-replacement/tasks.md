# Tasks: Aster CLI replaces mise

**Input**: [spec.md](spec.md), [plan.md](plan.md).

Each behavior task includes its owning-package hermetic tests and updates the
capability matrix with actual evidence. Unchecked tasks remain required for the
full goal. No Git commit or additional worktree is authorized.

## Phase 1: Behavior and tests

- [x] T001 [US1] Fix blocking-client lifecycle in `env_manager_cmd.rs`; move
  its tests to `env_manager_cmd/tests.rs`; test async installation refusal
  without a panic and empty installation without HTTP access.
- [x] T002 [US2] Extend `crates/use/tool-acquire/src/env_acl.rs` for bounded,
  closed ACL discovery and ancestor precedence; sibling tests cover nested
  paths, spaces/non-ASCII, malformed roots, oversized files, and typed errors.
- [x] T003 [US1] Extend `store.rs`, `acquire.rs`, and command execution to use
  project-selected installations rather than the global current pointer;
  execute two fixture versions and prove project isolation and offline reuse.
- [ ] T004 [US1] Add complete version requests, resolution, lock records,
  locked/offline install, upgrades, pruning, and tool selection commands in the
  tool owner; verify drift, missing payloads, checksums, and rollback.
  Partial evidence now covers bare/prefix/provider-alias/subtraction selection,
  bounded remote version caches and v2 selectors with v1 lock compatibility.
  Native path/system origins and executable lookup now pass library/pager and
  actual CLI comparisons against seven new pinned reference cases. Temporary
  exec overrides now pass library, pager and actual CLI checks and the paired
  build. Reference execution, path/system locks, user aliases,
  multi-version shims/stubs/synchronization, upgrades,
  pruning, unlocked/new-schema migration of ambiguous legacy labels, and
  the complete backend-specific contract remain required.
  V1 locks now preserve literal reserved/reference-like requests through
  parsing, rendering, declaration resolution, verified installation and offline
  reuse. They retain v1 wire semantics; newer schemas keep current operators.
  Exact per-version removal now has a native store API and CLI wiring. It
  preserves other versions, removes matching records and clears only a removed
  current version's pointer/shims. `--unpin` validates ACL before deletion and
  retains a different literal project pin. Removal uses the admitted receipt
  even when the version directory or executable is missing; execution retains
  its completeness checks. Exact-first/prefix candidates now support named-tool
  `--all`, ambiguity refusal, `--dry-run` and `--dry-run-code`. Preview preserves
  payloads/pins; selection admits complete receipt identities independently of
  executable completeness. Eight new pinned native cases also distinguish
  bare fragments from explicit prefix operators: operators/provider aliases
  resolve through source-identified metadata and do not fall back to an older
  installed version. Earlier evidence records 114 tool cases/two ignored,
  20 CLI failures, startup timeouts and a mid-replay artifact change. Those
  failures remain retained; the final-source results below supersede their
  acceptance status. Project removal, full backend selection, rollback,
  upgrades and pruning remain required; batch install/use results follow below.
  Multiple-tool removal and global `--all` are now implemented with shared
  bounded discovery, per-request preflight and deduplicated removal. Five new
  native cases from the unchanged pinned mise binary pass and are retained in
  `contracts/mise-multi-uninstall-reference.json`. Final-source verification
  now passes 129 tool cases (four ignored), 15 pager
  cases, the paired CLI/ACP build, owning tool Clippy and seven actual CLI
  uninstall cases. Those CLI cases include both the five new and eight earlier
  frozen mise comparisons. The first full CLI batch passes 18 cases and times
  out in eight; all eight pass unchanged isolated replays with stable binary
  hashes. Four new JSON-index lint findings are fixed without allowances; the
  remaining 42 pager findings are outside the changed file. Full concurrent
  CLI acceptance and package-wide Clippy remain open, as do `uninstall
  --project`, full backend selection, upgrades, pruning and the other T004 work.
  Batch install/use now validate all arguments before acquisition and write
  declarations once after success. Bare install uses the merged declaration;
  bare use reuses the newest stable complete installation, ignoring remote
  aliases while retaining prerelease flags. Explicit latest stays a provider
  request. Seven new frozen mise cases pass actual CLI comparisons. Current
  evidence passes 130 tool cases/four ignored, 18 pager cases, the paired build,
  owning tool Clippy, root formatting and seven workspace checks. The complete
  sequential CLI batch passes 28 cases and times out once; the same test binary
  passes that case unchanged in isolation. Pager Clippy repeats the other 42
  errors for lib and lib-test (84 diagnostics); no lint allowance was added.
  Five more native mise cases freeze ordered version arrays and fallback
  executables in `contracts/mise-multiversion-reference.json`. Aster's earlier
  declaration map then admitted one request per tool, so multi-version use and
  declarations were still missing at that capture. They are now implemented
  through an additive `EnvDeclarations` API that preserves the scalar Rust API.
  Ordered lists, whole-list ancestor replacement, version-specific unpinning,
  fallback executable lookup and v4 multi-version locks pass 134 tool tests
  (four ignored), 18 pager tests and all 30 actual CLI integration tests. The
  paired CLI/ACP build and owning tool Clippy pass. The five frozen native mise
  cases are unchanged. Temporary overrides now preserve each tool's ordered
  requests through the shared resolver and CLI, with exact duplicate collapse
  and unchanged legacy scalar APIs. Nine additional fixed native mise cases
  (24 commands) agree with the focused actual Aster CLI comparison in offline
  and no-install modes. The tool suite passes 138 cases/five ignored; tool and
  CLI Clippy pass. All 31 sequential CLI cases and 18 pager command
  regressions pass. Rebuilding both binaries resolves the observed post-test
  identity mismatch; all 31 cases pass again against the final pair, with
  unchanged artifact hashes and matching build identities. Pager Clippy still reports
  42 diagnostics at the same 39 prior
  primary locations outside the changed command file. Full
  shim/stub/synchronization and the other unchecked T004 work remain open.
  Unix project dispatch shims now select ordered declarations from the current
  directory, install a missing selected version through the native backend,
  retain native arguments and the bound tool store, and rebuild commands from
  every recorded version. `tool reshim` repairs missing commands and retires
  only obsolete owned scripts. Six additional native mise cases are frozen;
  five exact fixture comparisons and a separate real uv 0.13.6 auto-install
  case pass. Final verification passes 139 tool tests/five ignored, 18 pager
  command tests, all 32 sequential CLI cases and the explicitly run external
  auto-install case. The final CLI/ACP pair has matching identities and stable
  hashes across both regressions. Tool/CLI Clippy and root format pass. Pager
  Clippy retains the same 42 diagnostics/39 locations outside the command
  module. Windows project dispatch, stubs, remaining synchronization modes,
  reference execution, upgrades, pruning and the other T004 requirements are
  still open. See `mise-shims-results.json` in the CLI evidence directory.
  Project-scoped removal now uses only the nearest base project ACL, preserving
  its bytes and skipping external path/system requests. Eight new native mise
  cases cover scope, installed-prefix selection, ordered requests, empty/missing
  projects, confirmation and preview. A ninth freezes removal with a missing
  executable. The new read-only identity selector preserves execution's
  completeness mode and source/receipt validation. Final-source checks pass
  141 tool tests/five ignored, 18 pager tests, three focused CLI cases and all
  35 default sequential CLI cases. Two real A3S TUI scenarios validate decline
  and confirmation using uv 0.13.6. The same real uv executable hash reproduces
  the cleanup failure before the fix and successful removal after it, with
  unchanged configuration. Final paired identities and sampled source/artifact
  hashes agree throughout. Tool/CLI Clippy, root/scoped format and seven
  workspace checks pass. Pager Clippy retains 42 errors at the same 39 prior
  locations and the earlier test-only canonicalization warning. Invalid-byte
  filenames remain a Linux-native check because macOS rejects that fixture
  before CLI admission. All project profile/file variants, native platforms,
  upgrade/prune/reference/backend modes and the other T004 work remain required.
  Evidence is in `mise-project-uninstall-results.json` in the CLI directory.
- [ ] T005 [US2] Add environment profiles/directives/templates/secrets,
  aliases, virtual environments, and shell hooks in the environment owner;
  real shell tests prove quoting, directory switching, restoration, and trust.
- [ ] T006 [US3] Add ACL/file tasks, argument contracts, graph scheduling,
  ordering, bounded parallelism, output/exit propagation, and cancellation;
  fixture executables assert actual effects, cycles, shared dependencies, and
  owned process-tree termination.
- [ ] T007 [US3] Add incremental checks, watcher behavior, task templates,
  local/remote artifact caches, and monorepo graphs; test actual cache restores,
  invalidation, integrity failures, cross-project contexts, and event reruns.
- [ ] T008 [US1] Complete core language runtimes and every frozen backend /
  plugin contract in `crates/use/tool-acquire` or its existing owners; test real
  archive/package layouts, runtime prerequisites, verification, and platform
  selection. Parsed names or entry-file extraction are insufficient.
  `HttpSource::for_test` is now gated to unit tests or the non-default
  `test-support` feature; Studio opts in through its dev-dependency only.
  The HTTP owner now preserves typed transport/read/parser sources, strips
  request URLs from errors, and bounds metadata bodies like archive downloads;
  these checks do not complete the backend or cancellation contracts.
  The source protocol and store now admit complete verified tar.gz/ZIP layouts,
  native internal links, file modes and multiple entrypoints. CLI PATH/which
  use those layouts. V3 ACL locks bind archive bytes and the complete layout
  record before publication. The official Node prebuilt provider verifies GPG
  signatures, preserves Node/npm/npx/corepack layouts and resolves LTS aliases.
  External Node/npm/pipx/GitHub default paths now use native lookup without
  receipts or fetches. Aqua registry-described layouts still refuse explicitly;
  full backend options and native Windows/Linux execution remain required.
  Actual Node 20.11.0 stdlib/npm execution passed on macOS arm64. Directory
  metadata, backend-specific budgets, source builds/flavors/options, complete
  runtime behavior and all other required core providers remain open.
  User follow-up requires `aster-ai-lab/plugins` as the official trusted AIX
  source, domain-organized authoring and GitHub Packages/GHCR artifacts.
  Integrate native AIX publication checks, signed catalog/OCI digest binding,
  installation/update/cache behavior and real repository publication evidence;
  preserve all existing extension authorization and legacy compatibility.
  Native AIX inspection/CLI checks, shared catalog admission and the GHCR
  manifest/blob/resume/cache read contract now have focused local evidence.
  Publication, domain/ownership enforcement, initial signing root, real GHCR
  package acceptance and the wider SDK regression gates remain required.
  Native publication preparation now validates `domains.acl`, explicit source
  location, included LICENSE and complete source/archive fingerprint equality,
  and generates the downloader's OCI manifest contract. Actual reviewer
  authority, signed catalog publication and remote installation remain open.
  Native AIX construction now supports `registry package build`: reuse the
  existing bounded source inventory, preserve package hashes and Unix modes,
  fix ZIP ordering/time/compression, re-inspect the archive, and publish without
  replacing an existing output. Eight focused SDK cases, three CLI cases and
  eight isolated actual CLI/ACP build/check/prepare cases pass. The wider
  extension suite still has 57 known failing cases, each replayed alone; all
  remain required. This synthetic proof does not produce any of the 14 official
  dependency artifacts.
  The tool owner now has a bounded typed external-archive ACL parser, native
  full-tree installation, explicit publisher-declared command modes and an
  offline byte-install path. Its current native tests and Clippy gate, CUA's
  real producer archive execution and the desktop lifecycle adapter remain
  required. The source-owned CUA manifest has six CLI rows; downloaded macOS
  arm64 bytes and all four archive members match the producer's exact digest.
  Desktop follow-up additionally requires an account-free Plugins ActivityBar
  entry, retirement of the Aster dependency-settings entry and deep links, native
  official Registry discovery, install/update/remove/status/progress controls,
  and conversion of all 14 source-owned dependency/runtime items into AIX.
  Local navigation, catalogue and installation-adapter changes are in progress;
  native Tool artifact references, component adoption, all 14 package artifacts,
  initial trust-root custody, signed remote publication and native end-to-end
  installation remain acceptance requirements. The inventory and current limits
  are recorded in `docs/trusted-plugins.md`.
  Installed tool versions now come from verified toolchain receipts and remain
  separate from the catalogue's package version. Damaged installations suppress
  the tool version until repaired. Six native lifecycle cases/three ignored,
  13 Registry cases, 42 renderer cases and Studio Clippy pass. The broader client
  suite retains its unrelated ModelScope fixture failure; global typecheck and
  Oxlint remain failing outside the changed files. Native GUI acceptance and
  the rest of the dependency-page capabilities remain required.
  The 2026-11-02 native follow-up passes nine development-bundle, eight runtime
  and three account-free loopback cases. Its isolated desktop confirms Office
  install/remove and search recovery, but temporary market-load errors after
  installation changes keep GUI acceptance failed. Four official records do
  not complete the original inventory migration or domain-directory layout.
  See `docs/evidence/desktop/2026-11-02-plugins-native/OUTCOME.md`.
  A subsequent SDK regression reproduces immediate metadata-lock contention.
  The shared acquisition now supports bounded asynchronous waiting and cancellation;
  its two new cases pass, alongside nine other remote cases. Six unchanged
  permission-digest fixture failures persist and reproduce individually.
  Eight native runtime cases and the desktop build now pass. The fresh native
  GUI replay shows four records and no captured market-load error, but remains
  failed because of unexplained Work navigation after a refused stale-target
  action. The paired CLI/ACP build returned 101 after ACP interface errors;
  the latest core sources contain those interfaces and require stable build
  verification. The subsequent CLI environment suite passes 30 cases and its
  format step passes. Known domain labels localize without changing registry filter
  IDs. The four current source packages now use operations/publisher/package
  directories. Native preparation, CLI and producers pass 10, three and six
  respective cases; package and published-content identities are unchanged.
  CUA now has maintained operations-domain source and native catalog input;
  its exact new AIX passes build, check, preparation and actual macOS ARM64
  desktop install/query/version/remove/final-lease retirement. CUA remains
  unpublished; GUI/VM and other-target execution are not accepted. Native
  publication now preserves UI/Agent/Flow/OKF and native Tool graphs with
  executable planning and permission bindings. Twelve SDK cases, three CLI
  cases and both owning Clippy checks pass. Review 1.3.0 now has maintained
  knowledge-domain source and native catalog input; its actual AIX build,
  inspection and preparation pass. Review remains unpublished, and its host
  workflow/GUI installation is not qualified. The latest root-format check
  encountered another session's Studio merge conflict and passed after that
  session resolved the markers; focused changed-source formatting passes.
  Source landing and the remaining package inventory are
  still required.
  Seven more maintained toolchains now have one schema-v2 source package each
  and five explicit platform archives. Their actual seven builds, seven AIX
  inspections and 35 catalog/OCI preparations pass; all 48 source members retain
  their exact archive bytes. The tool owner passes 136 cases (five ignored),
  Clippy and formatting; eight repository cases pass. A separate macOS ARM64
  case installs all seven actual external archives, executes eight version
  commands, removes them and repeats from the verified offline cache. Sources
  remain unpublished. Other native platforms, desktop installation, ACP
  consumer integration and full backend replacement remain required. See
  `multiplatform-toolchain-results.json` in the same evidence directory.
  Runtime preparation now joins the same native job, preserves phase/cache-byte
  progress and can reopen from the Plugins toolbar. Nine native cases, three
  cache-observation cases, 32 final renderer cases and the desktop build pass.
  A partial real GUI replay confirms the account-free catalog and keyboard
  reopening/focus return, then aborts after three invalid CUA snapshots. Actual
  in-flight download GUI acceptance remains open. Studio Clippy's first queued
  attempt fails in an unrelated path assertion; that assertion was corrected
  in the checkout and the unchanged command passes on replay;
  no release or update to the running primary app is claimed. See
  `runtime-progress-results.json` in the same evidence directory.
  Box now has maintained operations-domain source and a digest-pinned embedded
  runtime recipe. Native build/check/catalog preparation, all seven source/archive
  members and retained desktop-owner install/query/version/remove/final-lease
  retirement pass on macOS ARM64. Twelve registry-repository tests pass. The
  source inventory is 16 units with four local catalog records; this increment
  publishes none. VM execution, macOS corresponding-source qualification, GUI
  acceptance, other hosts and the remaining original inventory stay required.
  See `box-maintained-source-results.json`; stale-builder and unsorted-metadata
  failures are retained with their isolated replays.
- [ ] T009 [US4] Implement bootstrap and host-package/state adapters in their
  owning library modules; test plans, idempotence, refusal, and native isolated
  apply for packages/accounts/files/services/firewall/Compose/repos/remotes.
- [ ] T010 [US4] Implement dotfiles/history/sync/encryption and daemon lifecycle
  with existing filesystem/process adapters; test actual restoration, template
  output, health readiness, cleanup, and platform refusals.

## Phase 2: Wiring

- [ ] T011 [US1–US4] Wire CLI command discovery, diagnostics, completion, tool
  docs/skills, policy controls, and MCP environment/task access; add isolated
  `apps/aster/tests/` coverage and preserve existing namespaces and permissions.
- [ ] T012 [US1–US4] Build the final `aster`/`aster-acp` pair and run every
  upstream-inventory witness with mise absent. Complete native platform gates
  and the existing dependency/format/lint/test checks. Do not erase failures.
  Final binary notices must include the retained mise MIT notice for the
  adapted version algorithms; validate the owning archive contract.
  The default `aster use` entry now embeds the existing local Rust SDK rather
  than requiring an unshipped `aster-use` binary. The explicit executable
  override remains compatible. Six SDK checks, all 20 isolated actual CLI
  cases and a copied-CLI native AIX check pass. The original parallel timeout
  failure is retained. Current runtime-pool integration checks, final release
  archives and native platform gates remain open.

## Phase 3: Evidence

- [ ] T013 [US1–US4] Record commands, results, counts, baseline references, and
  limitations in `docs/evidence/cli/2026-11-01-mise-replacement/OUTCOME.md`.
- [ ] T014 [US1–US4] Verify effective behavior and profile fixed cold/warm
  installation, selection, execution and task/cache workloads in release mode.
  Retain latency distributions, resource use, network requests and child-process
  counts. Remove measured redundant work and material scaling regressions;
  rerun the same inputs after each fix. Test/debug timings alone do not close
  FR-012 or the final efficiency gate.

## Final Verification Gate

- [ ] Every frozen capability and FR-001–FR-013 has behavioral evidence.
- [ ] Owning-package focused tests pass.
- [ ] Required package Clippy checks pass with supported features.
- [ ] `cargo fmt --all -- --check` is clean for Rust edits.
- [ ] `python3 tests/workspace_dependencies.py` passes; dependencies stay local.
- [ ] No native `<select>` is added on a product surface.
- [ ] Product configuration is ACL parsed by `aster-acl`.
- [ ] CLI/ACP pair passes native platform acceptance without mise.
- [ ] Release-profile cold/warm efficiency evidence satisfies FR-012/SC-004.
- [ ] Full completion audit finds no missing, deferred, or unverified capability.
