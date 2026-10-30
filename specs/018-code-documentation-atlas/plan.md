# Implementation Plan: Architecture atlas

**Branch**: detached | **Spec**: [spec.md](spec.md) | **Input**: [spec.md](spec.md)

**Gate context**: Existing static MDX documentation and README.

## Summary

Extend the current build-time React/MDX renderer with vendored SeeCode 0.6.2 and pinned Code Hike 1.1.0. Extract diagram styles into generated CSS, retain the one external navigation script, and add progressive controls. Documentation uses exact committed source rather than copied pseudocode. README exports share diagram specifications.

## Constitution Check

| Principle | Status | Evidence in the plan |
| --- | --- | --- |
| I. Fail closed | PASS | Reject unknown diagrams and stale source selectors. |
| II. Evidence retained | PASS | Named web evidence directory, command logs and source audit. |
| III. Hermetic tests | PASS | Existing Node/JSDOM and Python web suites. |
| IV. Closed sets | PASS | SeeCode schema validation and fixed authoring names. |
| V. Workspaces / APIs | PASS | No Rust dependencies or APIs changed. |
| VI. Local schema changes | N-A | No persistent schema. |
| VII. Shrink-only ratchets | PASS | Preserve inline-script/style and native-control bans. Asset catalog derives generated SVG names. |
| VIII. Replaceable transports | N-A | No transport changes. |
| IX. One configuration language | PASS | JSON only describes documentation drawings and excerpts. |
| XIII. One repository | PASS | Worktree inside repository; vendored licensed renderer source, no new repository. |
| XIV. Names are not guessed | PASS | Diagram labels are authored; selectors match exact source. |
| XV. Meaning is not matched | PASS | No intent or semantic parser. |
| XVI. Pickers are drawn | PASS | Buttons and native links, no select/datalist. |
| XVII. Dialogs stay in place | N-A | No modal. |

## Project Structure

### Documentation

`specs/018-code-documentation-atlas/`: spec, plan, tasks and research.

### Source Code

| Layer | Path | Changes |
| --- | --- | --- |
| Build | `apps/web/scripts/` | SeeCode integration, source walks, SVG generation. |
| Renderer dependency | `apps/web/vendor/seecode/` | Original MIT sources and provenance. |
| Content | `apps/web/content/docs/{code,acp}/9.1.6/` | Diagrams, walkthroughs and deep internal chapters. |
| Progressive UI | `apps/web/public/docs/{styles.css,navigation.js}` | Local controls and scroll walkthrough enhancement. |
| README | `README.md`, `assets/readme/` | Beginner overview and generated vector figures. |
| Tests | `apps/web/tests/` | Real renderer, selector and browser interaction tests. |

## Research

- `docs_diagrams.mjs` already renders build-time SVG and extracts inline styles. Reuse that boundary for SeeCode rather than adding embedded standalone HTML or iframes.
- `docs_sources.mjs` already verifies unique fragments against git snapshots. Extend its source resolution for code excerpts rather than introducing a second authority.
- `navigation.js` already supplies a single progressive script. Keep this delivery model for stepping and scrolling.
- SeeCode has zero runtime package dependencies. Vendor its unchanged skill source with MIT license and archive hash, keeping regeneration available offline.
- Code Hike explicitly supports build-time highlighting and manual token rendering without React Server Components. Use that API with existing static React rendering.
- `A3S-Lab/stc-answer` 1.2.0 matches the bundled skill. Apply its STC prose rules while honoring the user's existing documentation destination.

## Verification Plan

- Source and diagram unit tests, plus JSDOM interactions using actual generated markup.
- Existing documentation, homepage, canvas, download and cosmic Node tests.
- Existing Python site tests, static validation and staged byte comparison.
- STC lint of newly authored prose; retain findings and explain false positives.
- Inspect exported SVG renders if the available computer surfaces support it; otherwise record the visual QA limitation honestly.
- No Cargo commands because this change has no Rust source. Keep local services stopped. Publish to the existing GitHub Pages workflow after validation, as explicitly requested.


## Release integration

`content/home.html` and `content/download.html` become the authored templates for
public HTML. `content/releases.json` is generated from stable GitHub release assets
using the existing publishers’ exact artifact contracts. The existing Pages
workflow refreshes this snapshot for deployments, while PR tests use the fixed
committed snapshot. The desktop-stable ZIP contract is already declared by
`scripts/desktop_release_manifest.py`. No new distribution repository or runtime
SDK is introduced. Tests run against fixture release metadata and real generated
HTML; the final deployment is checked against fresh public metadata.
