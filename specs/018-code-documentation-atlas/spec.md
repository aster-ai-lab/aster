# Feature Specification: Aster architecture atlas and source walkthroughs

**Feature Branch**: none requested; isolated detached worktree

**Created**: 2026-10-28

**Status**: Complete

**Input**: Deepen a3s-code-core and a3s-code-acp documentation under the repository engineering standards and A3S-Lab/stc-answer. Use SeeCode for README and documentation diagrams, including animation and exploded views. Use Code Hike for interactive source walkthroughs. Explain internal prompts, long-running tasks, recovery and completion to both beginners and engineers.

**Gate context**: Existing `apps/web` static MDX site and root README. This user-requested documentation epic adds no Rust runtime capability and makes no measured quality claim.

## User Scenarios & Testing

### User Story 1 - Follow a task without knowing Rust (Priority: P1)

A reader starts at the README, identifies the interface, protocol adapter, runtime and workspace, then follows one task through an animated diagram.

**Why this priority**: A diagram must explain responsibility before introducing implementation names.

**Independent Test**: Web DOM tests exercise diagram next, previous, reset and keyboard controls, isolation between figures, and the complete static fallback.

**Acceptance Scenarios**:

1. **Given** JavaScript is unavailable, **When** the reader opens a chapter, **Then** every diagram and explanation remains visible.
2. **Given** reduced motion is selected, **When** the reader advances, **Then** the diagram changes state without automatic animation.
3. **Given** a narrow viewport, **When** the reader opens a diagram, **Then** its labelled scroll region preserves the drawing and controls.

### User Story 2 - Trace a design claim into source (Priority: P1)

An engineer follows prompt assembly, fact-driven execution, Goal ticks, completion gates and ACP event translation into exact committed source excerpts.

**Why this priority**: Detailed prose needs verifiable mechanisms and failure conditions.

**Independent Test**: The real documentation builder refuses missing or ambiguous source fragments, renders Code Hike tokens from the pinned revision, and preserves original line numbers.

**Acceptance Scenarios**:

1. **Given** a walkthrough step, **When** it is selected, **Then** its explanation, code excerpt and source link agree.
2. **Given** a stale snippet selector, **When** docs build, **Then** the build fails with the source identity.

### User Story 3 - Maintain the published atlas (Priority: P2)

An author changes diagram specifications and MDX, then regenerates all assets with the existing build command.

**Independent Test**: Deterministic build/check, existing site validation and staging check every registered route and local asset.

## Edge Cases

- Refuse unknown diagram types, unsafe names, missing source evidence and ambiguous excerpts at build time.
- Preserve ordinary reading when IntersectionObserver or storage is unavailable.
- Refuse scripts, inline styles and external font requests in generated documentation.
- Keep historical 9.1.5 prose and diagram sources unchanged.
- Describe uncertain external effects as unresolved, rather than treating log replay as execution.

## Requirements

### Functional Requirements

- **FR-001**: Replace current architecture illustrations with source-authored SeeCode diagrams. Refuse unverifiable runtime claims.
- **FR-002**: Supply overview, architecture, sequence, state and exploded diagram forms. Explain that exploded software layers are conceptual, rather than physical deployment.
- **FR-003**: Add Code Hike source walks for prompts, execution, Goal control, completion and ACP translation. Refuse source selectors without exactly one match.
- **FR-004**: Explain responsibility, admission, execution order, state, failures and investigation steps for each internal topic. Do not present illustrative traces as observed model runs.
- **FR-005**: Preserve the static site, existing navigation and source pin. Refuse browser dependencies on private Studio source or runtime network services.
- **FR-006**: Generate README SVG assets from the same diagram data. Do not hand-edit generated assets.
- **FR-007**: Apply STC sentence and terminology rules to newly authored Chinese prose. Preserve code and identifiers exactly.

### Key Entities

- Diagram: SeeCode specification, explanatory steps and source selectors owned by `apps/web/content/docs`.
- Source walk: ordered explanations and exact excerpt selectors from the version's committed source revision.
- Build evidence: commands, counts, provenance, review findings and delivery boundaries in `docs/evidence/web/2026-10-28-code-documentation-atlas/`.

## House Checklist

- [x] New web behavior has hermetic DOM and builder tests planned.
- [x] New authoring names are closed sets or validated mechanical identifiers.
- [x] No Rust `unwrap()` or `expect()` added.
- [x] Internal A3S dependencies unchanged.
- [x] No product configuration added; diagram JSON is build-time authoring data.
- [x] Evidence target named above.

## Success Criteria

- **SC-001**: Every current-version architecture diagram uses a valid SeeCode specification with a readable static fallback.
- **SC-002**: Seven source walkthrough topics build from committed source and expose actual line anchors.
- **SC-003**: New focused tests and existing web suites pass; deterministic build and staging pass.
- **SC-004**: Newly authored prose has zero confirmed STC mandatory findings after source-aware review.

## Assumptions

- Keep 9.1.6 pinned to `8729d7c84a2d887c61dc206ba47bf8a63425e768`; later unrelated WIP is not documentation authority.
- Preserve the stopped local service. Later user requests authorize integrating the homepage preview, exact alternating taglines, Magpie detail refinement and release-aware downloads.
- Use an isolated worktree because the main checkout has other sessions' edits.

## Explicitly Not Claimed

- Publication and scoped commits are now explicitly authorized by the active user goal. No Rust runtime change or external model evaluation is claimed.
- No promise that every reader understands every chapter; acceptance checks concrete explanation paths and interaction behavior.


## Authorized scope additions

The user subsequently requested alternating “Aster 是你需要的全部” and
“Aster 让想法落地”, detail refinement using `https://usemagpie.ai/zh/`,
and publication to GitHub Pages with latest per-platform GitHub Release downloads.

### User Story 4 - Download the latest published installer (Priority: P1)

Select the latest stable Aster artifact separately for each product/platform,
excluding SDK-only releases, drafts and prereleases. A new release refreshes the
existing Pages pipeline from main. The browser needs no release API access.

Independent proof: `apps/web/tests/releases.test.mjs`, six real asset HEAD checks,
and comparison of deployed HTML URLs to fresh GitHub release metadata.

### Additional requirements

- **FR-008**: Alternate both exact taglines with static reduced-motion output.
- **FR-009**: Refine typography, navigation and feedback while retaining the original layout and 3D space direction.
- **FR-010**: Refuse missing, incomplete or unqualified release assets; generate URLs, labels and CLI install versions from one snapshot.
- **FR-011**: Refresh Pages after published release changes and existing release workflow success; check out main rather than a historical release tag.
- **FR-012**: Commit only this work, push to main, verify successful Pages deployment and compare live download URLs to current stable releases.
