# Implementation Plan: Living knowledge Wiki

**Branch**: main, no branch requested. **Spec/Input**: [spec.md](spec.md).
**Gate context**: Product regime, personal knowledge maintenance.

## Summary

Extend OKF's existing isolated staging into a durable incremental maintenance
and review workflow. Reuse Aster agent sessions and document-change events.
Replace the implementation-oriented compile dialog with a knowledge workspace.

## Constitution Check

| Principle | Status | Planned evidence |
| --- | --- | --- |
| I Fail closed | PASS | Native bounded manifest/path/revision validation |
| II Evidence retained | PASS | Desktop living-wiki OUTCOME and commands |
| III Hermetic tests | PASS | OKF fixtures, renderer IPC fixtures, permission gate |
| IV Closed sets | PASS | Native enums with parse/as_str and strict manifest decoding |
| V Workspace/APIs | PASS | Root workspace, legacy Rust APIs retained |
| VI Schema owner | PASS | OKF-owned filesystem publication data |
| VII Ratchets | PASS | No allowlist growth |
| VIII Transports | N-A | Existing agent transport |
| IX ACL | PASS | No product configuration added; run manifests are output data |
| XIII Repository | PASS | Studio sources stay inside its existing private submodule |
| XIV Names | PASS | Explicit model-authored page/source records |
| XV Meaning | PASS | Typed model change/conflict declarations; existing pages always reviewed |
| XVI Pickers | PASS | Shared UI controls, no native select |
| XVII Dialogs | PASS | Main workspace; any confirmation uses existing in-place controls |

## Source Plan and Prior Art

| Layer | Paths | Reuse/change |
| --- | --- | --- |
| OKF | `crates/okf/src/personal/maintenance.rs` and sibling tests | Existing personal vault, generation, atomic write, OKF inspection |
| Host | `apps/studio/src-tauri/src/okf_compile.rs`, permissions, lib registration | Existing spawn_blocking/Tauri seam |
| Runtime | `knowledgeMaintenance.ts`, `windows/main/KnowledgeMaintenanceRuntime.tsx`, MainApp composition | Existing compile run driver, agent sender, document invalidation stream |
| UI | KnowledgeNotesPage and maintenance workspace | Existing source editor, concept reader, design system |
| Locale/tests | Existing knowledge test tree and catalogs | Shared fixture boundaries; every changed locale value translated |

## Product and Collection Contract

Owner is the canonical personal `wiki/local` vault; identity is its canonical
path plus exact content generations. Knowledge pages support read, proposed
replacement and explicit accept/keep-current; original-source deletion is not
a maintenance action. A job is a running workload, and activity is a ledger.
Source selection is current page only (20 rows); source search, state filter,
stable relative-path order and page change reset selection. Native snapshots
are bounded and provide honest limit/failure states. Review acts on one exact
candidate at a time, rechecking revisions. Missing native authority disables
writes with a recoverable error. Scope changes clear old identity's view.

## Visual Plan

Use the DESIGN.md palette: Page #ffffff, Ink #171717, Body #3f3f3f,
Muted #8a8a8a, Field #f5f5f5 and Hairline #efefef. Ubuntu/user font,
24px page titles, 13px rows and 12px metadata. Left-aligned, quiet record rows.
No added decorative palette, metric-card grid or progress simulation.

```text
Knowledge navigation | Knowledge maintenance                 [Back to reading]
Sources / Wiki       | [Changes] [Sources] [Activity]      [Update pending]
                     | actual run status / review reason
                     | change list | before / proposed text
                     |             | linked sources      [Keep] [Accept]
```

Review strips implementation metadata from the reading view. Narrow widths
stack detail and list, preserve keyboard focus and keep all actions reachable.
Immutable cited-source snapshots, Source-to-Wiki links and actual change history provide the useful hierarchy;
large file counts and ambiguous compile/rewrite buttons are removed.

## Research and Verification

Karpathy's [LLM Wiki idea](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)
separates raw sources, maintained interlinked pages, index and chronological log.
It keeps source ingestion, cited inquiry and health checks in one evolving
knowledge loop. Aster already owns source storage, agent sessions and reading;
the missing pieces are incremental durable ownership and reviewable publication.

Run focused OKF/host/renderer suites, strict permission checks, changed-file
format/lint/type checks, repository checks and a renderer build. Record unrelated
working-tree failures honestly. Do not invoke providers against user data.
