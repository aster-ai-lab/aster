# Goal and completion summary UX

Status: scoped implementation and shipped wire/Chinese Goal PTY validation complete with a local mock model; release readiness remains unproved. Latest retained results are in [paired runtime qualification](../2026-11-03-paired-runtime-qualification/OUTCOME.md).

The user screenshot shows English internal goal states and a Run ID after a Chinese objective, repeated end messages, and a completed Run beside0/1accepted goal steps. The implementation must preserve the distinction between execution ending and goal acceptance, carry the user language through generated planning/tick prompts, and show a concise truthful outcome and next action.

Design references: [Nielsen Norman Group usability heuristics](https://www.nngroup.com/articles/ten-usability-heuristics/) for visible status, familiar language and minimal necessary information; [Microsoft HAX guidelines](https://www.microsoft.com/en-us/haxtoolkit/ai-guidelines/) for relevant context and access to explanations. No new classifier, model request, framework or configuration is planned; reuse the existing reply-language and typed goal pipelines.

The a3s-test skill catalog path is missing and a bounded search of installed skill roots found no replacement. Existing repository PTY end-to-end tests are the validation fallback.

## Implemented behavior (qualification in progress)

- Core publishes the language of a newly authored goal definition before planning. Replaying the same definition does not reset a later user language. Generated goal ticks preserve the user language instead of treating their English control prompt as a new user request.
- ACP renders the actual goal state and accepted plan progress with concise Chinese or English messages. A completed Run does not claim an accepted Goal. Only a changed report is appended after a slice; internal Run IDs, plan revisions and decision codes stay in structured diagnostics.
- Existing advisory settlement feedback is exposed through the Core public API for the exact bound step. ACP may show its unmet criteria in original order; feedback grants no acceptance or permission.
- Ordinary completion guidance now leads with the useful outcome, decisive evidence, remaining work and next action, while preserving user language and avoiding repeated internal status narration.
- Existing shipped PTY coverage now uses a Chinese objective, asserts the actual no-progress stop and 0/1 accepted state, checks the planning language contract, and rejects raw internal IDs/codes or false completion. Existing real-model probes count original slice-admission facts rather than relying on removed UI narration. Models, tasks and acceptance criteria are preserved.

## Actual results so far

- `presentation-red-status.json`: 0 passed, 2 failed before implementation (raw decision copy and missing goal language publication).
- `core-goal-language-status.json`: `cargo test --locked --offline --target-dir target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5 --config profile.test.package.sha2.opt-level=3 -p aster-core --lib agent_api::goal_facade::tests::execution::language:: -- --test-threads=1`: 2 passed, 0 failed. The automatic tick uses actual tool execution and checks both observed model requests.
- `goal-connected-status.json`: 19 passed, 2 failed, 2 ignored in the first connected ACP run. Both new presentation regressions passed; two old English wording assertions were then updated without changing state assertions.
- `acp-unit-status.json`: build failed on a newly introduced ambiguous string conversion in the completion footer; corrected to string literals. No test result claimed from that build.
- `acp-unit-fixed-status.json`: 505 passed, 1 failed, 10 ignored. All goal tests pass. The existing native MCP cancellation case did not reach `tools/call` inside its unchanged 5-second bound; `mcp-start-isolated-status.json` reproduces that timeout alone. After the shorter prompt and version regeneration, `mcp-small-prompt-status.json` passes the unchanged case1/1. Its cause is not isolated; full ACP is being rerun using that exact Cargo-built test executable.
- `core-connected-default-status.json`: 682 passed, 2 failed, 6 ignored. The new response guidance exceeded the unchanged12,000-byte system-prompt limit; it was reduced from1,297 to682bytes without changing the budget/test. The existing state-cycle test hit its3-second deadline during the broad run; the unchanged same executable passes the case alone with exactly9model calls/9physical calls and the original cycle refusal (`state-cycle-isolated-status.json`). No state-cycle implementation or timeout changed.
- `release-preflight-status.json`: all10suites pass,87tests total (dependency7, Studio1, pair delivery21, Linux contracts13, CLI workflow16, Code workflow6, pair identity3, sandbox7, smoke self-check3, governance10). These static/self-check suites do not certify execution from the final archive.
- `cargo fmt --all -- --check` and scoped `git diff --check`: exit0 before the metadata version bump; final rootfmt is queued.
- Candidate package versions are Core/ACP9.1.8 and CLI0.1.8. `version-lock-regenerate-status.json` records successful Cargo regeneration/check; the lock diff contains only these3package-version changes versus its captured preimage. No lockfile was hand-edited. Release note references resolve3/3.
- Three final Core feature-profile connected suites, full ACP retry, clippy/workspace/optional checks, pair build/freeze and shipped PTY qualification remain in progress. Commands and actual results are retained beside this file.

## Explicitly not claimed

- No full UX qualification, shipped PTY success, universal locale coverage or real-model language result yet. Static ACP status copy currently supports Chinese and English; the existing model-language resolver is reused unchanged.
- The existing long-task repair remains active; original frozen task/model/input/limits/graders remain unchanged. No Release, tag or commit created.

## Closeout at the user's request to converge

Implementation is present, but qualification is incomplete; no stable/release-ready claim.

- Final Core focused suites: default300pass6ignored, CLI JSON300pass6ignored, minimal296pass4ignored. The one original manual-projection control outside that focused filter passes separately in each profile; all201original controls are covered. Combined distinct counts are301/301/297 respectively, with overlapping coverage.
- Core and ACP all-target clippy exit0. Existing warnings remain; examples include the pre-existing useless conversion in goal_facade.rs81 and prepare_settlement argument count in settle.rs119. No new broad lint allowance was added.
- Latest full ACP retry:504passed,2failed,10ignored, even with an isolated temporary root. Failures are registered-process cancellation startup and native MCP timeout readiness. Earlier runs and isolated recoveries remain retained; they do not establish a full-suite pass.
- Frozen pair/testers: all5members match `aster-cli-pair-v1:ea527afc2102651e88b57d2c21bb168a0a2e31fe:aarch64-apple-darwin`. Both original task-runner selfchecks pass50each. Qualification snapshot1c898… differs from build snapshot only in use/tool-acquire/prune/process.rs; subsequent source is e2527…. No writer attribution.
- Later-compiled conformance tester embeds e2527… and correctly refuses the ea527…pair (`candidate_build_identity`):13helpers pass,1wire case fails before protocol execution. The identity gate was not weakened. The dependent shipped PTY test was not started.
- Workspace all-target check fails2Studio contract references to missing crate::app_updater in host_family.rs153/224. Optional check and final rootfmt in that pipeline were not started after the failure. An earlier fullrootfmt passed; no final whole-workspace success is claimed.
- No root4/root5 real-model task was started, no supplementary case run, no commit/tag/Release/channel update, and no installed user binary replaced. Version metadata and candidate notes are prepared only.
- The new user-selected Kimi session exposes a separate ordinary-turn convergence issue with0compactions and0goal facts. Its read-only findings are retained in [session analysis](../2026-11-03-session-c48dfb73-loop-analysis/OUTCOME.md). The current compression/summary changes are not claimed to repair that trajectory.

## Shipped runtime follow-up

The later paired-runtime campaign freezes CLI, ACP and all five runners in one unchanged source interval, preserving strict Build IDs and executable digests. Actual ACP conformance passes14/14; the original runner selfchecks pass50/50 in each executable. The shipped wire test passes after its streamed mock is corrected to declare the same synthetic usage as its existing non-streamed replies; the original nonzero usage and state assertions remain intact. Two remaining old empty-goal wording callers are synchronized with the new copy.

The shipped PTY test passes the Chinese Goal creation, planning-language inheritance, truthful unfinished0/1state, single needs-input report, hidden internal IDs/codes, pause/resume/cancel, and all its original launch/resume/command checks. Its first corrected-fixture run timed out before Goal at transcript recovery; an isolated repeat with identical frozen bytes and unchanged deadlines passes. Both are retained, so this is not a claim that startup is stable. Actual binaries use a local mock model; no new GLM long-task campaign or universal locale proof is claimed. The independent native-process unit gate remains unresolved and blocks release/live acceptance.
