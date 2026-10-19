# Spec-Driven Development (spec-kit integration)

This repository applies [github/spec-kit](https://github.com/github/spec-kit)
in adapted form. The house templates, constitution, and commands live
in this tree. They do not call spec-kit's installer.

Reference revision at adoption in the sibling A3S OS repository:
`e1fa857a`. This copy is the Aster adaptation of that integration.

## What was adopted

- The **phase structure**: constitution → specify → clarify → plan →
  tasks → analyze → implement. Grok Build runs the procedures from
  `.grok/skills/speckit-*/SKILL.md`, which read
  `.claude/commands/speckit.*.md`. Codex runs them from
  `.agents/skills/speckit-*/SKILL.md`. Cursor runs them from
  `.cursor/skills/speckit-*/SKILL.md`.
- The **planning surface**: `specs/<NNN-slug>/` holding `spec.md`,
  `plan.md`, `research.md`, `data-model.md`, `contracts/`, and
  `tasks.md` for a bounded epic.
- The **constitution**: `.specify/memory/constitution.md`.

## What was adapted

- **Templates carry this repository's discipline.** The plan
  template's Constitution Check uses the regime of the epic's paths.
  A product-path epic uses hermetic tests. A control-plane epic
  under `apps/cloud` or `apps/api` uses the control-plane regime,
  including dual-adapter proofs.
- **No script dependency.** The commands read
  `.specify/templates/`, `.specify/memory/constitution.md`, and
  `Agents.md` directly.
- **Acceptance stays in `docs/evidence/`.** The spec directory is
  planning. It is not a second acceptance record.

## Grok surface

Slash names use hyphens:

| Phase | Grok command |
| --- | --- |
| constitution | `/speckit-constitution` |
| specify | `/speckit-specify` |
| clarify | `/speckit-clarify` |
| plan | `/speckit-plan` |
| tasks | `/speckit-tasks` |
| analyze | `/speckit-analyze` |
| implement | `/speckit-implement` |

When a procedure names `/speckit.<phase>`, use the hyphen form.
Do not run `specify init` or an upstream installer in this tree.

## Codex surface

| Phase | Codex invocation |
| --- | --- |
| constitution | `$speckit-constitution` |
| specify | `$speckit-specify` |
| clarify | `$speckit-clarify` |
| plan | `$speckit-plan` |
| tasks | `$speckit-tasks` |
| analyze | `$speckit-analyze` |
| implement | `$speckit-implement` |

Resolve paths from the Git repository root. Read the owning tree's
notes, then `Agents.md`, before editing. Enforce the constitution.
Where a shared procedure is less strict, follow `Agents.md`. Keep
analyze read-only. Do not commit unless the user asked. Stage only
this task's files and inspect the staged list before committing.

## Cursor surface

Cursor loads a skill when the user names it. The phase names match
the Grok hyphen form (`/speckit-specify` and the rest). Apply the
same path, constitution, and staging rules as the Codex surface.

## When to use it

- **Bounded epics** that span more than one session: run the full
  phase set. `specs/` is the epic's memory.
- **Single-slice fixes**: tests plus, when the change is recorded,
  `OUTCOME.md`. Do not scaffold a spec directory for a one-commit fix.
- **Design-decision items**: change the behavior with evidence first,
  then amend the constitution.

## Phase-to-house-process mapping

| speckit phase | House process | Artifact |
| --- | --- | --- |
| constitution | constitution + development standards | principles with NON-NEGOTIABLE marks |
| specify | acceptance design | `specs/<NNN>/spec.md` |
| clarify | decisions, including abandoned alternatives | marker-free spec |
| plan | Constitution Check | `specs/<NNN>/plan.md` |
| tasks | task breakdown with test obligations | `specs/<NNN>/tasks.md` |
| analyze | read-only consistency check | findings table |
| implement | code + hermetic tests + evidence | `docs/evidence/...` |

## Non-negotiables the phases cannot waive

Fail closed. Evidence retained. The test obligation is the regime
of the epic's paths. Closed sets. Shrink-only ratchets. One ACL
language. One repository.
Names are not guessed. Meaning is not matched. A phase that needs to
violate one stops and escalates to a constitution amendment with
evidence.
