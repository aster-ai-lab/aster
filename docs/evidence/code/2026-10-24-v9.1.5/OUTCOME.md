# Aster CLI 0.1.3 / Core and ACP 9.1.5

## Current published outcome

Published [Aster CLI0.1.3 / ACP9.1.5](https://github.com/aster-ai-lab/aster/releases/tag/aster-cli-v0.1.3) from source220562fe9. Exactly macOS/Linux/Windows archives plus latest.json are uploaded; all four server SHA256 digests, tag commit and real public latest/updater check match. Mac is a local locked release/default-feature build with pinned Rust1.97.1 (951.835s), selected after the remote job exceeded2h; Linux/Windows reuse their successful original CI archives. It is not described as a fully successful three-platform CI run.

Current selected Mac bytes: ACP20 (13 product+7 auxiliary), protocol10, controlled compiler-VM H1 with19 independent audits, actual installer/update5 and public updater check pass. Relevant unchanged Core741 (two existing ignored) remains component evidence; Windows native install/rollback2 passes and Linux glibc2.31/base-library checks pass. Source, binary, manifest and tester fences are unchanged. `published-release.json` and the linked current qualification record bind the exact assets.

A live draft lookup exposed a publisher rerun bug: the unpublished tag REST lookup returned404. The shared script now reads targetCommitish through gh release view. A retained SOLO red control and full green4 tests verify matching-draft reuse and refusal of published/different-source releases. Workflow lint passes with shellcheck/pyflakes disabled. No product source or binary was changed by that pipeline-only correction.

Whole-workspace CI remains red on pinned Studio gaps. The architecture objective is unfinished: arbitrary non-returning callback ownership, full recovery faults and changing-argument/model-derived-output loops remain open, and no real-model Terminal Bench speed/token gain, full130-case, cross-platform long-task/GUI/soak or signed-binary claim is made. The chat-exposed read-only token still needs restriction/rotation.

## Earlier checkpoints retained

The user authorized the existing paired Aster CLI/ACP release channel. Rust SDK packages remain local and non-publishable. This source was prepared in an isolated worktree from bfdfb797de3dc373025c5469dded57be2acb0e91. Primary-worktree Studio, Gateway, Office and terminal feature changes are excluded. Three existing Gateway formatting errors are corrected as whitespace-only changes so the workspace format gate can run; no Gateway behavior is changed.

Current checks: 84 confirmation-related Core tests passed, including four original SDK scenarios (cancelled, approved, rejected, and unconfirmed host cleanup), receiver abandonment and consent identity conversion. The latter two checks are included through explicit test filters. Both isolated CLI/ACP packages pass locked offline checking; dependency boundary 6, paired installer/rollback 10 and static guest-linker 2 tests pass. Scoped CLI/Core/ACP formatting checks pass. The earlier whole-workspace pass claim is corrected below: the exact pinned Studio source fails format traversal. `checks.json` and the retained logs record the completed scope.

Commands: `cargo test --locked --offline -p a3s-code-core --lib -- confirmation approval_identity_tests an_abandoned_receiver_cannot_accept_or_publish_a_late_approval --nocapture --test-threads=1`; `cargo check --locked --offline -p aster -p a3s-code-acp`; `cargo fmt --all -- --check`; `python3.13 tests/workspace_dependencies.py`; `python3.13 tests/cli_pair_delivery.py`; `python3.13 tests/musl_linker_flags.py`.

The final isolated paired candidate passed actual ACP/conformance and compiler-VM qualification. Four platform archives and final publication are still pending. The workflow assembles a draft bound to its actual build commit; a later check publishes only the qualified release.

## Subsequent qualification

The isolated release source passed 741 relevant Core tests (two existing ignored), actual ACP20 (13 workflows plus7 auxiliary) and protocol10. Controlled H1 first failed before ACP launch because its owned artifact parent was absent; preparing that parent exposed a separate scripted-model assumption that the provider ID must remain `initial-check`. The model fixture now correlates the exact admitted command and its adjacent native result through their opaque IDs. Two positive/negative fixture tests pass. Production completion and verification gates are unchanged. A new paired build and actual qualification are required because the source identity includes fixture bytes.

The first four-platform CI run40092859072 stopped before compilation because its required private-Studio token was missing. A single-job macOS rerun confirmed this prerequisite. Dedicated deploy-key creation was refused by the repository policy; no deployment key was created or policy changed. The user requested automatic configuration and explicitly selected one-year validity for a restricted Studio-read token. Credential configuration and publication remain pending.

## Explicitly not claimed

The architecture goal remains active. Arbitrary unreturned store/export callbacks retain their original owner; their cancellation/settlement contract is unfinished. Crash/recovery faults, changed-argument or model-derived-output loops, complete 130-case coverage, GUI/multi-platform runtime/soak qualification and real-model Terminal Bench speed/token improvements are not proved. Current tests use controlled models and synthetic usage. No public release has been made at this checkpoint.

## Final local qualification and CI checkout correction

The final paired build `3c601c086ffef3b3e86a13c0f71d19b49abc3dae` passed actual ACP20 (13 product workflows plus7 auxiliary), protocol10 and controlled H1 with19 independent checks. The H1 workflow took34,453ms with8 tool calls and7 original approval requests; its18 synthetic tokens are not real billing. Binary, manifest, tester and compiled-source guards remained unchanged. Original Goal facts show6 created goals,10 admitted and10 settled slices,4 completed goals and2 intentionally stopped cases. No slice remains pending in that controlled cohort. `final-qualification.json` records the scope.

The user supplied a token; `STUDIO_READ_TOKEN` was saved and its GitHub metadata verified without persisting its value locally. New run40101228548 used source65ab9bb9 and passed Studio checkout on macOS, Linux and Windows. Kylin failed on original `fatal: detected dubious ownership`; the common resolver now grants per-command trust only to the explicit `GITHUB_WORKSPACE`. One hermetic test executes the original action shell, reproduces the failure, proves the exact workspace passes, rejects a different workspace and proves no global trust setting changes. The production Code input remains unchanged.

The supplied token currently has organization-wide code/metadata read-only access and was exposed in chat. Restriction to Studio and credential rotation remain required; no secret value is retained in these records. Four-platform release compilation and publication remain unfinished.

The Kylin glibc probe now explicitly chooses Bash for its `pipefail` script. [GitHub documents that container run steps otherwise default to sh](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/run-jobs-in-a-container). Workflow YAML and all11 non-PowerShell run scripts pass syntax checks.

## Older Git counterexample and complete CI trust scope

Run40102189209 retained a counterexample: Ubuntu20 Git2.25.1 still refused the command-line `safe.directory` override that passed on the local Git. This is not counted as a successful Kylin build. The paired source-identity build scripts also execute Git, so a resolver-only exception would leave later compilation broken. The shared checkout action now adds only the exact non-empty `GITHUB_WORKSPACE` to the ephemeral runner global Git configuration, then uses that checkout explicitly. No wildcard or user-machine global configuration is changed. The updated hermetic case proves original refusal, original revision output, an unrelated current directory, subsequent build-style Git reads, and refusal of a missing or different workspace while preserving unrelated explicit entries. The old logs and local-only passing trial are retained. Actual older-Git CI and final publication remain pending.

## Protocol compiler and workflow validation

Run40103045893 passed Studio checkout, the glibc2.31 guard and Rust installation on Kylin, then the distribution protoc rejected existing proto3 `optional` fields. The container now installs official protoc29.3 (matching the existing Windows version), checks its pinned archive SHA256 and prints its version before compilation. The actual downloaded ELF passes the repository glibc2.31 check using LLVM readelf. Required CMake and unzip build tools are explicit. Native container execution remains a CI check.

The push-triggered Rust workflow had not started tests: one newly added single-line command ended filters in `:: `, which YAML treated as a mapping. That was an implementation error in the CI definition, distinct from the completed local tests. The command now uses a block scalar. Actionlint1.7.12 reproduces the original failure and validates both changed workflows; shellcheck and pyflakes were disabled. The verified linter binary stays in ignored build tools and adds no product dependency. Final four-platform release and the broader architecture objective remain unfinished.

## Corrected workspace scope

Actual Rust CI40104203537 compiled the static guest agents, then both native jobs failed format traversal because pinned Studio92cec9d references absent `app_updater/os_feed_tests.rs`. A local SOLO `cargo fmt --all -- --check` now reproduces the same failure on the same clean Studio HEAD; the file is absent from its tree. The earlier whole-workspace formatting pass record was not valid for this exact pin and is corrected here and in `checks.json`. CLI/Core/ACP scoped formatting and Core optional `advanced-harness,deep-live` compilation pass. Workspace checks after format did not run.

Both desktop renderer jobs separately fail on the missing parent `packages/intelligent-ui/src/jsonRender.ts` imported by that Studio pin. `git diff` proves Studio, Office and intelligent-ui sources identical to basebfdfb797; the pinned alias points at a file absent in that baseline. No concurrent frontend work or private Studio source file is imported into the release. `workspace-limits.json` records the blocked scope. These failures are not waived or presented as passing; the CLI/ACP release has independent component tests and builds and contains no desktop renderer.

Four-platform release run40104259230 remains on frozen sourcee2d2f6224; these evidence-only corrections do not change its product or dependency inputs. Final release archive checks and public publication remain pending.

## User-directed standard-platform release

The user explicitly removed a special Kylin CLI version and called out duplicate CI work. The release now has exactly macOS/Linux/Windows, with one Linux x86_64 glibc2.31 build. Installer and compiled updater select the standard Linux archive; the retired platform and direct URL are refused. There is no legacy Kylin alias or extra published archive. Published CLIv0.1.1 predates this paired updater module, so no published Kylin updater channel is being migrated.

Checks:13 Linux packaging/installer contracts,10 pair delivery/rollback checks,2 actual draft-assembly shell controls,2 Rust updater tests and actionlint pass. The publisher controls bind reused artifacts to their original commit and refuse missing or retired extra archives before any release creation. Quick product preflight precedes native compilation; whole-workspace format now runs before expensive static-guest builds. The existing artifact_run_id path promotes completed archives without rebuilding them.

Compiled updater behavior changed, so historical pair3c601c08 and run40104259230 cannot be presented as the final standard-platform binaries. A new three-platform build and actual archive qualification are required. Core production inputs are unchanged; its prior741-test regression remains scoped to that unchanged component. Whole-workspace Studio failures and credential rotation remain open.

## Main integration

The release is integrated onto main3aa265e06 without replacing its newer Studio, Power, GPU or Gateway work. The shared checkout resolver retains both pinned submodules and exact CI workspace trust. The regression checks both original gitlink outputs. Locked CLI/ACP checking passes in2m56s after fetching an uncached pinned dependency; scoped formatting, dependency6 and checkout1 pass. Actionlint passes with shellcheck and pyflakes disabled. The integrated Studio pin still references the absent updater test module; whole-workspace format remains red. `main-integration.json` separates this integration from frozen release build220562fe9, whose actual three-platform archive qualification remains pending. An unreachable retired Kylin alternative was also removed from the packaging shell; the existing first platform boundary already refused it.
