# Feature Specification: Evaluation-driven delivery hardening

**Feature Branch**: Existing branch; no branch or commit requested.
**Created**: 2026-10-23
**Status**: Ready for implementation
**Input**: 根据开发规范按规划完成相关优化和端到端测试。
**Gate context**: Product regime. Implement the seven work packages agreed after the 2026-10-23 background-CUA evaluation. Baseline: 11/28 completed, seven passed, three model failures, one Studio/Office presentation failure. Model and application failures can coexist. Baseline evidence remains immutable.

## User Scenarios & Testing

### User Story 1 - Trust the delivered file (Priority: P0)

Users see the numbers actually stored in a workbook, retain attachment names, and open only available workspace products.

**Independent Test**: Office spreadsheet import/display fixture; Studio attachment and product projection tests; capability-scoped file metadata/open tests.

**Acceptance Scenarios**:
1. Given A07 D2 is numeric 12000, importing and rendering shows 12000 in both grid and formula bar without converting the value to text or changing the original file.
2. Given Chinese, spaced, long, multi-dot or duplicate attachment names, staging retains the supplied complete name and byte digest; malformed names cannot escape the inbox.
3. Given prose, tracebacks, deleted scratch files or inaccessible paths, the product list refuses to advertise them as available deliverables. Verified files remain available while a turn waits for input.

### User Story 2 - Read sources without repeated environment experiments (Priority: P1)

ACP reuses the native Office engine for bounded source reads. Execution facts distinguish the host from the actual guest, persistent workspace from temporary storage, and available from unavailable capabilities.

**Independent Test**: ACP fixture reads with document/sheet/page positions and bounds; sandbox metadata and companion availability tests.

**Acceptance Scenarios**:
1. DOCX and XLSX reads return source locations and typed values without Python packages or external programs. PDF text/page-image capabilities are explicit; absent OCR refuses an OCR claim rather than fabricating text.
2. Source reads refuse denied/outside-workspace paths, symlink escapes, malformed or oversized input and unbounded output.
3. Missing sandbox companions fail before command dispatch with the owning reason. The model receives the actual guest shell and temporary-directory lifetime, without permissions being widened.

### User Story 3 - Recover correctly and make supported claims (Priority: P1)

Tool schemas match execution. Approval stays bound to the exact admitted invocation. Deterministic checks support date and sequence calculations; models resolve semantic conflicts and remain responsible for prose interpretation.

**Independent Test**: Core schema/approval tests, ACP calculation tests and replay fixtures, followed by live model regression.

**Acceptance Scenarios**:
1. A valid unchanged approved call runs; denied or rewritten arguments remain refused or require their own approval. Invalid patches never partially write. Reproduce repeated heredoc refusals before deciding whether a product fix is justified.
2. Date calculations have an explicit endpoint-counting convention; sequence checks return every decrease, not a guessed business conclusion. Missing/invalid values are not silently converted to zero.
3. A01 date claims, A02 department claims and list-13 conflicting constraints are checked against independent controls; no case-specific phrase matcher or answer is embedded in product code.

### Edge Cases

- Numeric zero, negatives, decimals, booleans, cached formulas, dates, explicit hidden number formats and empty cells remain distinct.
- Artifact publication and open are both capability-scoped; denied and missing share the not-found contract.
- Cleanup and renames invalidate current availability without deleting historical tool evidence.
- A rejected optional cleanup is disclosed separately from content correctness. A settled request is not automatically an accepted user outcome.
- Concurrent unrelated changes are preserved. New candidate binaries never silently replace the paused baseline run.

## Requirements

### Functional Requirements

- **FR-001**: Office MUST render General numeric values correctly and MUST refuse to invent missing values or flatten typed data to strings to mask the defect.
- **FR-002**: Studio MUST preserve the supplied complete attachment name and bytes and MUST refuse unsafe inbox destinations rather than guessing an extension from MIME.
- **FR-003**: Studio MUST derive deliverable candidates from explicit tool/publication data, validate current workspace access and existence, and MUST refuse promotion of prose or shell diagnostics into deliverables.
- **FR-004**: ACP MUST provide bounded Office source reads through the existing Rust Office API, and MUST refuse unsupported, inaccessible, malformed or oversized reads with explicit limits and source locations.
- **FR-005**: Execution metadata MUST state actual backend shell, workspace persistence and capability availability; unavailable execution MUST fail closed without native fallback or network expansion.
- **FR-006**: Advertised schemas and approval bindings MUST match execution, and MUST refuse unknown arguments, invalid diffs and unapproved effective arguments without mutation.
- **FR-007**: Existing output checking MUST support the required date/sequence calculations with closed input operations and MUST refuse ambiguous/invalid numeric or date input. Semantic meaning stays with the model.
- **FR-008**: Verification MUST separately record model content, tool execution and application presentation, preserving failures and evaluator interventions. Tool success MUST NOT stand in for task correctness.
- **FR-009**: Candidate evaluation MUST freeze source/binary/renderer/sandbox/model identities, keep answers and evaluators out of model input, and preserve original inputs and baseline evidence.

### Key Entities

- Attachment identity: existing Studio file metadata and transport parts.
- Published artifact: existing ACP verification metadata and workspace file capability.
- Source read: ACP projection of native Office data with location and bounded output.
- Calculation: explicit typed output_check operation, not a semantic classifier.
- Evaluation attempt: versioned evidence with independent content/execution/presentation outcomes.

## House Checklist

- [x] Each behavior task names its owning hermetic test in tasks.md.
- [x] New closed operation sets use explicit parsing and stable serialized names.
- [x] No new production unwrap/expect without an infallibility argument.
- [x] Internal A3S dependencies remain local; no new SDK bindings or workspace.
- [x] No new product configuration format; existing ACL governs authority.
- [x] Evidence target: docs/evidence/code/2026-10-23-evaluation-hardening/OUTCOME.md.

## Success Criteria

- **SC-001**: All new focused tests and applicable workspace/format/ratchet checks pass, or an unrelated existing failure is independently demonstrated and retained as an unresolved gate.
- **SC-002**: Background native CUA proves A07 numeric display, filename preservation and available product open/download on the built candidate.
- **SC-003**: A07, A01, A02 and list-13 run three fresh same-model candidate attempts each; the seven prior passing cases receive regression and the 17 unfinished cases receive actual end-to-end evaluation. Each result has an independent verdict. Any remaining model failure stays explicit rather than being relabeled as an SDK pass.
- **SC-004**: A02/A07 median tool counts target a 50% reduction from 56/28 respectively without forced call caps or quality loss. Report measured results rather than assuming the target was met. Separate model/tool/approval wait time where authoritative data permits.

## Assumptions

- The new goal authorizes implementation and resumption of end-to-end work. The original A08 attempt remains intact; candidate runs receive distinct version/attempt identities.
- Source-only changes in the private Studio submodule remain inside that submodule. No commits or pushes are requested.

## Explicitly Not Claimed

- Structural artifact validation proves neither semantic truth nor universal model reliability.
- Three live repeats are a regression sample, not a statistical reliability certification.
- No new OCR engine, parser platform, language binding, native fallback, permission bypass, model substitution or answer-specific production rule.
- Missing tools or correctly refused calls are not automatically SDK bugs.

## Recovery note — 2026-10-23

The user authorized restoration after external root restore/clean removed implementation and evidence. This document was recovered from the original task patch. Prior baseline screenshots/logs are currently missing; historical summaries are not current proof. All implementation gates require new verification. Retained Studio changes and original evaluation inputs are preserved.
