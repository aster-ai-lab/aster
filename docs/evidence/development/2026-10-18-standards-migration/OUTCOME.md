# Development standards migrated from the sibling a3s repository

## Result

The Aster workspace now has its own development standards, constitution,
spec-kit templates, phase commands, and Grok, Codex, and Cursor skills.

Checked by reading the new files and confirming the sibling sources they
replace as the house copy:

- `docs/development-standards.md`
- `.specify/memory/constitution.md`
- `docs/spec-driven-development.md`
- `.specify/templates/`
- `.claude/commands/speckit.*.md`
- `.grok/skills/speckit-*/SKILL.md`
- `.agents/skills/speckit-*/SKILL.md`
- `.cursor/skills/speckit-*/SKILL.md`

`Agents.md` and the root README point at the standards. `DESIGN.md`
already linked `docs/development-standards.md`; that link now resolves.

## What landed

The section structure and the non-negotiable engineering rules came
from `../a3s`: fail closed, retained evidence, closed sets, shrink-only
ratchets, one ACL language, one repository, names are not guessed,
meaning is not matched, drawn pickers, and dialogs that stay in place.

The control-plane obligations were not copied as rules of this
repository. There is no `apps/cloud` layout, no `#[controller]`
surface, no `CLOUD_MIGRATION_COUNT`, and no PostgreSQL dual-adapter
campaign on ordinary changes. New behavior uses a small hermetic test.
Crate identifiers that start with `a3s-` stay.

## Explicitly not claimed

No root ROADMAP gate row exists for this governance set, so none was
updated. The speckit skills were not executed against a sample epic.
No commit was created.
