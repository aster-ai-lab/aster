# Aster CLI / mise replacement progress

Full replacement is **incomplete**. The frozen reference is mise
`v2026.10.25`, commit `3c78bbb2d58c0b668fe8d5bebf4a8cb66194e1aa`.
The 690-path baseline and capability matrix are under
`specs/031-cli-mise-replacement/contracts/`. The upstream executable is an
isolated test reference only; Aster does not delegate to it.

## Implemented behavior

- Blocking HTTP client construction and destruction occur off Tokio executor
  threads. Failed installation returns a recoverable error and preserves pins.
- Bounded ACL reads merge user, ancestor and nearest project layers. Invalid
  declarations refuse before writes; atomic writes reuse the existing native
  filesystem adapter, including permissions and symlink destinations.
- Per-version receipts select project installations independently of the
  global current pointer. Real fixture commands prove version isolation.
- Unix exec preserves native arguments, exit status and signal termination.
  Windows uses the existing process-job adapter and inherited console group.
  Secret-bearing command/config Debug implementations redact values; output
  errors return instead of panicking.
- Project locks use ACL, source-declared file manifests, exact requests and
  OS/architecture/ABI. Lock generation checks all declared files; locked
  acquisition verifies bytes before publication; offline mode never fetches.
  Ancestor lock discovery supports nested directories. Legacy installs lacking
  manifests must be reinstalled before lock capture.
- Version requests support bare/exact labels, explicit prefixes, provider
  aliases and subtraction. Bare queries prefer an exact version; prefixes use
  separator boundaries. GitHub/aqua listings paginate, npm preserves dist-tags,
  and bounded source-identified caches support offline selection. Installed
  latest prefers stable versions and falls back to an installed prerelease.
- V2 locks retain original requests and resolved selectors. Locked execution
  works after deleting version metadata. The reader retains v1 managed-lock
  compatibility. Native path/system selection is implemented in the tool owner
  and passes actual CLI context comparisons.
  Reference execution and path/system lock capture are still unsupported.
- Source-aware ACL merging retains each tool's declaring workspace or user
  home. External selections use native executable lookup and bounded default
  layouts; system selections exclude the store's emitted shim/install roots.
  They create no managed receipts and need no HTTP client. Four new library
  cases include actual command execution and filesystem effects.
- V1 lock requests now retain their historical literal semantics, including
  `system`, reference-like labels and empty legacy reference suffixes. Parsing,
  rendering, declaration resolution, verified download and offline reuse keep
  the exact pin. These locks retain their v1 representation instead of silently
  adopting current request operators. Catalog identity resolution is shared;
  v2/v3 requests retain their current refusal and selector contracts.
- The existing SDK `acquire_with` retains exact-pin behavior, including reserved
  literal labels. CLI request resolution uses the new `acquire_requested_with`
  entry point. HTTP/config Debug redacts endpoint credentials as well as tokens.
- HTTP metadata and archive bodies share the existing 200 MiB byte bound,
  including unknown-length bodies. Typed HTTP/status, read and JSON parser
  errors retain their sources; request URLs are stripped from HTTP errors.
- A default-compatible source method can now provide a complete checksum-
  verified tar.gz/ZIP installation. The store retains its native directories,
  regular files, internal links, file modes and declared command entrypoints.
  Per-version archive records identify PATH directories and executable paths;
  CLI exec/which and uninstall use them. Existing single-file SDK callers keep
  their payload and receipt shapes.
- Archive extraction preflights tar extension metadata, bounds entries/expanded
  bytes and refuses escapes, duplicate paths and invalid link graphs before
  publication. Current receipt reads validate identity before deriving archive
  record paths. Executable/companion payload bytes are omitted from Debug,
  including containing sources and installation payloads.
- The official `core:node` prebuilt provider verifies detached OpenPGP signatures
  with independent `pgp 0.21.0`, verifies the signed archive checksum and retains
  complete Node/npm layouts with optional npx/corepack entrypoints. Public keys
  and signed historical regression bytes are pinned in docs/source-manifest.json.
  Actual Node 20.11.0 stdlib filesystem/hash effects and npm execution passed.
  Metadata includes prereleases and LTS aliases. Complete SemVer pins reuse an
  identified installation without metadata lookup or distribution download and
  republish its current pointer/shims when switching versions. Latest still
  refreshes metadata. The standalone SDK's exact-fetch contract is preserved.
- V3 ACL locks bind archive SHA-256 and the deterministic complete v1 archive
  record digest. Capture/offline reuse check recorded file bytes/modes, links
  and membership against disk. Locked download validates bytes and layout
  before publication. Existing v1/v2 file locks remain readable.
- The loopback fixture constructor is gated behind unit tests or the opt-in
  `test-support` feature. Studio enables it through its dev-dependency only.
  Studio's existing source wrapper now forwards native installation payloads
  and version metadata rather than flattening them through the old fetch API.
- Native AIX inspection reuses the installer archive reader, schema-v3 ACL,
  declared file inventory and surface-file checks without installation or
  execution. `aster-use registry package check` exposes it to source authors.
  Shared catalog admission now accepts `.aix` alongside supported legacy formats.
- The official GHCR read contract uses an explicit domain and OCI manifest
  SHA-256 in the signed target name. Existing catalog digests and package locks
  seal that identity plus AIX blob length/SHA-256. The reader checks one native
  layer, artifact/config media types, filename and repository annotation, then
  uses the existing verified resume/cache transaction. It bounds both declared-
  and unknown-length metadata, redacts authorization, strips URL-bearing HTTP
  errors and omits authorization on cross-origin storage redirects. Altered blob
  bytes are discarded before publication; a clean retry succeeds.
- Offline TUF cache reads now convert file URLs through native paths. The
  previous Tough file adapter treated percent-escaped directory text as a path,
  breaking real caches under spaces/Unicode names. Native I/O errors and bounds
  remain typed. This host rejects non-UTF-8 filenames; Linux-only valid-name
  coverage remains required alongside native platform acceptance.
- Native publication preparation now checks an explicit bounded `domains.acl`
  policy, admitted publisher and native domain source path, an included LICENSE
  and complete source/archive fingerprint equality. The existing SDK CLI can
  write exact OCI manifest bytes and report both manifest/archive identities;
  the encoder and downloader share the OCI data structures. Existing output
  files are preserved and output within the plugin source refuses. Six owning
  library tests and five OCI protocol tests pass with the production encoder.
  Two SDK CLI tests, the SDK build and owning Clippy also pass. Actual Aster CLI
  preparation and both refusal cases pass against a fixed native 14-file,
  6262-byte archive. Manifest and source bytes were reopened and compared.
- `aster use` now calls the existing local Rust SDK by default. An explicit
  `ASTER_USE_BIN` still forwards native arguments and exit status. This removes
  the former default dependency on an unshipped third binary; the CLI/ACP
  archive protocol is unchanged. SDK parsing/output tests pass six cases.
  All 20 actual CLI cases pass when replayed individually with the same compiled
  test and CLI binaries after the first parallel run timed out. A copied CLI
  with no sibling SDK and a system-only PATH also checks the fixed 14-file AIX.
  Current runtime-pool changes, paired build and owning Aster Clippy remain
  under validation; these results do not certify a final release archive.
- Exact `tool uninstall name@version` now has an owning store API and CLI
  wiring. It retains other versions and clears the removed current version's
  declared shims and pointer without choosing a fallback. Archive tests execute
  both surviving command entrypoints after removing another version. Optional
  `--unpin` reads and admits the ACL before deleting payloads and preserves a
  different literal version pin and environment values. The owning library
  passes 111 cases with two ignored live-network cases; tool Clippy and seven
  dependency-locality cases pass. Native CLI acceptance remains pending.
- Eight new fixed-binary uninstall cases now distinguish physical installations
  from mise's compatibility symlinks and distinguish bare fragments from
  explicit prefix operators. The latter exposed an incorrect assumption in the
  initial implementation and collector: an explicit prefix resolves one provider
  version. The owner now uses source-identified metadata and preserves older
  installations when that resolved version is absent. Its suite passes 114
  cases with two ignored network cases, and Clippy passes. The actual CLI
  comparison reads the new frozen cases and remains under validation.

## Verification

Host: Darwin / arm64, pinned Rust 1.97.1, default package features. Tests use
portable isolated temporary directories, including spaces and non-ASCII names.

| Command / behavior | Result | Evidence |
| --- | --- | --- |
| `cargo build --locked -p aster -p aster-acp` after HTTP changes, before final archive wiring | Pass | build-versions-final.log |
| `cargo test --locked -p aster --test env_manager` with native archive execution | 14 pass | cli-archive-tests.log |
| `cargo test --locked -p aster-pager env_manager_cmd::tests --lib` before HTTP error changes | 12 pass | pager-version-tests.log |
| Isolated actual CLI smoke checks before lock work | 10 pass | cli-smoke.json |
| `CARGO_TARGET_DIR=target/mise-replacement-checks cargo test --locked -p aster-use-tool-acquire` with archive and receipt admission | 91 pass, 1 live-network test ignored | archive-tool-suite-admission.log |
| Actual loopback status/transport, truncated-body, malformed-JSON and size-limit checks | 4 pass | http-error-tests-fixed.log |
| Owning tool-library Clippy, all targets, no dependencies, with archive and receipt admission | Pass, no package warnings | archive-clippy-admission.log |
| Owning library after native archive locks | 102 pass, 2 live-network tests ignored | archive-lock-tool-tests.log |
| Owning library after exact Node reuse and pointer switching | 102 pass, 2 ignored | node-exact-reuse-tests.log |
| Genuine signed Node 20.11.0 install, native stdlib effects and npm | 1 pass | node-live-native-fixed.log |
| Same Node 20.11.0 workload after exact-pin optimization | 1 pass; 1 distribution fetch, 0 metadata queries; debug cold 51233 ms / warm 105 ms | node-live-native-exact-reuse.log |
| Owning tool-library Clippy after Node/archive-lock/test-feature changes | Pass, no package warnings | archive-lock-clippy.log |
| Studio native source-protocol regression, default tenant | 1 pass; 1455 filtered out | studio-node-protocol-tests.log |
| Dependency-locality check after PGP dependency resolution | 7 pass | node-workspace-dependencies.log |
| Focused Rust formatting after Node/archive locks | Pass | node-focused-format.log |
| Workspace formatting after Node/archive locks | Fail: 782 distinct paths; no mass-format performed | node-root-format.log |
| Actual CLI archive-lock suite after Node | Fail: 1 pass, 13 fixed-deadline child-startup timeouts | archive-lock-cli-tests.log |
| Each of those 13 failures rerun alone with the same compiled test binary and unchanged input/deadlines | 13 pass | archive-lock-cli-isolated.log, archive-lock-cli-isolated.json |
| `cargo build --locked -p aster -p aster-acp` after Node/archive-lock implementation | Pass; predates the AIX/GHCR slice | build-node-lock.log |
| AIX native format, source membership and refusal cases | 4 pass | plugin-aix-library-tests-native-case.log |
| OCI signed identity, lock retention, auth redirects/resume, bounds, blob integrity/retry and cached/offline package preparation | 5 pass | plugin-ghcr-integrity-tests.log |
| Native cached-metadata URL decoding, I/O-source preservation and bounds | 3 pass on macOS | plugin-native-cache-transport-tests.log |
| Public core native-AIX catalog contract and foreign-format refusal | 1 pass | plugin-native-aix-contract.log |
| SDK CLI package-check command with a real native archive | 1 pass | plugin-aix-cli-tests-native-manifest.log |
| `cargo build --locked -p aster-use` with native AIX/GHCR support | Pass | build-plugin-source-cli.log |
| Actual `aster use registry package check <file.aix> --json`, isolated state, native companion discovery and mise absent | Pass: 13 source files / 6216 bytes match; debug cold 13092 ms within the unchanged 15 s deadline | plugin-aix-cli-smoke.json |
| Owning use/core/extension Clippy, all targets, no dependencies | Pass; one existing too-many-arguments warning in plugin_manifest.rs | plugin-source-clippy-final.log |
| Dependency-locality check after AIX/GHCR changes | 7 pass | plugin-workspace-dependencies.log |
| Focused Rust formatting after AIX/GHCR changes | Pass | plugin-focused-format-final.log |
| Domain source/publication preparation, bounds and refusal behavior | 6 pass | plugin-publication-tests.log |
| Existing signed OCI read/cache/resume/integrity protocol with the production publication encoder | 5 pass | plugin-publication-oci-reader-tests.log |
| SDK CLI native AIX check and publication preparation/output-preservation cases | 2 pass | plugin-publication-cli-tests.log |
| Native publication SDK build | Pass | plugin-publication-build.log |
| Owning SDK/extension publication Clippy | Pass; existing dependency unused import and extension too-many-arguments warnings retained | plugin-publication-clippy.log |
| Actual isolated `aster use registry package prepare`, source/digest agreement, existing-output and source-drift refusal | 3 pass; debug cold 2213 ms within unchanged 15 s deadline | plugin-publication-cli-smoke.json, plugin-publication-cli-smoke.log |
| Focused Rust formatting for native publication preparation | Pass | plugin-publication-format-final.log |
| Dependency-locality check after publication preparation | 7 pass | plugin-publication-dependencies.log |
| Workspace formatting after publication preparation | Fail: 779 distinct paths; no new publication path appears in the diagnostics | plugin-publication-workspace-format.log |
| New literal-v1 regression before the repair, run alone | Expected failure: one case rejected the legacy literal as a current context request | legacy-literal-lock-before.log |
| Complete tool library after literal-v1 repair | 103 pass, 2 ignored | legacy-literal-tool-suite.log |
| Tool-library Clippy, all targets, after literal-v1 repair | Pass, no package warnings | legacy-literal-tool-clippy-final.log |
| Final focused lock suite after removing a duplicate identity check and refreshing the dependency graph | 11 pass | legacy-literal-lock-tests-refreshed.log |
| Dependency-locality check after literal-v1 repair | 7 pass | legacy-literal-dependencies.log |
| Focused literal-v1 Rust formatting | Pass | legacy-literal-tool-format-final.log, legacy-literal-cli-format-final.log |
| Workspace formatting after literal-v1 repair | Fail: 779 distinct paths; none of the four changed literal-v1 paths appears | legacy-literal-workspace-format.log |
| Seven additional pinned native context reference cases, unchanged 25 s deadline | Captured: six success cases and one missing-path refusal | mise-context-reference-fixed.log; contracts/mise-context-reference.json |
| Focused native path/system library cases | 4 pass | context-library-tests.log |
| Complete tool library after native context selection | 107 pass, 2 ignored | context-tool-suite.log |
| Tool-library Clippy, all targets, after context selection | Pass, no package warnings | context-tool-clippy.log |
| Context dependency-locality and owning-edition focused formatting | 7 dependency cases pass; formatting passes | context-dependencies.log, context-tool-format.log, context-cli-format.log |
| Pager command unit suite after native context wiring | 13 pass, exit 0 | context-pager-tests.log |
| CLI/ACP paired build after native context wiring | Pass, exit 0; existing dependency/ACP/linker warnings retained | context-paired-build.log |
| First CLI context suite after building | Fail: all 17 cases hit unchanged 20 s per-command deadlines | context-cli-tests.log |
| Every failed CLI case replayed alone with the same test and CLI binaries | 17 pass; CLI digest unchanged across every case | context-cli-isolated.log, context-cli-isolated.json |
| Same compiled CLI suite rerun with its original parallel runner and deadlines | 17 pass in 58.20 s | context-cli-parallel-replay.log |
| Temporary override behavior before CLI integration | Expected failure: old CLI cannot execute the new --tool request | context-override-before.log |
| Temporary override library regression | 1 pass, actual native commands and file effects; declarations unchanged | context-override-library-test.log |
| Complete tool library after temporary overrides, plus owning Clippy | 108 pass, 2 ignored; Clippy passes | context-override-tool-suite.log, context-override-tool-clippy.log |
| Temporary override owning-edition focused formatting | Pass | context-override-tool-format.log, context-override-cli-format.log |
| Owning CLI/pager Clippy after context wiring | Fail, exit 101: 42 diagnostics in 9 other paths; primary starts/messages/multiplicity match the retained earlier run | context-cli-clippy.log, context-cli-clippy-findings.json |
| Current workspace formatting after temporary overrides | Fail: 781 distinct paths; none of the five changed context paths appears | context-override-workspace-format.log |
| Same native --tool probe after CLI integration, fresh state and unchanged command/deadline | Pass: prints override-probe | context-override-after.log |
| Updated actual CLI suite, including temporary overrides and direct argument/reference comparisons | 18 pass in 36.90 s, unchanged per-command deadlines | context-override-cli-tests.log |
| Fresh paired CLI/ACP build after temporary override integration | Pass, exit 0; existing warnings retained | context-override-paired-build.log |
| Updated override pager tests | 15 pass, exit 0 | context-override-pager-tests.log |
| Five native fresh/repeated-state path-selection pairs per binary | Debug Aster medians 573.354 / 152.608 ms; optimized reference 443.656 / 16.361 ms; diagnostic only, differing profiles and uncontrolled host load/caches | context-selection-benchmark.json |
| Early build-ID/version startup diagnostic | Debug medians 10.685 / 11.910 ms; does not attribute the full exec cost | context-startup-stage-benchmark.json |
| Environment command startup side effects before optimization | Actual isolated exec writes 27 agent guide files | context-startup-side-effects-before.json |
| Startup optimization focused Rust formatting, including main | Pass | context-startup-main-format.log, context-startup-tests-format.log |
| Startup CLI checks before SDK embedding | 18 pass in 30.93 s | context-startup-cli-tests.log |
| Startup debug CLI/ACP pair, owning Aster Clippy and standard release pair | Pass; predates SDK embedding | context-startup-paired-build.log, context-startup-aster-clippy.log, context-release-paired-build.log |
| Native path-selection release benchmark, five fresh/repeated pairs per binary | Aster median 627.316 / 210.642 ms; mise 393.927 / 15.197 ms; efficiency not accepted | context-selection-benchmark-release.json |
| Shared SDK CLI parsing/output checks after embedding and host help | 6 pass; owning SDK Clippy passes | embedded-use-sdk-tests-final.log, embedded-use-sdk-clippy-final.log |
| Initial embedded CLI parallel suite | Fail: all 20 cases hit unchanged 20 s command deadlines | embedded-use-cli-tests.log |
| Each failed embedded CLI case replayed alone with the same compiled test binary | 20 pass; one CLI digest across all cases | embedded-use-cli-isolated.json, embedded-use-cli-isolated.log |
| Copied CLI without sibling SDK, system-only PATH, fixed native AIX check | Pass: 14 files / 6262 bytes, 6422.760 ms, archive unchanged | embedded-use-aix-smoke.json |
| Embedded paired build queued before the new SDK entrypoint existed | Fail: E0425, old SDK artifact lacks `run_embedded_os` | embedded-use-paired-build.log |
| Embedded Aster Clippy while the new uninstall API/caller were being edited | Fail: E0599, old tool-library artifact lacks `remove_version_sync` | embedded-use-aster-clippy.log |
| Tool library after exact-version removal and archive entrypoint cleanup | 110 pass, 2 ignored; Clippy passes | uninstall-version-tool-suite.log, uninstall-version-tool-clippy.log |
| Damaged-payload uninstall regression, same isolated input before/after receipt-reader fix | Before: 1 fail; after: 1 pass | uninstall-version-missing-payload-before.log, uninstall-version-missing-payload-after.log |
| Complete tool library after damaged-payload uninstall repair | 111 pass, 2 ignored; Clippy and focused formatting pass | uninstall-version-tool-suite-final.log, uninstall-version-tool-clippy-final.log, uninstall-version-tool-format-final.log |
| Eight additional pinned native uninstall cases, complete physical membership and surviving executable checks | Pass, assembled only from passed fresh-state cases; initial failures retained | contracts/mise-uninstall-reference.json, case_provenance |
| Tool library after provider-resolved uninstall correction | 114 pass, 2 ignored; Clippy passes | uninstall-provider-selection-tool-suite.log, uninstall-provider-selection-tool-clippy.log |
| Earlier parallel CLI suite after SDK/runtime integration | Fail: 3 pass, 20 fail in 40.03 s | embedded-use-cli-tests-final.log |
| Every one of those 20 failures replayed alone with the same compiled test binary | 8 pass, 12 fail; the paired build changed the CLI during one case | uninstall-selection-cli-isolated.json, uninstall-selection-cli-isolated.log |
| Refreshed paired build and native build-ID/help checks before provider correction | Pass; pair IDs match and new flags are present | uninstall-version-paired-build.log, uninstall-selection-current-cli-probe.json |
| Owning Aster Clippy before provider correction | Pass; final comparative test/provider correction still require a fresh check | uninstall-selection-aster-clippy.log |
| Dependency locality and focused formatting after exact-version removal | 7 dependency cases pass; formatting passes | uninstall-version-dependencies.log, uninstall-version-tool-format.log, uninstall-version-cli-format.log |
| Workspace formatting after exact-version removal | Fail: 13 other paths; no changed uninstall path appears | uninstall-version-workspace-format.log |
| Current native CLI regression and refreshed CLI/ACP pair | Pending: original CLI process and fresh pair process retained; fresh owning Aster/pager Clippy still required | embedded-use-cli-tests-final.log, uninstall-version-paired-build.log |
| Complete extension library suite before test-precondition fixes | Fail: 138 pass, 59 fail, 1 ignored | plugin-extension-suite.log |
| Every extension-suite failure rerun alone using the same binary | 59 fail; retained original precondition/digest errors | plugin-extension-isolated.log, plugin-extension-isolated.json |
| Complete shared catalog-contract suite | Fail: 4 pass, 18 fail | plugin-catalog-contracts.log |
| Every catalog failure rerun alone after replacing the new test's invalid prerequisite with a fresh native record | New native-AIX case passes; 17 older cases fail | plugin-catalog-isolated.log, plugin-catalog-isolated.json |
| Required CLI/pager Clippy, all targets, no dependencies, after archive wiring | Fail: 42 distinct indexing/slicing spans in 9 other files | archive-cli-clippy.jsonl, archive-cli-clippy-findings.json |
| `python3.12 tests/workspace_dependencies.py` | 7 pass | version-workspace-dependencies.log |
| Focused Rust formatting before archive work | Pass | http-focused-format.log, version-cli-focused-format.log |
| `cargo fmt --all -- --check` before archive work | Fail: 785 paths outside this slice | version-format-final.log |
| Upstream reference digest and four isolated help/version invocations | Captured | mise-cli-reference.json in spec contracts |
| `python3.12 tests/cli_mise_version_reference.py --mise .tmp/mise-replacement/reference/mise` | 10 native, fresh-state execution results; all agree with resolver and CLI tests | mise-version-reference.log; contracts/mise-version-reference.json |

The ten reference cases use eleven fixed input versions and a local plugin that
only lists them and installs an executable echoing `ASDF_INSTALL_VERSION`.
The frozen official binary does the selection. No fixture code reproduces the
resolver, changes the baseline, or delegates Aster execution to mise. Its raw
binary SHA-256 is
`8a3ef00765b009278197aba627ac00d6e9c63f6aeb04d4f4b2cfe0e59ddc2fd6`.
The positive and refusal cases are retained together.

The archive-phase paired build passed in build-archive.log, pager tests passed
12 cases in pager-archive-tests.log and focused formatting passed in
archive-focused-format.log. Those results predate Node. The paired Node build
completed successfully in build-node-lock.log. The CLI archive-lock suite
ended with 13 actual child-startup timeouts; every failed case then passed alone
using the same compiled binary and unchanged deadlines. The full-suite failure
is retained and is not relabeled as a pass. No observation timeout is treated
as a terminal result.
Some native test children temporarily remained at startup with 32 KiB RSS;
later the owning suite and live Node workload completed successfully. The single
debug cold/warm measurements include host effects and are not a release-profile
latency distribution or an FR-012 completion claim.

The user added the official domain-organized `aster-ai-lab/plugins` AIX source
and associated GHCR distribution requirement. The read contract is documented
in docs/trusted-plugins.md. No repository, root key, workflow or GHCR artifact
was published by this task. Initial signing-root choice, source activation,
publication, actual remote artifact installation and final packaged CLI access
remain required; loopback protocol tests do not satisfy those gates.
At the user's request, GitHub CLI OAuth device authorization added package and
workflow access. The stored credential now reports `write:packages` and
`workflow`, and the formerly refused organization Container Packages query
succeeds across all pages with zero accessible packages. Repository push/admin
permissions were also verified. No package was pushed to test write access;
see github-package-permissions.json. Credentials and authorization codes are
absent from that evidence.
The earlier actual debug CLI also found its sibling `aster-use` companion and validated
a freshly packed AIX without an executable override or mise in PATH. Every ZIP
file was reopened and compared with the native source tree; the reported file
count and byte count match. This inspection created no installed plugin state.
Its single cold sample is not release-profile efficiency acceptance. The
subsequent native SDK embedding removes the default companion requirement:
an isolated copy of the CLI checks the fixed publication archive with no SDK
sibling or SDK in PATH. The new shared renderer preserves the standalone SDK's
JSON envelope, human errors and exit status. SDK operands still require UTF-8;
native invalid-byte path admission remains open. The explicit subprocess
override retains native arguments. See embedded-use-validation-summary.json.

During isolated replay, one test process was sampled at `_dyld_start` before
entering its test code. Its signature verified and the replay later passed.
This observation does not establish the cause of the original timeouts.
The latest runtime change gives environment commands one default async worker
and prewarms one blocking worker while retaining the existing capacity and
explicit thread override. Its current checks remain pending, and no measured
speedup or release efficiency acceptance is claimed.

The earlier pair failed with E0425 because its already compiled SDK predates
the subsequently added embedding entrypoint. The earlier Clippy failed with
E0599 after this task edited the new uninstall API/caller while the check was
queued/running. Neither is a successful current-source check. The original
native CLI process remains live and the pair has one fresh retry; current Rust
public APIs remain unchanged during the subsequent private receipt-reader
repair. New native uninstall cases cover
retained-version execution, pin preservation and malformed-ACL refusal before
deletion. See uninstall-version-validation-summary.json. The library evidence
does not substitute for these pending CLI cases or native release acceptance.

The refreshed pair later completed successfully. The earlier native suite ended
with three passes and 20 failures. Every failure was then replayed individually:
eight passed and 12 failed; the CLI artifact changed during one replay as the
pair finished, so this is not a uniform-artifact validation pass. A sampled
child was still at `_dyld_start` before Rust main with a 112 KiB footprint.
That observation does not explain every failure or prove a root cause.

The new native uninstall collector initially counted a compatibility symlink as
an installed short-label payload and also expected an explicit prefix to be
ambiguous. Raw pinned-binary output and entry-kind observations disproved both
assumptions. Original failed captures remain intact; the completed eight-case
contract contains only individually passed cases and their source digests.
The original ten version cases, eleven input versions and seven context cases
were not changed. Aster's corrected provider selection and its new eight-case
CLI comparison require current-source acceptance. See
uninstall-provider-selection-validation-summary.json.

A further single-case regression reproduced stale records when the requested
version's directory or main executable is missing. Removal now reuses the
validated version receipt independently of executable completeness; execution
still requires the complete installation. The same isolated case passes after
the fix. Full late-I/O rollback remains open; this is not a transactional
uninstall acceptance claim.

## Retained failures and coverage limits

- Root `cargo fmt --all -- --check` fails on existing unrelated files. Initial
  capture found 790 paths; no broad formatting was performed. See format.log
  and format-final.log. The post-lock check also fails in format-lock.log.
  The post-Node run reports 782 paths; the latest publication-phase run reports
  779, with no new publication path in the diagnostics. No broad formatting was
  performed.
  The final workspace format gate remains open.
- The initial required CLI/pager Clippy run failed on 42 indexing/slicing lint
  diagnostics in unchanged paths. Exact locations are in
  clippy-gate-findings.json. These are lint findings, not automatically proved
  panic bugs; the latest run also reports 42 distinct spans (84 emissions across
  lib/lib-test). Primary spans were read in context: they include JSON indexing
  and UI arrays. A correctness repair requires checking each owning invariant;
  this task did not rewrite those other modules or relax lints.
- Older queued runs used a dependency graph from before the new `versions`
  declaration and ended with E0432. Those failures remain in
  pager-lock-tests.log / cli-lock-clippy.stderr.log. The current locked library,
  pager tests, CLI tests and paired build resolve the declared dependency.
- The first new HTTP error test mistakenly expected a userinfo-bearing URL to
  reach its fixture server. The existing policy refused it before I/O; the test
  hung waiting in accept and was terminated (exit 143, http-error-tests.log).
  The test now uses admitted query/path secrets and a bounded fixture wait.
  Product URL policy and frozen version-reference inputs were not changed.
- Concurrent edits temporarily omitted `aster-core` / `crossterm` direct
  dependencies. Later manifests contain their fixes and the paired build plus
  nine CLI tests passed. Earlier failed logs remain intact.
- No native Windows or Linux runtime evidence exists. Cross targets installed
  on this macOS host do not satisfy native acceptance.
- A dependency-check refresh using system Python failed because `tomllib` is
  missing (archive-workspace-dependencies.log). The prescribed Python 3.12
  archive check completed seven tests in archive-workspace-dependencies-python312.log;
  the subsequent PGP dependency refresh also passed seven tests. Neither script
  nor interpreter behavior was changed to pass the gate.
- The first actual Node run passed native Node/npm effects, then failed on
  warm resolution because the real provider index includes
  `27.0.0-alpha.0.0.0` (node-live-native.log). Admission now preserves full
  SemVer prereleases while stable latest excludes them; the same fixed 20.11.0
  workload passed afterward. Its reference payloads/signatures were not changed.
- Studio's existing read-only CI counterparts to its writing lint command were
  run after `pnpm install --frozen-lockfile` passed. Lint fails in tsgolint on
  two renderer filenames missing from its program; formatting fails on an
  existing evidence file containing non-JSON text; typecheck reports unrelated
  renderer/e2e errors; i18n rejects unsorted en-us keys. See studio-node-pnpm-*
  logs. No broad autofix or catalog rewrite was performed. The full Studio
  lint gate is not claimed.
- All builds use the root pinned toolchain. The tool library now inherits the
  existing workspace Rust 1.97 baseline; it does not claim standalone Rust 1.82
  support. No product toolchain baseline was raised.
- No live tool-download matrix, final archive, sandbox release smoke, complete
  language/backend/plugin matrix, shell restoration, tasks, provisioning,
  dotfiles, daemon or MCP parity has passed. No release or completion is claimed.
- Blocking source requests are bounded by HTTP timeouts but are not canceled
  merely by dropping their async join handle. Cancellation remains a required
  T004/T008 follow-up.
- Online locked downloads still discover URLs through the source API. Locking
  file bytes is not the upstream pre-resolved-URL/no-registry-lookup contract.
- Native archive locks and the Node prebuilt provider now use the archive
  protocol. Complete directory metadata, all source/archive formats, other core
  providers, runtime options and measured per-backend budgets remain required.
  Full rollback of multi-entrypoint shim publication across differing layouts
  and late I/O failures still needs implementation and failure-injection tests.
- V1 literal-name locks preserve their schema and meaning, including nine
  reserved/reference-like labels in the new regression. Unlocked ambiguous
  declarations and regenerating such pins under a newer request grammar remain
  required migration work. Native path/system execution now has library
  evidence and actual CLI context comparisons. Temporary overrides have library
  evidence and actual CLI/pager comparisons plus paired builds. Reference execution,
  path/system locks and full backend layouts remain
  required; accepting an old literal does not implement these modes.
- One queued CLI compilation retained the manifest graph from before the new
  test-support feature was declared and warned about that cfg spelling. The
  subsequent owning Clippy resolves the declared feature without warnings;
  the queued binary check is not substituted for final feature-graph checks.
- The first Studio Clippy request also retained that earlier graph and ended
  with two E0599 fixture-constructor errors. The manifest has the test-only
  feature opt-in. Its refresh ended with status 255 while waiting for the build
  directory and produced no terminal compiler diagnostic. Supported-tenant
  final Studio checks remain open; neither attempt is labeled a pass.
- Initial AIX inspection tests had two setup mistakes: case-colliding names on
  this filesystem and a CLI fixture missing its required repository block.
  Both original logs are retained. The corrected focused cases exercise the
  real validation and pass. A new Unix-only filename test also attempted a name
  this macOS filesystem rejects with EILSEQ; Linux valid-byte-name coverage is
  now explicit, while this host verifies native error preservation.
- The first OCI end-to-end preparation failed on the real escaped-cache-path
  bug (plugin-ghcr-tests-initial.log). The same signed package then passed
  after native file-URL decoding (plugin-ghcr-tests-native-cache.log).
- Wider SDK suites refuse existing fixed fixture digests. For example,
  permission-ceiling-v1.json's stored sidecar declares c30a6414… while the file
  without its final LF hashes to 22fb8169…. Package/OKF fixtures also disagree
  with their declared content evidence. All 59 initial extension failures and
  18 catalog failures were independently replayed. Frozen fixture bytes and
  expected digests were not rewritten. The full regression gates remain open;
  passing focused AIX/OCI tests does not resolve these failures.
- The existing public SDK `UseError` is a serializable, cloneable struct with
  public fields and no error-source field. GHCR failures retain its stable
  error-code contract and redact credential-bearing transport URLs, but cannot
  preserve an arbitrary typed transport/parser source through this boundary.
  A compatible owner-level error-API decision remains required; this source
  change does not silently break public struct construction or claim complete
  Rust error-source compliance. Native metadata transport preserves its I/O
  source before conversion at the existing SDK boundary.
- Publication-preparation checks completed through their original process
  handles after the shared Cargo queue cleared. The native CLI probe uses its
  frozen 14-file/6262-byte input and unchanged deadline. Its single debug cold
  sample is not release-profile efficiency acceptance. No signing root, remote
  repository commit or GHCR upload is produced by preparation.
- A final focused-lock check and the first literal-v1 CLI test attempt were
  refused before test execution because Cargo reported that the lock file
  required an update while --locked was set. Both original failures are retained. A subsequent
  `cargo metadata --locked --offline` succeeded without this task regenerating
  Cargo.lock; the same checks were then retried with --locked still enabled.
- The earlier paired build ended with an ACP `Meta` type mismatch (exit 101).
  The current shared source already contains a correction from another session.
  The earlier literal CLI regression and CLI Clippy processes were intentionally
  interrupted (exit 130 and 255) when context integration changed source and the
  dependency graph. They are superseded by current-source checks, not recorded
  as passes. No process was restarted merely for a timeout or lock wait.
- The first context reference capture executed its seven cases but failed while
  writing the report because the argument namespace was shadowed by a command
  list. The capture script was corrected and rerun with identical case inputs
  and deadlines. The original failure is retained in mise-context-reference.log;
  the successful report and log are separately retained. The frozen ten-case
  version reference and expected fixture digests were not changed.
- `cargo update --workspace --offline` reported zero newly locked packages while
  resolving the existing workspace `which` dependency for the tool owner.
  Cargo.lock also has concurrent edits; unrelated changes are not attributed to
  this task. Native Linux/Windows and release-profile efficiency remain untested.
- The initial 17-case CLI run timed out every case at the existing 20-second
  command deadline. All 17 then passed individually with the original compiled
  test binary and the same CLI SHA-256, followed by all 17 passing under the
  original parallel runner in 58.20 seconds. No input, timeout or expected output
  was changed. The initial timeout cause remains unresolved; a warm rerun is not
  cold-start or release-efficiency acceptance.
- `aster exec --tool TOOL@REQUEST` now replaces temporary selections before path
  inspection, preserves other declaration roots, and never writes configuration.
  Repeated requests for a tool use the last value. Locked requests refuse these
  overrides both through Clap and the direct Rust entry point. The library
  regression and full suite pass, as do all 15 updated pager and 18 actual CLI
  cases and the refreshed paired build.
- An independent read-only audit of four frozen core plugin JSON fixtures found
  that every SHA-256 sidecar differs from the canonical fixture's actual hash.
  All four JSON and sidecar byte sequences equal HEAD. This supports a fixture-identity
  defect for these four inputs, not a correctness claim for all other failing
  extension/OKF cases. No fixture or expected digest was changed, and production
  SHA-256 must not be altered to produce a false expected identity. See
  plugin-frozen-fixture-audit.json.
- The diagnostic path benchmark compares a debug Aster artifact with the pinned
  optimized mise artifact; it does not establish release performance. Both use
  the same external program, five fresh/repeated state pairs and their existing
  deadlines. OS cache state and concurrent host load are uncontrolled. The
  standard release paired build is running before further efficiency claims.
- Environment commands no longer need agent guide extraction or agent HTTP
  prewarm. The pre-change native probe produced 27 guide files. Both operations
  are now skipped for tool/env/exec/activate/use while policy/sandbox admission
  stays in place. Physical no-guide assertions were added to real CLI cases;
  their fresh run and the performance recapture are still pending.
- The retained MIT notice is under crates/use/tool-acquire/licenses/mise-MIT.txt
  and source provenance is in docs/source-manifest.json / docs/migration.md.
  Final binary/archive notice inclusion has not yet been validated (T012).

- The provider-selection CLI follow-up completed with **0/24 passing in the
  parallel batch**, all at existing deadlines. All **24/24 then passed alone**
  with the same compiled test binary and unchanged CLI bytes during each case,
  including the eight frozen native uninstall comparisons. Both outcomes are
  retained in `uninstall-provider-selection-cli-isolated.json` and its source
  log. The parallel failure cause remains unconfirmed; isolated passes do not
  establish parallel stability, cold-start behavior or release efficiency.
  The queued pager Clippy also completed: its 42 errors remain a failed gate.
- Desktop plugin navigation and local official-Registry adapters are drafted in
  the Studio submodule. The relevant seven renderer test files pass **82/82**;
  `cargo check --locked -p aster-studio --lib` passes on macOS arm64 with default
  `tenant-aster` features. Focused formatting and Oxlint pass, ESLint has zero
  errors and six warnings. Web typechecking fails with 305 diagnostics and
  whole-workspace formatting fails in nine paths; neither is reported as clean.
  An initial all-target Clippy run also returned zero. Later checks are blocked
  by seven E0004 matches missing a concurrently added `UsageProtocol::Gemini`
  in other host paths. Current whole-host acceptance is therefore **not clean**.
  The focused native tests first returned 3/4: the new projection test borrowed
  an existing catalog fixture rejected by permission-ceiling validation. Its
  standalone replay also failed. The replacement synthetic input then lacked
  mandatory package digests, likewise replayed alone; those fields were added
  without changing production validators or frozen fixtures. The final test
  input had not executed in that run because of the shared compile errors.
  After the shared Gemini matches were repaired, the same four current native
  registry tests passed. A subsequent tenant-directory correction adds a fifth
  native case and is still being checked. See
  `desktop-plugins-validation.json` for source hashes, exact logs and limits.
- The 14-item Desktop conversion inventory was compared against the owning
  TypeScript array and runtime-card AST, preserving their order. None of those
  14 AIX artifacts has been produced or published by this task. Native external
  tool references, Tool/Flow host support, existing/custom-install migration,
  production signing custody, actual Registry/GHCR publication and remote/native
  GUI acceptance remain required. The UI work is not a complete migration.

## Native AIX construction follow-up

`registry package build` now uses the SDK's bounded package inventory, sorted
ZIP members, fixed timestamps/compression and Unix modes. It re-inspects the
staged archive against the source fingerprint and publishes without replacing
an existing output. Eight focused extension tests and three SDK CLI cases pass.
The final paired CLI/ACP build passed; an isolated copied pair, without an
`aster-use` sibling or mise on PATH, passed eight actual commands including
build/check/prepare, reproducibility and refusal to overwrite. These debug
measurements are not the release efficiency or release-completeness gate.
See `aix-build-validation.json` and `aix-build-native-smoke.json`.

The full extension suite returned 155 pass, 57 fail and one ignored before the
last error-mapping test was added. All 57 failures were replayed individually
with the same compiled executable; all still fail and all names were already
present in the earlier failing baseline. The original macOS EILSEQ filename
test and its failing isolated replay are retained. The corrected test follows
an actual filesystem oracle, preserves native paths and refuses partial or
alternate output. Initial CLI test compilation errors are also retained.

CUA's pinned `cua-sdk-v0.9.0` producer manifest has six explicit CLI artifact
rows. Its 73,285,759-byte macOS arm64 archive matches the exact SHA-256 and
contains four regular files. The producer installer gives present companions
0755 permissions and coordinates an existing daemon during updates. Both raw
manifest bytes and structured provenance are recorded. A strict GitHub SLSA
attestation lookup returned 404; this does not verify or disprove the separate
producer Sigstore blob bundle. Publisher-signature verification remains open.
The separately run pinned native acquisition case now passes: an isolated
temporary tool store retained all four files and exact lengths, applied the
producer-declared 0755 command modes, and executed native `cua --version`
successfully with the expected 0.9.0 result. The store was temporary; no
permanent user installation or CUA VM/daemon operation is claimed.

The acquisition owner now parses typed, bounded external-archive ACL and can
install complete archives through existing HTTP/store code or supplied bytes.
Seven focused acquisition tests pass (one real-artifact case is ignored by
default). The same compiled test executable passed the full 124-case library
inventory: 121 pass and three default ignores. The real CUA case was then run
separately and passed. Clippy and the tenant-path follow-up remain in progress.
The first test process spent time in `_dyld_start` before entering libtest; its
native sample and successful read-only code-signature verification are retained.
This delay is not attributed to an unentered parser or called an efficiency
acceptance result. Desktop
integration, existing/custom-install migration, all 14 actual AIX artifacts,
production signing and remote publication remain required. The full goal and
all 24 frozen feature families remain open.

All remaining tasks and capability children stay required; no grader or
baseline was changed to obtain passes. No worktree, branch, commit, push or
release was created by this task.

## Current objective and conditional release

The user replaced the full mise-replacement goal with: ensure every plugin can
be queried and managed from Aster's plugin page. The earlier 24-family matrix
and unfinished mise tasks above remain historical evidence; they are not the
current goal's acceptance definition.

All 35 existing `apps/use-registry/packages` sources now pass actual native AIX
construction using an isolated copied CLI/ACP pair with matching build IDs.
The original batch passed 30 and failed five. Each of those five failures was
reproduced alone with unchanged inputs, binary and deadline. Complete UI-root
inventory now retains legal notices and non-web files in its content identity,
while browser projection only advertises supported web assets. The existing
strict web-asset API still refuses unknown media. Five UI-inventory tests, one
SDK projection test and both owning all-target Clippy checks pass.

The screenshot's initial failure was reproduced as native HTTP 503: no official
source was configured and the official repository was then empty. Missing-source
discovery now returns an explicit setup state, without the duplicate failure
toast. Account-free Aster plugin views use their local tenant owner; managed
ToYou retains its member-session requirement. Two local-view HTTP tests and six
native registry tests pass. Studio all-target Clippy passes. Seven renderer
files pass 85 tests; web typechecking still has 242 diagnostics, none in the
changed catalogue or page. The isolated native host returned authenticated 200
for the setup state and view catalogue, and unauthenticated 401. GUI interaction
acceptance and replacement of the user's primary app are not claimed.

Concurrent runtime work has since populated `aster-ai-lab/plugins`. A live
native SDK query verifies its three macOS ARM64 runtime records against the
`ASTER_PLUGIN_ROOT_SHA256` repository variable. The public root is
`e1cf8be656c072939721b765eef12a597c668fe9c33cc0de2379370d3879ae9b`.
The 35 existing package IDs plus the 14 dependency-migration IDs are 49 distinct
known packages; 46 are absent from that verified three-record catalogue.
Construction checks do not establish publication readiness. Per-package
licenses, all other package publication, Tool/Flow and external-tool host
lifecycle ownership, installed-package retention and anonymous GHCR installation
remain open. See `plugins-current-progress.json` and the runtime owner's
`docs/evidence/desktop/2026-11-01-aix-runtime/OUTCOME.md`.

The immediate offline query failed because the cache omitted the versioned
metadata names required by TUF consistent snapshots. An unchanged native replay
reproduced that failure. The fix retains exact verified metadata bytes under
those names; signature, expiration and digest enforcement remain enabled.
Its focused regression now passes, including offline pagination and refusal of
damaged signed metadata. Three native metadata-transport tests and owning
all-target Clippy pass. A freshly built, isolated SDK CLI passes the actual
official-source online query and all three offline pages. The first probe driver
mistook the Plugin Manager's per-registry `nextCursors` array for `nextCursor`;
both native commands had succeeded. The corrected report retains those commands
and records the driver mistake; no product input, deadline or validator was weakened.
The real desktop API independently returned all three records and correctly
filtered Office to one record. Its test instance has been stopped.

The user separately authorized ACP, CLI and desktop releases after every
current development task finishes. No new product release, tag or upload has
been created by this task. Source is still changing and is not frozen on a
clean reviewed commit. The CLI publisher and workflow no longer hardcode
0.1.6: source versions determine artifact names and manifests, mismatched tags
and dirty or different source commits refuse, and both installed-archive
sandbox modes remain mandatory before GitHub writes. An intermediate 14-test
assembly suite passed. The first expanded 16-test run had six deadline errors in
three methods; all three methods passed unchanged standalone replays at their
original deadlines. A subsequent complete 16-test run passes at those unchanged
deadlines. All outcomes remain recorded. Two qualification-gate and ten
governance tests pass. Final release archives, native platform gates,
signing/notarization, channels and download verification are still required.

The three current GHCR package settings still report private visibility. Browser
automation and the native browser window are unavailable, so no visibility
change or fresh anonymous installation is claimed. A further native fix routes
runtime materialization proofs to the same pinned SDK registry that installs
them, instead of checking the ordinary plugin-generation store. Five runtime
tests, nine ordinary-plugin callable regressions, six registry regressions and
Studio all-target Clippy pass with unchanged checked source files. The real SDK
fixture verifies the active receipt, exact fixed version and digest, unchanged
generation reuse, refusal of a mismatched pin and refusal after withdrawal.
An initial test compilation used a string accessor on the numeric lifecycle
generation; its E0599 log is retained and the shared source now uses the original
`u64` value. No success label or empty callable list substitutes for that
verified installed receipt and matching package/manifest pin. Remote installation,
GUI operation, the complete remaining plugin inventory and a new final desktop
archive remain unaccepted.

The installed catalogue now reads actual local publications and SDK runtime
receipts before consulting any remote source. It preserves legacy installation
slots and native IDs, including case and additional segments. A missing or
damaged presentation cache falls back to native identity; a withdrawn generation
stays absent even when raw files remain. Cache failures do not authorize an
installation or turn an already committed install into a reported failure.
Required runtime IDs and their existing storage IDs remain protected from removal.
The initial five new fixture cases failed the catalogue's existing sorted-category
contract; all five were replayed unchanged alone before sorting the synthetic
fixture. Two subsequent regression cases reproduced the presentation-cache and
legacy-ID defects individually with the original binary before the fixes.
Original logs and standalone outcomes are retained.

The freshly verified official catalogue has six release records for three package
IDs: each runtime has its original and shortened content-version selector. The
market now prefers the exact release fixed by the host's version, platform, size
and archive/package/manifest digests. A different runtime release remains visible
but unavailable when no matching host pin exists. The new pin regression checks
all of these bindings. No version, signature or digest validator was weakened.

The final focused native run passes 27 cases: six installed-catalogue cases, six
registry cases, six runtime cases and nine ordinary callable regressions. Studio
all-target Clippy exits successfully; its existing warning count remains 306.
Nine frontend files pass 109 cases, including both runtime and opaque legacy
receipts while the market fails. Scoped ESLint, Oxlint and formatting pass. The
case mismatch in the Aster OAuth module import and its two test callers is fixed.
The full web typecheck still reports 241 diagnostics outside the changed paths;
workspace formatting also fails elsewhere. macOS ARM64 with Rust 1.97.1 is the
native test host; Windows and Linux runtime qualification is outstanding.

A fresh `just build-studio aster` completes. An independently launched test fork
uses an isolated source store with an unsupported schema: its market request
returns 503, authenticated Installed returns 200 with an empty list, and the
unauthenticated request returns 401. This proves the installed-list source and
authentication boundary, not a real plugin removal or native GUI acceptance.
The owned fork was deliberately stopped with SIGTERM; its launcher exit code 1
records that cleanup. The user's main window and source configuration were not
changed.

Conditional release remains waiting. The last observed parent head is
`4d5c8dfa32ff2c7a5239c47f6f13795fe65dbb79`; its ACP, CLI, Rust and backend
qualification runs fail. The working source remains unfrozen, with 2291 parent
status entries and 437 Studio status entries at observation. Committed-head
failures include a missing Core reasoning-effort field and the OAuth module case;
local compilation does not replace qualification of a final clean commit. All
three runtime GHCR packages still report private visibility. The other 46 known
package IDs, remote lifecycle, native GUI and final archive/signing/notarization
gates remain outstanding. No ACP, CLI or desktop release was created by this task.

The next goal turn adds a concrete Notion acquisition candidate, not a claim
that all plugins are managed. The actual `ntn@0.23.25` npm artifact contains five
producer-declared native platform binaries. Its 30,645,442-byte tarball was
verified against npm's SHA-512 integrity and pinned as SHA-256
`2e822403b0b52b6f159c44c72085329d42415aebe12b570889f6a1fc45277004`.
Toolchain ACL now admits the official npm HTTPS origin under the existing npm
mirror policy, with the same credential/query/fragment, size, platform and
archive checks. The original `npm:ntn` source identity is preserved. The complete
upstream package and license remain installed; the npm bootstrap is not invoked.
The generic npm backend and pipx virtual-environment ownership are not claimed fixed.

Five native AIX candidates use `aster/ntn`, domain `productivity`, and explicit
producer platform paths. Every candidate passes build, check and OCI preparation;
all five source files are reopened and compared with each final ZIP. The initial
preparation batch lacked the canonical source license, and the second lacked the
canonical source manifest. All five failures in each batch were replayed alone
with unchanged commands and 60-second deadlines. Final preparation materializes
one exact canonical source tree per target, as the existing runtime producer
does. The five AIX SHA-256 values remain identical across the attempts. No SDK
validator or signed source-to-archive binding was relaxed.

The final tool-acquire suite passes 122 cases with four opt-in cases ignored.
The separately invoked real Notion case passes online installation through the
production HTTP transport, receipt query, native command-shim execution
(`ntn 0.23.25`), explicit uninstall, offline reinstall and a second uninstall.
It uses a temporary path with spaces and non-ASCII characters and leaves no
permanent installation. Owning all-target Clippy passes with unchanged checked
sources. Only macOS ARM64 execution is accepted; the other four targets have
format/publication-preparation evidence. See `plugins-notion-native-checks.json`,
`plugins-notion-actual-lifecycle.json` and `notion-plugin-candidate/`.

The candidate is not yet in the official repository or trusted catalogue. Its
desktop toolchain lifecycle adapter, host feature compatibility and remote
publication remain required. Tool/Flow and executable MCP still need the SDK's
reviewed permission-plan flow. BabelDOC still needs ownership of its complete
Python environment. Existing application-package licensing labels were not
changed. A fresh read still finds all three runtime GHCR packages private; the
known catalogue contains three package IDs, so 46 known IDs remain absent.
The goal remains active and no conditional product release was performed.

The desktop owner now installs external `plugin "toolchain"` archives into the
package generation's owned runtime directory. Downloads are prepared before
reconciliation, and extraction plus full-tree verification finish before that
generation is published. The verified runtime receipt digest is bound into the
publication cursor. Failed extraction leaves the working publication and raw
source markers unchanged. Existing generation leases retain accepted runtime
files through an update or removal, then reclaim them when the final lease ends.
Installed listings preserve a damaged tool's row and report `needsRepair`; the
plugin page offers retry after loading that persisted state. Repair still uses
the pinned official catalogue, while query and removal remain offline operations.

The final SDK batch passes 54 reconcile tests and 123 tool-acquire tests, with
five opt-in cases ignored. The focused Studio batch passes 34 cases, with its
real Notion case invoked separately. That actual candidate passes a fresh
upstream download, desktop installation, installed-list query, callable proof,
native `ntn 0.23.25` execution, API removal and lease retirement in a temporary
path with spaces and non-ASCII characters. It does not leave a permanent install
or invoke the upstream installer. The three owning all-target Clippy checks
pass; Studio still reports 306 warnings. Scoped Rust and renderer format checks
pass, and the three affected renderer suites pass 39 cases. This is native host
and component-test evidence, not native GUI acceptance or official publication.

Failures are retained. The first new runtime-record test expected `None` for
an invalid archive record; its unchanged isolated replay also refused the
record, and the expectation now preserves that typed error. An internal test
caller was updated for the new preparation argument. The broader Studio batch
had five failures, all replayed alone with its original binary and inputs:
four new fixtures supplied a prefixed hash where the existing tool declaration
requires plain hexadecimal; those fixtures were corrected without relaxing
the parser. The unchanged ACP cloud example fixture also lacks its AIX manifest
and remains a separate failure. The expanded renderer batch had one unchanged
ModelScope skill fixture that expects upstream HTTP instead of the current local
proxy; its isolated replay fails too. Full web typecheck still has 241 diagnostics
outside the changed paths. See `plugins-toolchain-native-checks.json`,
`plugins-toolchain-notion-desktop.json` and the retained expanded/isolated logs.

Publication remains conditional. At the final observation the parent and Studio
heads are unchanged, with 2304 and 469 status entries and four development
processes still present. The three runtime packages remain private; only three
of the 49 known IDs are in the official catalogue, represented by six records.
The Notion candidate is still unpublished. Other plugin migrations, permission
adapters, source licensing, anonymous package access, native GUI and other-target
validation remain outstanding. ACP, CLI and desktop releases still await a clean
qualified source freeze and their final archive, signing, notarization and
channel gates. No product tag or Release was created by this task.

The next goal turn adds seven concrete dependency recipes: uv 0.13.6, Bun 1.4.4,
fd 10.5.0, ripgrep 15.2.0, RTK 1.1.0, Lark CLI 1.0.98 and GitHub CLI 2.103.3.
All 35 upstream archives have independently checked release lengths/SHA-256 and
inspected complete layouts. The five native Registry target spellings are used;
the upstream x64 labels are not substituted for `x86_64` catalogue identities.
Both x86_64 Linux musl-only inputs (ripgrep and RTK) are measured as static ELF
before forming GNU-host candidates; this is not Linux runtime qualification.
All 35 AIX variants pass 105 build/check/prepare commands. Reopened sources match
the final ZIP members, and their archive sizes/digests, package identities, OCI
layer identities, OCI manifest digests and domain repositories agree.

The new native host case passes actual production HTTP downloads, acquisition,
callable evidence, installed-list queries, eight real version commands across
the seven packages (including uvx), API removal and exact generation-lease
retirement. It uses an isolated path with spaces and non-ASCII characters.
The initial case incorrectly asked `ToolStore.which` to find a companion as an
independent tool. Its unchanged input/binary replay also failed. The test now
uses the SDK's native executable lookup over verified installation directories;
the seven-case input hash and ten-second command deadlines remain unchanged.
The initial results and replay are retained. Four focused unit cases, the real
seven-package case, owning all-target Clippy and scoped format checks pass.
Clippy retains the existing 306 Studio warnings.

CUA CLI 0.9.0 additionally has a macOS ARM64 AIX candidate under
`plugins/operations/aster/cua`. Its declaration requires the CLI, virtualization
helper, microVM initializer and Linux guest helper, preserving their entire
73,285,759-byte upstream archive. Native build/check/prepare passes; a separate
actual host case passes installation, installed query, `cua 0.9.0`, API removal
and lease retirement. It does not execute the three helpers as macOS commands,
prove GUI/VM operation or claim Sigstore verification. The Notion target mapping
now explicitly distinguishes its unchanged upstream-style archive names from
canonical catalogue target names.

BabelDOC's exact `v0.6.4.post4` GitHub release has no binary assets. The isolated
Python 3.12/macOS ARM64 wheel preparation resolves and installs with hashes,
without executing the tool or changing a system installation. Before an owned
Python interpreter is added it contains 13,583 files, 15,135 total tree entries
and 722,258,729 expanded bytes; its dependencies-only gzip archive is
271,270,401 bytes. Both current tool archive limits (8192 entries and 200 MiB)
would reject it. No files were dropped or limits relaxed to obtain a pass.
Its declared console entry is `babeldoc-stream = babeldoc.main:cli`.

Prepared AIX inputs now cover 47 of the 49 known package IDs; first-party Box
and the complete BabelDOC runtime remain unprepared. Preparation is not official
publication: the signed catalogue still contains only three IDs/six records,
and all three current runtime packages still report private visibility. The
browser-access host is unavailable and both observed Quark application paths
fail selection, so no package visibility change was attempted through private
endpoints. A3S Browser still reports 0.3.2 against its [0.4.0, 0.5.0) guard.
Minimum implementing-host gating, remaining runtime/permission adapters,
application-source publication metadata, anonymous package access and full
remote/native GUI acceptance remain pending. No conditional product release
was performed. See `plugins-dependency-native-checks.json`,
`plugins-dependency-actual-native.json`, `plugins-cua-actual-native.json`,
`plugins-babeldoc-footprint.json` and `dependency-plugin-candidates/`.

## Implementing host floor and complete BabelDOC runtime follow-up

The Use SDK and extension contract are now 0.3.4. All 41 acquisition AIX
variants (seven tools across five targets, five Notion variants and macOS ARM64
CUA) require `>=0.3.4, <0.4.0`. Native admission rejects the 0.3.3 host input;
the implementing host accepts every candidate. The first build exposed a
schema-v3 parser defect that required compatibility with exactly 0.3.0.
Every failed archive case was replayed separately with the unchanged binary,
source and deadline before correcting the parser. Later patch floors now work,
while requirements admitting pre-0.3 hosts and empty 0.3 ranges still refuse.
The lockfile was regenerated through Cargo; frozen manifest/catalog fixtures
were not edited.

The 25 focused manifest, admission and signed-catalogue checks pass, as do
owning all-target Clippy for `aster-use-extension` and `aster-use` (including
the reconcile/tool-acquire features) and scoped format. The new catalogue case
uses its own typed record and production permission digest. The unchanged
existing catalogue fixture still prevents its incompatible/expired test from
parsing; both original failed cases were isolated and retained. A fresh 0.3.4
SDK profile verifies the actual official catalogue online and offline, retaining
the caller-provided root pin and all six records at metadata version 5.

All nine acquisition candidates pass the desktop owner's real download,
installation, installed-list query, requested version commands, removal and
lease retirement with their new AIX bytes. The seven-tool case takes 82.18
seconds; CUA and Notion take 32.03 and 24.73 seconds in the test runner. A shared
Studio edit initially referenced a missing `code_cli_tests.rs`; the unchanged
isolated compile failed too. The file later appeared, after which all three
native cases passed. Compiler sampling separately recorded macOS validating a
procedure-macro dynamic library. These are macOS ARM64 native-owner proofs;
they do not establish GUI acceptance or native Windows/Linux execution. Current
sources/artifacts are in `dependency-plugin-candidates-v2/` and
`notion-plugin-candidate-v2/`; earlier candidate directories remain historical.

BabelDOC's complete first runtime failed its actual entry because bitstring 5
removed `BitStream`. The failure was replayed unchanged. The recipe now follows
the upstream `<5` compatibility guidance, selecting bitstring 4.4.0; its tibs
dependency changes to 0.5.7 and bitarray 3.12.3 is added. Every other previously
resolved version remains constrained and unchanged. The copied runtime executes
`babeldoc-stream 0.6.4.post4` with its own CPython 3.12.14. Reopening its archive
confirms every file/link against the source: 15,270 regular files, nine relative
symlinks, 16,989 entries and 786,780,922 expanded file bytes. Its gzip archive is
294,583,079 bytes, SHA-256
`57e2fa8525cc34f3c139b029b8dc3d622b873afb8e5194d159a89200bb5a1e72`.
It remains a runtime candidate, not an AIX installation or translation proof;
bounded embedded-archive support is still required. No archive limits were
relaxed. The runtime bytes remain temporary; the recipe, lock, launchers and
verification are retained in `babeldoc-runtime-candidate-source/` and the
`plugins-babeldoc-compatible-*` records.

The refreshed release-governance run initially timed out three workflow inputs
at their existing 10-second deadlines. Each input was replayed separately,
then all 10 original tests passed without source or deadline changes. The
parent and Studio checkouts are still dirty, committed-head CI and whole-root
format checks are not green, the official catalogue still has only three IDs,
and all three current GHCR packages remain private. First-party Box and
BabelDOC AIX packaging, application-source publication/adapters, anonymous
artifact access, GUI acceptance and final product archives remain outstanding.
The conditional ACP, CLI and desktop release remains waiting; no product tag,
release or channel update was created by this follow-up. See
`plugins-host-floor-checks.json`, `plugins-host-floor-official-query.json`,
`release-host-floor-governance.json` and `release-after-development.json`.

## Embedded BabelDOC AIX and release preflight follow-up

Toolchain declarations now accept either the existing HTTP `url` or a portable
relative `archive` inside the admitted AIX. Both paths retain declared length,
SHA-256, platform and complete installation-tree verification. Embedded payload
reading runs off the async executor and retains the owning package bytes.
Runtime archives allow 512 MiB compressed, 32,768 entries and 4 GiB of file
bytes; the receipt remains bounded to 4 MiB. Desktop AIX extraction allows
512 MiB compressed and 1 GiB of outer file bytes, retaining its 512-entry cap.
The HTTP 200 MiB, uploaded-skill 32/64 MiB and generic companion 8,192-entry
limits remain unchanged.

The actual BabelDOC recipe 0.1.0 preserves tool version `0.6.4.post4`, embeds
owned CPython and all pinned wheels, and adds a multiprocessing-safe launcher.
It retains upstream AGPL-3.0 LICENSE/NOTICE and has a separate MIT recipe notice.
Its runtime has 15,270 regular files, nine relative symlinks and 786,780,953
expanded file bytes. The 293,903,386-byte runtime archive has SHA-256
`bfc1cef574cd1ddba11ce34d952a311e08b562c212d2d60e22794114319d6818`;
the final 292,412,155-byte AIX has SHA-256
`bf2a745eb305ace0b44a2b5657e119fe0f50d72b68683060f940a674d862f3ef`.
Native build, check and OCI preparation pass. The desktop owner installs this
exact AIX, queries its installed record, executes the actual version command,
removes it and retires its leased generation. The native case took 185.67
seconds; its command deadline was selected as 120 seconds before first execution
from the earlier measured startup. Existing nine-plugin deadlines remain 10
seconds. PDF translation, native GUI use and other-target execution are untested.

The SDK suite passes 126 tests, with four explicit opt-in cases ignored; the
desktop focused suite passes eight tests, with three explicit opt-in cases
ignored. All 10 dependency candidates pass actual native management, including
the seven-tool, CUA and Notion rechecks. Both owning all-target Clippy checks and
scoped format pass; Studio retains 306 reported warnings. The first SDK compile
failure accessed a private receipt field; its unchanged isolated replay failed
too, then the test was corrected to inspect the public installation API and
real files. Concurrent Core development temporarily broke the Studio consumer
compile. That separate compilation recheck observed changing Core source; the
plugin sources remained unchanged. After the owning developer's API correction,
the normal native batch and Clippy pass. First failures and rechecks are retained.

Reopened deliverables preserve 49 distinct IDs and 48 prepared IDs; only
`aster/aster-box` lacks an AIX candidate. Small BabelDOC source/legal files and
the OCI manifest are retained in `babeldoc-aix-candidate-source/`; the large
runtime and AIX bytes stay temporary. The copied source files and OCI layer,
archive length and manifest digest have been compared with their producers.
The official source still verifies six catalogue records for three IDs, both
online and offline, at metadata version 5. All six observed GHCR package paths
remain private. The other 46 IDs, application license/publication review,
Tool/Flow adapters and GUI management acceptance remain outstanding.

Release governance passes all 10 tests after replaying all eight initial
10-second workflow timeouts separately; the one remaining timeout then passes
an unchanged individual replay. The test/workflow sources and deadlines were
not changed. Developer ID and the existing notary profile are available locally,
but no new product archive has been signed or notarized. Other development,
dirty source, failed committed-head CI and whole-root format still prevent a
release. No product tag, Release or channel was created or updated. See
`plugins-embedded-toolchain-checks.json`, `plugins-babeldoc-native-lifecycle.json`,
`plugins-embedded-toolchain-official-query.json`,
`release-embedded-toolchain-observation.json` and `release-after-development.json`.

### Full-goal and plugin-page follow-up

The active objective again includes the complete frozen mise baseline, all
former environment-dependency functions through plugins, and DESIGN.md
acceptance. Earlier candidate counts do not close that objective or authorize
skipping its unchecked tasks.

The Box AIX candidate now passes native build/check/prepare, final archive
digest comparison and isolated native install/query/version/removal with its
runtime lease retained. This brings the historical authoring inventory to 49
prepared IDs. The Box native libraries use the pinned Rust target's macOS 11.0
baseline; its guest executable is static ARM64 Linux ELF. These checks do not
qualify VM execution, remote publication, GUI management or other host targets.
Box SDK Clippy still fails because its existing bridge protocol/operation JSON
fixtures are absent; the unchanged isolated replay confirms that failure.

The plugin page now retains the disabled Install action until host completion.
It confirms success only after materialization verification, and a subsequent
read failure retains Retry without repeating the installation. Its 32 renderer
tests and focused formatting check pass; ESLint reports no errors and five
existing warnings. The two original failing cases were replayed separately.
The read-recovery test uses the real SWR revalidation behavior: that API reports
read errors through hook state rather than rejecting its bound revalidation
promise. It checks successful-write feedback, the read error, recovery, exactly
one PUT and no DELETE. This is component evidence, not native GUI acceptance.
Header actions now wrap, and categories move above the cards below a 640px
container width. Essential descriptions/status/version text and local button
focus indicators use the existing Body color instead of low-contrast muted
values. Search has a localized accessible name. The same 32 tests, formatting
and ESLint checks pass after these edits; actual narrow-window, focus and zoom
acceptance remains open.

A fresh official-source query exposed a new release blocker: the retained SDK
binary expects the previous domain-bearing OCI target, while the live registry
now emits the package-identity layout. The failure is
`use.extension.registry_oci_invalid` and repeats unchanged in isolation. The
current source already reads the new layout; a fresh build and real query are
required before accepting that chain. Earlier metadata-version-5 successes
remain historical evidence. All six observed GHCR package locations are still
private. No product version, tag, Release or channel was changed by this work.

The fresh pinned, locked `aster-use` build then passes both actual online and
offline searches in the same isolated source profile: three IDs at signed
metadata version 6, with `refreshed` and `cached` provenance respectively. Its
copied binary remains unchanged throughout the queries. This resolves the SDK
query mismatch; it does not verify a rebuilt desktop host, anonymous artifact
installation or the other 46 known package IDs. See
`plugins-official-current-build-query.json` and
`plugins-official-current-build.log`.

See `plugins-current-checkpoint.json`, `plugins-box-validation.json`,
`plugins-box-native-management.json` and the `plugin-ui-owner-*` logs. Full mise
replacement, all-plugin GUI acceptance and final release gates remain open.

### Multiple-tool and global uninstall implementation

The pinned upstream uninstall source was read at commit
`3c78bbb2d58c0b668fe8d5bebf4a8cb66194e1aa`. Its five additional native reference
cases pass: multiple exact tools, a later ambiguous tool, global removal, global
dry-run-code and duplicate requests. The fixed binary was downloaded again and
matched the existing frozen SHA-256; it is a test oracle, not an Aster runtime
dependency. Every case uses fresh local plugin/store state, the predeclared
25-second deadline, physical installation membership and execution of survivors.

Aster now admits multiple uninstall arguments and `--all` without a tool.
The shared store discovers identified versions without current pointers or
complete executables, bounds the whole traversal, and rejects malformed
per-version metadata and linked directories. The command resolves all requests
before deletion, deduplicates effects, preserves request/version order, and
retains the existing single-tool JSON fields. New JSON `tools` entries identify
each selected name/version once. Tests cover missing selection, later invalid
or ambiguous requests, previews, unchanged ACL bytes, noncurrent installations,
missing executables, unpinning only selected names and empty repeat previews.

Scoped Rust formatting and all seven workspace-locality tests pass. The first
workspace check used Apple's Python 3.9, which lacks `tomllib`; its unchanged
isolated failure and the successful Python 3.13 run are retained. Whole-root
formatting still fails outside these edited paths. Aster acceptance is pending:
the original tool-test compiler and queued CLI/ACP build remain live. A sampled
compiler is waiting inside dyld's code-signature mapping of a proc-macro dylib;
it has not returned a compile failure. No running build was killed or restarted.
See `multi-uninstall-progress.json` for current source hashes and exact handles.
No full-mise capability row or release gate is marked complete by this work.

The initial compiler wait then resolves and the tool library passes 129 cases
with four ignored tests, including all three new global-discovery tests.
Because final source edits occurred during that first compile wait, a new run
captures the final source hashes before and after execution. The CLI/ACP pair
build is compiling, and owning Clippy for tool-acquire, pager and CLI is queued.
Actual Aster CLI comparison and the final-source checks remain unaccepted until
their original handles return; the five mise reference results do not replace
them.

### Verified uninstall and collection recovery

The final-source tool suite passes 129 tests with four ignored; its independent
Clippy check passes. Pager environment-command tests pass all 15 cases. The
initial 26-case actual CLI batch passes 18 and times out in eight. Every failed
case then passes alone, using the same test binary, CLI binary, inputs and
20/25-second fixture deadlines; binary digests remain stable throughout those
replays. This does not qualify the original concurrent batch.

Clippy identifies four indexed writes in the new uninstall JSON report. An
unchanged isolated owning-package check repeats them. The report now constructs
a JSON object map directly, preserving the single-tool fields and deduplicated
name/version entries without indexing. A fresh paired CLI/ACP build and all
seven actual CLI uninstall tests pass after the correction, with unchanged
source hashes. These tests exercise the new five-case and earlier eight-case
frozen mise references, global preview/unpin, invalid later selections, damaged
payloads, surviving executables and preserved ACL data. Package-wide Clippy
still fails on exactly the other 42 indexing diagnostics in nine untouched
modules; no allowance was added. Diagnostic counting retains multiplicity:
nested indexing can produce two findings at the same source location.

The plugin collection now accepts the owner's corrected page after deletion,
resets page and committed criteria together, preserves the page for whitespace
that does not change the search, retains a selected category on no matches,
and rejects malformed pagination metadata at the data boundary. Market and
installed owners return the nearest valid page, including page 1 for an empty
collection. The native test builds and reconciles three AIX packages, requests
page 3, withdraws their actual generations, and checks recovery through pages
2 and 1 to the empty collection. All 13 native Registry tests and current
Studio Clippy pass. The three current renderer suites pass 97 tests; formatting
passes and ESLint reports four existing import-boundary warnings, with no errors.
The original paging/search/metadata failures and individual replays are retained.
An additional test initially held the detached pre-navigation pager element;
its isolated failure is retained and its locator now checks the live pager.

The broader shared capability client suite retains one ModelScope route/fixture
failure: its fixture rejects `/api/modelscope/openapi/skills`, before execution
can reach the changed page decoder. The identical isolated case still fails;
its expected result and fixture were not changed. Native GUI focus, narrow-window,
zoom and full management acceptance remain unverified.

Current root formatting and all seven workspace-locality tests now pass. The
official source verifies online and offline at signed metadata version 8 with
four IDs: document, OCR, Office and sandbox. Seven observed GHCR paths remain
private. The original 49-item inventory stays in scope; the current source adds
`aster/aster-ocr`, yielding 50 distinct known IDs. Forty-six original IDs remain
absent from that catalogue. Historical candidate preparation is not requalified
publication or anonymous installation evidence.

See `plugins-and-cli-verification-current.json`, the `mise-multi-uninstall-*`
logs and the `plugin-pagination-*` logs for source hashes, commands, counts,
failures and coverage. The full frozen mise objective and release gates remain
open; no product tag, Release or channel was created by this work.

## 2026-11-02: batch install/use implementation, native checks pending

`tool install` and `tool use` now accept multiple arguments. Install uses a
bare tool's merged declaration; bare use selects the newest complete stable
local installation, retaining provider prerelease flags but ignoring remote
aliases. Explicit `@latest` remains a provider request. Identical arguments are
deduplicated in order. Argument resolution and external-path serialization
finish before acquisition; use writes ACL once after all selections succeed.
The current ACL map still holds one version per tool: batch installation can
select several versions, while use refuses conflicting same-name requests.

The unchanged frozen mise binary passed seven new reference cases through 73
native commands, including configured bare install, two-version installation,
duplicate requests, pinned batch use, bare installed reuse and a later failed
acquisition preserving configuration. Capture command:
`python3.13 tests/cli_mise_batch_install_reference.py --mise .tmp/mise-replacement/reference/mise --output specs/031-cli-mise-replacement/contracts/mise-batch-install-reference.json`.
`cargo test --locked -p aster-use-tool-acquire --lib` passes 130 cases, with four
ignored. The added store test checks newer remote aliases, provider-marked
prereleases and unchanged current pointers against actual local installations.

Pager tests and owning tool Clippy were launched but have not returned at this
capture. The pipeline queues the paired CLI/ACP build, actual CLI tests, pager
Clippy and root checks behind them. Original compiler
processes were retained. Native samples observed procedural-macro dynamic
library loading waiting in macOS dyld signature validation through `fcntl`;
the initial compiler processes subsequently resumed. Two CLI fixture refinements
occurred before its first compilation/execution and are recorded separately;
no frozen reference inputs or timeouts changed. See
`batch-install-use-pending.json` for source hashes, returned results and running
session references. The running pipeline remains in `.tmp/mise-batch-install-verification.json`.

Explicitly not claimed: new CLI acceptance, package-wide Clippy, this source's
root gates, full mise parity, complete dependency-page migration, native GUI
acceptance or release qualification. No product tag, Release or channel was
created by this work.

## 2026-11-02: returned batch install/use and installed tool-version results

The pending batch pipeline above has now returned. The owning tool Clippy and
paired CLI/ACP build pass, as do all 18 pager command tests, root formatting and
seven workspace-locality checks. The full sequential CLI batch passes 28 cases
and times out in its first explicit companion-override case. That exact case
passes a separate invocation of the unchanged test executable with stable CLI
and test-binary SHA-256 digests; inputs and the 20-second timeout were not
changed. All three new batch cases pass in the original batch, including the
seven native mise comparisons, mixed managed/path/system selections, current
pointer independence, cached newer aliases and later-argument refusal.

Pager Clippy remains failing in nine other modules. It emits 42 diagnostics for
the library and repeats them for its unit-test target: 84 total diagnostics,
39 unique primary locations, none in `env_manager_cmd`. No lint allowance was
added. Two CLI test-fixture refinements occurred before the CLI compilation and
execution began; the final source hashes and timestamps establish that ordering.
See `batch-install-use-results.json` and its retained exact command logs.

The plugin page now displays the installed tool version independently from its
AIX release version. The native Registry retains the version from a fully
verified toolchain receipt, including offline installed entries; a damaged
runtime has no installed tool version until repaired. The renderer preserves
the complete receipt version and keeps the package version in install/update
requests. An existing real lifecycle case now checks recipe `1.0.0` against
tool `0.6.4.post4`; the renderer repair/removal flow checks package `1.0.0`
against tool `0.23.25`. Its new visible-version assertion failed before the fix
and in an unchanged isolated replay. The corrected plugin suite passes all 37
cases. Six native toolchain cases pass (three external-candidate cases ignored),
all 13 Registry cases pass, and Studio Clippy passes with existing warnings.

Scoped Oxlint, ESLint, formatting and i18n checks pass; ESLint retains four
existing cross-page import warnings. The full non-mutating equivalents of the
lint stages were also run to preserve unrelated worktree changes. Global
Oxlint's tsgolint child panics on the existing `bot/asterBotAvatar.tsx` program
membership; a scoped run over the three changed renderer files passes. Global typecheck
reports 263 diagnostics (four node, 18 E2E, 241 web), none in the changed
renderer files. The broader renderer batch passes 55 and fails the existing
ModelScope URL fixture; that exact isolated case still fails before the changed
decoder. See `plugins-installed-tool-version-results.json` for the actual
commands, source hashes, retained failures and coverage limits.

Five further cases from the unchanged frozen mise binary now establish the
next required multi-version behavior through 25 native commands. Declaration
order selects the default executable; later versions also provide otherwise
missing commands; identical requests collapse to a single scalar declaration.
The producer initially compared macOS `/var` and `/private/var` spellings
inconsistently. Its unchanged isolated failure is retained, and both paths are
now canonicalized before comparison. Native commands, hooks, case inputs and
25-second timeouts were unchanged. Aster does not yet implement these five
cases; `mise-multiversion-reference-results.json` records that limit explicitly.

The installed A3S Test Web adapter currently rejects A3S Browser 0.3.2 against
its required `[0.4.0,0.5.0)` range. Its separate GUI matrix admits macOS profiles
against locked CUA 0.23.2, matching the installed Driver's version; that matrix
is not a host certification. A fresh desktop/ACP/Office build was started through
`CARGO_TARGET_DIR=/Users/roylin/code/aster/target just build-studio aster` for a
future owned-window GUI check. It has not yet returned at this capture.

Explicitly not claimed: full mise replacement, multi-version declarations in
Aster, complete migration of the old dependency page, GUI/keyboard/zoom
acceptance, anonymous GHCR installation, Windows/Linux native qualification or
release readiness. No product tag, Release or channel was created by this work.

## 2026-11-02: ordered multi-version declarations and locks

`EnvDeclarations` adds ordered request lists without changing the scalar
`EnvConfig` Rust contract. The CLI now reads and writes those lists, preserves
them during variable edits, replaces a whole tool list at a nearer declaration,
and removes only matching pins during uninstall. Every selected version enters
PATH in declaration order. Executable lookup includes verified companion
commands from later versions. V4 locks retain each version's request, archive
and complete installation record; changed order or membership refuses locked
execution. Existing single-version lock schemas remain readable.

All five unchanged native mise reference cases now pass actual Aster CLI
comparisons: forward order, reverse order, configured installation without a
rewrite, duplicate collapse and a command supplied only by the later version.
The same integration case also executes the v4 lock offline, edits variables
without losing version lists, and unpins one version while preserving the
other version and unrelated variables. The frozen reference retains five cases,
25 native commands and SHA-256
`70ab8db3b3034b0c20453e6638ddd64ac538b39d601a2d67f4dbc248107ec07b`.

Returned verification:

- `cargo test --locked -p aster-use-tool-acquire --lib`: 134 passed, four ignored.
- `cargo clippy --locked -p aster-use-tool-acquire --all-targets --no-deps`: exit 0.
- `cargo test --locked -p aster-pager --lib env_manager_cmd::tests`: 18 passed.
- `cargo build --locked -p aster -p aster-acp --bins`: exit 0.
- `cargo test --locked -p aster --test env_manager -- --test-threads=1`: all 30
  passed, no timeouts or isolated acceptance substitutes in this batch.
- Pager all-target Clippy: exit 101 with 84 diagnostics at 39 unique primary
  locations in nine other modules; none in `env_manager_cmd`. Multiplicity is
  retained in the diagnostic summary, and no lint allowance was added.
- Root `cargo fmt --all -- --check`: exit 0. Workspace-locality checks: seven
  passed.

The original old-binary refusal is retained under
`.tmp/mise-multiversion-cli-direct-before.json`. An early compilation used the
wrong ACL enum spelling `Value::Array`; the AST uses `Value::List`. The SDK was
corrected by another concurrent writer while a check was running, so that check
is not an unchanged failure replay. Final commands retain their start hashes
and actual results in `mise-multiversion-implementation-results.json`. The
integration fixture was refined while the pager build ran; its monitored hash
changed before CLI compilation. Owning tool/pager files stayed unchanged, and
the subsequent paired build and all CLI tests used their recorded later hashes.

Explicitly not claimed: full mise replacement, multi-version temporary
overrides, shims/stubs/synchronization, reference execution, path/system locks,
complete backend/multi-platform locks, native Windows/Linux execution or
release readiness. The broader unchecked capability matrix remains required.
No product tag, Release or channel was created by this work.

## 2026-11-02: native desktop plugin follow-up

The [desktop outcome](../../desktop/2026-11-02-plugins-native/OUTCOME.md)
retains development-bundle/cache repair, runtime-event list refresh,
account-free callable-route verification, 39 passing renderer cases and
successful native build/lint checks. Real desktop installation/removal and
search recovery were observed, but post-installation market-load errors keep
GUI acceptance failed. Four official records do not complete the original
inventory migration, domain-directory layout or the full mise matrix.

## 2026-11-02: ordered temporary tool versions

The actual earlier CLI returned `two` for `exec -t node@path:./one -t
node@path:./two -- node`. It collected repeated tools into a scalar map and
discarded the first request. Exec now groups requests in argument order and
uses the shared `resolve_tools_with_version_overrides` API. Each overridden
tool replaces its complete declared list before native path inspection; other
tools retain their declaration roots and all versions. Exact duplicate
requests retain their first occurrence. The published scalar API delegates to
the additive API without changing its signature. Empty and oversized lists
refuse before path lookup. Project declarations, variables and store current
pointers remain unchanged by cached temporary execution.

`mise-exec-multiversion-reference.json` freezes nine new native cases and 24
commands from the same pinned binary. The producer validates its binary hash,
retains the original 25-second per-command deadline, and compares its reopened
capture against all nine planned input identities and orders. The original
five-case multi-version reference retains SHA-256
`70ab8db3b3034b0c20453e6638ddd64ac538b39d601a2d67f4dbc248107ec07b`.
The new reference has SHA-256
`e19993e4c793867cff05d3045f41ab80f9a8c64a12d119195451c2ca626c4678`.

Initial native captures timed out in managed installation and native exec; an
unchanged older context probe also timed out. Those failures remain retained.
Separate verbose, environment-order and sampled probes are diagnostic evidence,
not accepted reference replacements. A subsequent full replay of the original
nine inputs, producer and deadlines passes without sampling or verbosity
changes. Only that complete result becomes the new frozen reference.

The CLI test reads its expected output and PATH order directly from that
reference. It runs all nine cases with both `--offline` and `--no-install`,
preserves a separate parent-declared tool and variable, verifies no declaration
or current-pointer changes, and refuses a later missing path before the
requested process can create its marker. These fixed command deadlines remain
20 seconds. Both managed versions and native paths execute their real fixtures;
no mise executable is present on the tested CLI PATH.

| Check | Actual result |
| --- | --- |
| Tool context cases, including the new ordered/limit cases | 8 pass |
| Whole tool library | 138 pass, five external cases ignored |
| Tool owning Clippy | Exit 0 |
| CLI owning Clippy, all targets | Exit 0 |
| Focused actual CLI comparison | One test passes all nine frozen cases in both modes, plus the failure-before-effect assertion |
| Final-source paired CLI/ACP build | Exit 0; tracked sources unchanged |
| Full sequential CLI environment suite | 31 pass, no failures/timeouts; 151.99 seconds |
| Pager command suite | 18 pass, zero failures; the retained live command returned exit 0 |
| Post-regression CLI/ACP identities | Initial mismatch retained; final paired rebuild and direct 31-case regression pass, with matching identities and unchanged final artifact hashes |
| Pager Clippy, all targets | Exit 101: 42 diagnostics, the same 39 primary locations as the retained prior result; none in the changed command module |
| Final workspace formatting | Exit 0 |
| Workspace dependency checks | Seven pass |

The first paired build also passed before the test was wired to the frozen
reference. The final paired build follows that source change. Binary identity
and final regression evidence are retained with the follow-up rather than
attributed to the earlier build. The pager diagnostic packet lists exact
locations for the existing indexing and related lint follow-ups; no allowance
was added. Native platform qualification remains macOS ARM64 with Rust 1.97.1
and the recorded development/test profiles, not release-performance acceptance.

[mise-exec-multiversion-results.json](mise-exec-multiversion-results.json)
retains 53 source/reference/check files, including failed reference attempts
and the initial mismatched binary identities. Execution session 80178 returned
exit 0: all 18 command-module tests pass. Rebuilding both CLI and ACP then
returns exit 0. Because the CLI artifact changed, the unchanged native
integration-test binary runs all 31 cases again without invoking another
single-product Cargo build; all pass in 123.48 seconds. Both final binaries
report `aster-cli-pair-v1:04f9a33ad03dff4794deeacc47f57df6ef1590e6:aarch64-apple-darwin`.
Their hashes remain unchanged across that regression. The tested CLI SHA-256
is `7a3e31b4348bfda45b31946c0578d5f0dd468f4faca5e968883d7924d58c9083`;
the ACP SHA-256 is
`e4cdca944d2478a4aa158b9c275c3566a0bad1915d9643ef3093f2902e6e263d`.

The prior desktop runtime Clippy queue has now returned: its first attempt
fails in an unrelated path assertion, the corrected checkout passes the same
command on replay, and the runtime-progress sources remain unchanged. Both
logs are retained by the linked desktop packet. A later desktop GUI replay
reproduces and fixes the plugin page's missing registry-selection argument,
then verifies a real OCR download, dismissal, reopening and final installation.
Its additional completed-state replay and remaining limits are recorded in
the desktop `runtime-progress-registry-route-results.json` packet.

Explicitly not claimed: full mise replacement, shims/stubs/synchronization,
reference execution, path/system locks, all backends, the full plugin inventory,
native Windows/Linux/Intel macOS behavior, complete plugin-page visual/GUI
acceptance, packaged archive gates or any product publication. The full
unchecked capability matrix and conditional release requirements still apply.

## Unix project dispatch shims and repair

Unix CLI installation and selection now regenerate project dispatch shims.
Each invocation reads the current directory's declarations and uses the shared
ordered-version execution path. It preserves native argument bytes and its
bound tool store even when the caller changes `ASTER_HOME`. A missing selected
version uses the normal native acquisition path; an unconfigured owning tool
refuses instead of using the global current pointer. Production execution does
not call mise.

`aster tool reshim` collects declared commands from every recorded version,
including an executable flat companion or a command supplied only by a later
version. It repairs missing entries and removes obsolete scripts only when
they carry the owned dispatch header. It preserves unrelated files and linked
entries. The renderer runs before publication and the existing store lock
covers the snapshot and writes; renderer failure leaves existing scripts
unchanged. This does not claim rollback of every possible multi-file I/O
failure or complete cross-tool command-collision parity.

The unchanged pinned native mise executable supplies six new cases. The first
capture exposed an incorrect new-fixture assumption: a missing selected version
is automatically installed. The next capture exposed an omitted launcher
directory on the fixture PATH. The pinned upstream
[shim launcher implementation](https://github.com/jdx/mise/blob/3c78bbb2d58c0b668fe8d5bebf4a8cb66194e1aa/src/shims.rs)
confirms its launcher lookup. The complete corrected native capture passes all
six cases and is copied byte for byte to the new frozen contract. Both failed
captures, their producer variants and the earlier Python 3.9 `tomllib` admission
failure remain retained. These were pre-admission fixture repairs, not edits
to an already accepted expected result.

The actual Aster CLI matches five exact frozen fixture cases: forward/reverse
project selection, later-version companion fallback, unconfigured refusal and
missing-shim repair. The initial Aster fixture used an invalid top-level ACL
block; the strict parser refused it. It now uses the public ACL serializer with
the same ordered versions and unchanged frozen expectations. A separate real
canonical GitHub-backend case starts with uv 0.13.6 absent, invokes its shim,
installs it and executes the real version command. That verifies the missing
selection behavior; it does not reproduce the upstream custom-plugin ABI.

| Check | Actual final result |
| --- | --- |
| Whole tool library after lock/lint cleanup | 139 pass, five explicitly external cases ignored |
| Pager command suite | 18 pass |
| Tool owning all-target Clippy | Exit 0, no changed-path finding |
| CLI owning all-target Clippy | Exit 0, no changed-path finding |
| Pager owning all-target Clippy | Exit 101; 42 diagnostics at the same 39 prior primary locations, none in the command module |
| Paired CLI/ACP build | Exit 0 |
| Full sequential actual CLI environment suite | 32 pass, one external case ignored; 102.54 seconds |
| Explicit real uv auto-install against the final pair | One pass; 29.96 seconds, original 120-second command budget |
| Root format | Exit 0 |
| Workspace dependency checks | Seven pass |
| Regression identities and hashes | Matching CLI/ACP IDs; both binaries, the test runner and all sampled sources unchanged across both final regressions |

The first owning Clippy run identifies six needless borrows in the new shim
code. Removing those warnings adds no allowances; the clean source then passes
the owning checks above. The initial single-product harness build also advances
only the CLI build identity. Both that mismatch and the first matching paired
build remain retained. After the lint cleanup, the final paired build reports
`aster-cli-pair-v1:ad4358ab227821ae8767c39a6009b2164ae671fc:aarch64-apple-darwin`
for both binaries. The final CLI SHA-256 is
`4b0b6078b46228a60915801c880cc7e0cec8e911243cd24be267c432f7333a3f`;
the ACP SHA-256 is
`46621fc3d5c694641d426336eb6073de6bb8798acd5949f22642746775212227`.
The integration runner is invoked directly after rebuilding the pair, avoiding
another single-product Cargo invocation. Its fixture code is unchanged after
the six library borrow cleanups; its before/after hash is retained too.

[mise-shims-results.json](mise-shims-results.json) retains 52 reopened,
hash-verified source/reference/check files. Native qualification is macOS ARM64
with the pinned Rust 1.97.1 development/test profiles. The final comparison uses
the same test inputs and existing command deadlines. No runtime timeout or
expected output was weakened.

Explicitly not claimed: complete C04/T004, Windows project dispatch, stubs,
remaining synchronization modes, dynamic shell activation, reference execution,
path/system locks, upgrades, pruning, full backend/legacy-plugin behavior,
original plugin inventory and desktop acceptance, other native platforms,
release performance, final packaged archive gates or publication. All 24
families and the unchecked T004–T014 requirements remain required.

## Project-scoped removal — initial validation checkpoint

The CLI now has `tool uninstall --project`, selecting the nearest project ACL
without merging user or parent declarations. It preserves declaration bytes,
skips path/system requests, supports preview and non-interactive confirmation,
and prompts before actual project removal. Eight new pinned native mise cases
pass exact installed-state comparisons. The healthy checkpoint passes both
focused CLI cases, all 34 default CLI cases and two A3S Test TUI scenarios using
real uv 0.13.6. The first scenario declines and the second confirms; declarations
remain unchanged and both cleanup records have no error. These checkpoint
artifacts and sampled sources stay unchanged during that run.

A further native counterexample shows that mise removes a version whose
executable is missing. A real uv reproduction in Aster instead returns exit 1
and leaves the version directory: project selection incorrectly requires an
executable suitable for execution. The new read-only `uninstall_candidate_for`
API shares version resolution with the existing store and selects identified
receipt records for cleanup. Execution keeps its original completeness mode.
Its focused real-filesystem regression passes, as do the latest 141 tool tests
(five external cases ignored) and owning Clippy. The final sequential command,
CLI and paired-build checks are still running; native acceptance of this fix
is not yet claimed.

The initial supplementary guard fixture is rejected by macOS with `Illegal
byte sequence` before reaching the CLI. Its original invalid-byte assertion is
retained for Linux, with native Linux qualification still required. A subsequent
intermediate build fails with E0599 because the caller was edited while Cargo
held the earlier SDK metadata. That failure is retained; the new SDK test passes
after recompilation, and further checks now run sequentially. No frozen expected
result or command deadline was relaxed. The latest root format check fails in
four unrelated Studio files. Pager Clippy retains its prior 42 errors at 39
locations, plus the previously recorded test-only canonicalization warning.

[mise-project-uninstall-progress.json](mise-project-uninstall-progress.json)
retains 31 reopened, hash-verified source/reference/check files. Explicitly not
claimed: final damaged-cleanup native acceptance, all project profile/file and
backend modes, native Linux/Windows qualification, full replacement or plugin
migration, desktop visual acceptance, packaged release gates or publication.


## Project-scoped removal — final native verification

`aster tool uninstall --project` now uses the nearest base project ACL and its
own ordered requests. Parent/user declarations do not enter this selection;
path/system requests are skipped. Preview preserves installations and config,
non-interactive removal requires `--yes`, and terminal confirmation can be
declined. A valid recorded version remains removable when its executable is
missing. The shared resolver's new identity-only removal path keeps source and
receipt checks, while execution retains completeness verification. These
behaviors are compared against the pinned
[upstream removal contract](https://github.com/jdx/mise/blob/3c78bbb2d58c0b668fe8d5bebf4a8cb66194e1aa/docs/cli/uninstall.md)
and actual native captures; the existing eight cases are unchanged and the
additional missing-executable case has its own frozen contract.

The final source passes three focused CLI tests, including all nine native
comparisons and the preflight guards. All 35 sequential default CLI environment
tests pass, with one unrelated external case explicitly ignored. Two A3S Test
TUI scenarios use the actual uv 0.13.6 installation: decline leaves it for the
second prompt, confirmation removes it, both cleanup records are null, and the
public installed list becomes empty. Configuration bytes remain unchanged.
The selected real uv executable has the same SHA-256 before and after the fix.
Deleting that executable originally produced exit 1 and left the version
directory; the final binary returns exit 0, removes the directory and reports
an empty installed list. The error was reproduced before editing the selector;
no output or timeout was changed to make the checks pass.

| Check | Actual final result |
| --- | --- |
| Whole tool library | 141 pass, five external cases ignored |
| Pager command suite | 18 pass |
| Tool / CLI all-target Clippy | Both exit 0; no new owned-path finding |
| Pager all-target Clippy | Exit 101; the same 42 errors at 39 prior primary locations |
| Existing sibling-test warning | `std::path::Path::canonicalize` at line 312, unchanged from before this work |
| CLI test build and paired CLI/ACP build | Both exit 0 |
| Focused actual CLI project removal | Three pass |
| Full sequential actual CLI environment suite | 35 pass, one external ignore; native runner reports 107.75 seconds |
| A3S real terminal confirmation | Two pass, zero cleanup errors |
| Real uv missing-executable cleanup | Exit 0, directory absent, installed list empty, config unchanged |
| Scoped and root formatting | Exit 0 on the latest checks |
| Workspace dependency checks | Seven pass |

The final CLI/ACP pair reports
`aster-cli-pair-v1:479d87f8c04f1768e0f4d4f59dc1336ea98ed695:aarch64-apple-darwin`.
CLI SHA-256 is `297c67b4a85769380d8688091bef2dcdcbcf71f59429bb6b4cb2cdb79df21485`;
ACP SHA-256 is `a9c58a3c404652d2a43e0b3ebda7c5a5440400418d748eb3c741e26469df7c4d`.
Both binaries, the integration runner and all sampled source files remain
unchanged through the final run. The integration tests run directly after the
paired build, without another single-product Cargo invocation. Only the
explanatory CLI documentation and task/matrix witnesses change afterward.

[mise-project-uninstall-results.json](mise-project-uninstall-results.json)
retains 57 reopened, hash-verified files, including the final native/TUI
reports, VT evidence, source/before snapshots and checks. Its linked initial
progress record retains the APFS fixture failure, real pre-fix cleanup failure
and the intermediate E0599. The invalid-byte filename case remains a required
Linux-native test; macOS rejects that input at filesystem admission. The final
serial compilation resolves the stale SDK metadata. The earlier four unrelated
Studio format differences no longer appear in the latest root format check.
The Pager errors and existing canonicalization warning remain exact follow-ups,
with no allowance added.

Explicitly not claimed: complete C02/T004 or C04, all project profile/file
variants, all backend/version/plugin modes, upgrades, pruning, stubs,
synchronization, native Linux/Windows/Intel macOS qualification, all 49 original
plugin migrations/publications, desktop consumer/visual acceptance, release
performance, final archive/sandbox/channel gates or publication. All 24 feature
families and unchecked tasks remain required by the full objective.

### Box maintained source and qualified-input recipe

`plugins/plugins/operations/aster/aster-box` now retains the native candidate's
schema-v3 manifest, original source snapshot/origins, MIT recipe license and
native component notices. The primary domain is the candidate's explicit
`operations` admission. No package identity, target or domain is inferred from
its name. The target recipe stages the complete source outside Git, generates
`toolchain.acl`, and invokes native build/check/catalog preparation. It admits
only the existing 26,582,246-byte runtime archive with SHA-256
`73e341931c6d8e3171b114a1ccb3694895cb1fb3d949c96bbb3256f510d47ef7`.
It resolves no new upstream release and commits no native payload.

Two distinct preparation failures are retained. The preexisting SDK executable
looks for the old flat source path; replaying preparation alone repeats that
failure. A pinned, locked current-source SDK build succeeds in 5m 25s, retaining
its existing unrelated `RuntimeUnitSpec` import warning. The fresh SDK then
rejects this increment's unsorted metadata keywords; the same isolated command
repeats the catalog error. Sorting those keywords fixes the new input without
changing validation, package identity, runtime bytes or test assertions. The
initial metadata, both SDK identities and all failed/replayed outputs remain
in the evidence packet.

| Check | Actual result |
| --- | --- |
| `python3 -m unittest discover -s plugins/tests -v` | 12 pass, including two damaged Box runtime rejections |
| `cargo build --locked -p aster-use --bin aster-use` | Exit 0 with pinned Rust 1.97.1 on macOS ARM64 |
| Maintained recipe's native AIX build/check/catalog prepare | All three exit 0 |
| Canonical source/staged source/final AIX comparison | All five canonical files and seven final members match byte for byte |
| Retained Studio native test runner, exact dependency lifecycle case | One pass; install, installed query, Box 3.2.7 execution, removal and final-lease cleanup |
| Scope/whole plugins diff whitespace checks | Exit 0 |

The final AIX has 26,529,849 bytes and SHA-256
`05e32dd6d1270cb02195a156c138c650406102c98fc5252873d3b0d3a4e761f6`.
The native owner reads that exact digest and executes `aster-box --version`
with its unchanged ten-second command deadline. The copied SDK and retained
Studio test runner keep their hashes throughout final verification, as do all
nine sampled authoring files and ten sampled native-validation source files.
This source-only increment reuses the existing compiled Studio test runner;
it does not claim a new desktop build. No Rust source is edited here.

There are now 16 maintained source units and four local catalog records. This
increment creates no catalog release record, TUF metadata, product tag, channel
or publication. The canonical source is an input to its target recipe, requiring
the separately qualified archive; it is not independently buildable without
that input. The original native provenance document describes Windows artifacts
and does not establish corresponding-source redistribution for this macOS
candidate. That limitation is explicit in the new source README.

[box-maintained-source-results.json](box-maintained-source-results.json) retains
reopened, hash-verified authoring files, recipe logs, current SDK build output,
source/archive comparison, native management evidence and both failure replays.

Explicitly not claimed: Box VM execution, macOS corresponding-source publication
qualification, other hosts, a current Studio rebuild or GUI acceptance,
anonymous registry installation, migration/publication of every original ID,
complete mise replacement, release performance, final archive/sandbox/channel
gates or release. All 24 families, the complete original inventory and the
unchecked implementation/verification tasks remain required.

### Box runtime execution through the installed package owner

The previous turn made concrete progress by maintaining Box's source and proving
its installation lifecycle. This turn closes the gap between version startup
and two basic native MicroVM executions. The original maintained AIX is first
checked outside the development tree with isolated `ASTER_HOME`/`HOME`, no
DYLD overrides and only system directories on PATH. Its exact payload starts
an ARM64 Linux guest, prints the fixed marker and `uname` results, exits, and
leaves an empty instance inventory. This diagnostic is separately labelled;
it is not substituted for the installed-package consumer test.

The Studio test now exercises the native package owner's real installed
executable while holding that package's runtime lease. It loads a fixed offline
Alpine OCI archive, uses state paths containing spaces and Chinese text, runs
one Linux command with exit 0 and a second command with exit 17, and requires
empty instance inventories afterward. Existing installed-query, package-removal
and final-lease retirement assertions still execute. The shared helper accepts
an optional test consumer; its two original callers pass `None`, retain their
original deadlines and preserve the original report shape. The default caller
is also replayed against the exact final Box AIX.

`contracts/box-native-vm-reference.json` freezes the inputs before the first
native owner run: Alpine 3.22.1, pulled digest
`sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1`,
the 4,139,520-byte exported OCI fixture with SHA-256
`0429125e10364631040a9eda0da9340494e6b5e0552403533e480b4e111841cb`,
exit codes 0/17, guest stdout blocks and the unchanged declared deadlines.
The native test requires this qualified external fixture; the native image is
kept outside Git. Its successful `--rm` path already removes each instance.
The subsequent unconditional cleanup attempt returns 1 because it is absent;
the independently queried inventory must still be empty. The test does not
branch on rendered cleanup error text.

| Check | Actual result |
| --- | --- |
| `cargo test --locked -p aster-studio --lib -- capabilities::toolchain::tests --nocapture` | Six pass, four external cases ignored |
| Initial native installed-Box consumer | One pass; two real VM commands, exits 0 and 17 |
| Final README-bearing AIX build/check/catalog prepare | All three exit 0 |
| Canonical source/staged source/final AIX | All five canonical files and seven AIX members match byte for byte |
| Final native installed-Box consumer | One pass; same inputs, deadlines and two VM commands |
| Final original dependency-owner caller | One pass; original output shape and lifecycle retained |
| `cargo clippy --locked -p aster-studio --all-targets --no-deps` | Exit 0; no diagnostic in either changed Rust file |
| Scoped Rust format / latest full root format | Both exit 0 |
| `python3.12 tests/workspace_dependencies.py` | Seven pass |

The Clippy run reports 309 Studio lib-test warnings, including 246 duplicates.
The two type-complexity matches were read in context at
`agent_plane/catalog.rs:639` and `workspace_browser.rs:303`; neither is an edited
path. No allowance is added. This is a scoped review, not a claim that every
unrelated warning or Rust guideline has been audited. The first full-format
observation reports differences in three parallel-development files
(`metering.rs`, `providers.rs`, `providers_control.rs`). Their contents later
change outside this increment; the exact root command passes on the final
recheck. The initial default Python invocation lacks `tomllib` under Python
3.9; the installed Python 3.12 runs all seven original assertions successfully.
Both initial failures remain retained.

Updating the source README to describe only the newly proved operations
changes the AIX's source bytes. Therefore the package is regenerated and its
consumer is rerun. The final archive is 26,530,058 bytes with SHA-256
`370beff7b50aed22bfd4e28b579628747c76b90dbda9d21b8c0f12d1ba29b45e`.
The final native owner records exactly that digest. Its copied, freshly built
test runner has SHA-256
`a6324a687934d1398cbfb09c36d263eb236b36900f3613b0353d8c9d4289e0f2`;
the copied SDK builder remains
`2d2784edcf1ca58c6dd755a67eceb971fd227722d2ae67b8f41b14f1d71e7d23`.
Both copies and sampled native sources remain unchanged during the final runs.

A separate native inspection identifies Linux 6.12.76 in the candidate. The
[producer's v5.3.0 release](https://github.com/boxlite-ai/libkrunfw/releases/tag/v5.3.0)
and its [Makefile](https://raw.githubusercontent.com/boxlite-ai/libkrunfw/v5.3.0/Makefile)
declare that version; the release API also matches the prebuilt archive digest
already pinned by `libkrun-sys/build.rs`. This narrows the source-qualification
work. It does not make the existing Windows 6.12.91 provenance document a
corresponding-source proof for this macOS artifact.

[box-native-consumer-results.json](box-native-consumer-results.json) retains
commands, frozen inputs, original/current helper diff, source snapshots, native
reports, archive comparison, build/Clippy/format logs and the initial Python
failure. There is no pending check owned by this increment, no permanent
installation, and no catalog release, TUF update, product tag or publication.

Explicitly not claimed: every Box operation, snapshot/service/network behavior,
ACP/agent sandbox integration, macOS corresponding-source publication
qualification, GUI acceptance, other native targets, migration/publication of
all 49 original IDs, complete mise replacement, release performance or final
archive/sandbox/channel gates. All 24 families and unchecked tasks remain in
the full objective.

## Prune lifecycle reference follow-up

The unchanged native mise binary passes twelve additional fixed prune cases
(89 real commands), including two independently tracked projects, prefix and
ordered requests, configuration changes/removal, preview and confirmation,
config-only cleanup and preservation of a live native executable. The accepted
capture is frozen in `specs/031-cli-mise-replacement/contracts/mise-prune-reference.json`.
The current actual Aster CLI rejects `tool prune --dry-run --json` with exit 2.
No Aster prune parity or T004 completion is claimed.

The retained command, artifact hashes, three rejected fixture attempts and
native startup diagnosis are in `mise-prune-reference-results.json` and
[mise-prune-reference/OUTCOME.md](mise-prune-reference/OUTCOME.md). The next
implementation must track used ACL files explicitly without changing existing
read-only loader APIs, then retain selected and running versions before removal.
