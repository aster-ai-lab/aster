# Studio land discipline gates

Verified on 2026-11-07, macOS arm64. No commit was created.

## Result

Parent and Studio checkouts now refuse the three rollback paths that kept
undoing Studio optimizations: dirty-tree resets, ancestor gitlink bumps, and
stash/WIP restore delivery subjects. Pinned perf/layout renderer contracts are
wired into `just` and CI.

## Gates

| Gate | Entry |
|------|--------|
| Dirty Studio worktree | `python3 scripts/studio_land_guard.py dirty` |
| Ancestor pointer rollback | `python3 scripts/studio_land_guard.py pointer` |
| Stash/WIP restore subjects | `python3 scripts/studio_land_guard.py messages` |
| Pre-bump bundle | `just studio-land-check` |
| Perf/layout contracts | `just studio-contracts` / `pnpm run test:contracts:perf-layout` |

Agent rule: `.cursor/rules/studio-land-discipline.mdc`.

## Verification

```bash
python3 tests/studio_land_guard.py
just studio-contracts
```

- Hermetic guard suite: **5/5 passed** (rollback refuse, forward bump, dirty,
  restore subjects, contracts manifest presence).
- Perf/layout contracts: **9 files, 185/185 passed**.
- `git diff --check` on parent paths touched by this change: passed.

## Explicitly not claimed

- Studio submodule commits for the new contract script are not pushed; the
  parent still has a dirty `apps/studio` worktree from concurrent local work.
- Full `pnpm test`, full Studio lint, release packaging, and a live GUI e2e
  were not run for this process change.
- Existing historical restore commits already behind the pinned tip are not
  rewritten; the messages gate only refuses landing a tip range that still
  contains those subjects.
