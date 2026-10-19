## Guiding Principles (MUST FOLLOW)

### Mindset

How to approach any coding task in this repo.

#### Think Before Coding

- State assumptions explicitly. If uncertain, ask before implementing.
- When multiple interpretations exist, surface them — do not pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what is confusing. Ask.

#### Simplicity First

- Write the minimum code that solves the problem. Nothing speculative.
- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that was not requested.
- No error handling for impossible scenarios.
- If you wrote 200 lines and it could be 50, rewrite it.
- Inline comments cap at 2 lines. Needing more means the code is a patch — fix the implementation instead of narrating it. Say *why*, never restate *what*; no changelogs, no rationale essays, no pasted chat/review replies. (Doc comments on an exported API — TSDoc `@param`/`@returns`/`@deprecated` — are documentation, not narration, and are exempt.)

#### Surgical Changes

- Touch only what the task requires. Do not "improve" adjacent code, comments, or formatting.
- Do not refactor things that are not broken.
- Match existing style even if you would do it differently.
- If you notice unrelated dead code, mention it — do not delete it.
- Remove imports / variables / functions that **your** changes orphaned. Leave pre-existing dead code alone unless asked.
- Every changed line must trace directly to the user's request.

#### Goal-Driven Execution

- Convert tasks into verifiable goals before coding:
  - "Add validation" → "Write tests for invalid inputs, then make them pass."
  - "Fix the bug" → "Write a test that reproduces it, then make it pass."
  - "Refactor X" → "Ensure tests pass before and after."
- For multi-step tasks, state a brief plan with explicit verification per step:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
```

### Operational Rules

Project-specific tools, paths, and conventions.

- **Keep it clear**: Write code that is easy to read, maintain, and explain.
- **Read local READMEs first**: Before editing code in a directory, check for a `README.md` in that directory (and its parents) and read it — these files capture local conventions, invariants, and entry points that aren't obvious from the code alone.
- **Fix upstream, don't hack downstream**: When a new feature hits an existing module's limitation, flag the upstream improvement for the user's decision before proposing a downstream workaround.
- **Library-first, custom-last**: Before writing custom code, check library/framework docs for built-in options or existing solutions. Write custom code only when no adequate alternative exists.
- **Build with Tailwind CSS & Shadcn UI**: Use components from `@a3s-lab/ui` (the OS repo `packages/ui`, Shadcn UI + Tailwind CSS) for every new UI component.
- **Log centrally**: Route all logging through `loggerService` with the right context—no `console.log`.
- **Access paths centrally**: Desktop filesystem paths live in the Tauri host (`src-tauri`). Do not invent an Electron `application` singleton or call `app.getPath()`.
- **Check what you changed, not the whole repo**: for code, run `pnpm lint` (it covers format + typecheck + `i18n:check`) plus the tests covering your change — per-project wrappers (`pnpm test:renderer <file>`, `test:aicore`, `test:shared`, `test:pkg:ui`, `test:scripts`) or `pnpm exec vitest run <file>` for a few files; full `pnpm test` only when the change is broad or you can't name the affected tests. Never use `pnpm test <path>`: the script chains several vitest invocations with `&&`, CLI args reach only the last one, and earlier projects run their full suites unfiltered. Docs/markdown-only edits need just `pnpm docs:check` (links + structure + frontmatter + index). CI runs the full gate; your job is to not obviously break it.
- **Write conventional commits**: Commit small, focused changes using Conventional Commit messages (e.g., `feat(data-api):`, `fix(lifecycle):`, `refactor(quick-assistant):`, `docs(testing):`, `chore(deps):`, `test(window-manager):`). Scope must be a specific kebab-case module, never generic like `main` — when `git log` conflicts with this rule, this rule wins.
- **Sign commits and sign off**: Every commit must be both cryptographically signed and DCO-signed off. Use `git commit -S --signoff` (not `--signoff` alone), verify the commit object contains a `gpgsig` header with `git cat-file commit HEAD`, and verify the pushed PR commits show `Verified` on GitHub.
- **Target the right branch**: `main` is the default branch for all active development — submit features, refactors, optimizations, and fixes here.

## Development

### Commands

Run `pnpm install` first (Node and pnpm versions are pinned in `package.json` — let it enforce them). For every other script, read `package.json` — the ones you must know:

- `pnpm lint` — oxlint + eslint fix + typecheck + i18n check + format (writes files)
- `pnpm test` — run all Vitest tests
- `pnpm format` — Oxfmt format (write mode)
- `pnpm docs:check` — the docs gate (`check-links` + structure closed-set + frontmatter/`sources` existence + generated-index freshness); the only thing `build:check` adds over `lint` + `test`. Run it for docs/markdown edits instead of the full gate. Docs under `docs/references/**` and `docs/contrib/**` carry `description`/`sources` frontmatter; `docs/README.md` is generated — edit frontmatter and run `pnpm docs:index`, never the index by hand.
- `pnpm build:check` — `lint` + `docs:check` + full `test`, i.e. the whole gate in one command. Worth it for broad or risky changes; for anything narrower run the piece that matters. If it fails on i18n sort, run `pnpm i18n:sync` first; on formatting, run `pnpm format` first; on broken doc links, fix the link.
- `pnpm test:lint` — the CI-equivalent lint gate: Oxlint errors and warnings block CI (`--deny-warnings`); ESLint errors block CI while its warnings remain non-blocking.

### Testing

- Tests run with Vitest 5 (see `vitest.config.*` for project setup).
- **No behavior-pinning tests**: a test whose only assertion records what the code currently does — a snapshot of whatever came out, `toHaveBeenCalled` on a mock, an expected value re-derived the way the implementation derives it — has zero value. It cannot fail for a real reason, it breaks on every refactor, and it certifies existing bugs as "expected". Assert the contract instead: real input → the outcome the feature promises, plus the failure and edge cases. Before writing a test, state the bug it would catch; if you cannot, do not write it. **The existing suite is full of these** — delete the ones in a file you are already editing rather than keeping them green; a repo-wide purge is its own task, not a side effect of an unrelated PR.
- **Frontend Tests — MUST READ**: [Frontend Testing Guidelines](docs/references/testing/frontend-testing.md).
- **Test Mocking**: Use the unified mock system — do NOT create ad-hoc mocks for services or data layers. See [tests/__mocks__/README.md](tests/__mocks__/README.md) for available mocks, usage patterns, and best practices.

### Patched Dependencies

Before upgrading any dependency, check `patches/` for custom patches.

Some families only work on one shared release: every `@tiptap/*` package (except `@tiptap/y-tiptap`), and `react` with `react-dom`. Upgrade a family as a whole — `package.json`, the `pnpm-workspace.yaml` overrides, and the patch file names together — then run `pnpm deps:check` and a renderer build (`rsbuild build`). A passing `pnpm install` and Vitest run do not prove the bundle links.

### Launching Studio

`just studio` owns the tenant's single window: each launch stops the running bundle for that tenant, and the window needs the client server on 127.0.0.1:4318 that `scripts/studio-shell.sh` starts. Agents testing a build use `just fork <slug>`. The tenant is required. That opens a separate window and leaves the user's window running. Do not `kill` the user's bundle or run `src-tauri/target/debug/Studio` directly.

## GitHub

### Pull Requests

Use the `gh-create-pr` skill. Fallback: read `.agents/skills/gh-create-pr/SKILL.md` directly.

### Code Review

When reviewing a GitHub PR, do NOT run `pnpm lint` / `pnpm test` / `pnpm format` locally — its CI already ran them; inspect via `gh` instead.

### Issues

Use the `gh-create-issue` skill. Fallback: read `.agents/skills/gh-create-issue/SKILL.md` directly.

## Conventions

### TypeScript

- Cross-process types belong in `src/shared/`; renderer-only shared types in `src/renderer/types/` (see [Shared Layer Architecture](docs/references/architecture/shared-layer.md)).

### Naming Conventions

**MUST READ**: [docs/references/architecture/naming-conventions.md](docs/references/architecture/naming-conventions.md) — files, directories, identifiers, and singular/plural rules.

### Logging

```typescript
import { loggerService } from "@logger";
const logger = loggerService.withContext("moduleName");
// Renderer only: loggerService.initWindowSource('windowName') first
logger.info("message", CONTEXT);
logger.warn("message");
logger.error("message", error);
```

### Paths

Desktop filesystem paths live in the Tauri host under `src-tauri`.

### i18n

- All user-visible strings must use `i18next` — never hardcode UI strings
- Locale catalogs live in `src/renderer/i18n/locales/`; `en-us.json` is the source of truth
- Only when you add or change a key: edit `en-us.json`, run `pnpm i18n:sync` (fills the other locales with `[to be translated]:` placeholders), then translate every one. No separate `pnpm i18n:check` run needed — `pnpm lint` includes it, and it rejects leftover placeholders as well as empty values, interpolation/tag mismatches, and unsorted keys.

### UI Design

For any UI component or page style work, read [DESIGN.md](../../DESIGN.md) first and follow its colors, fonts, spacing, and component specs strictly.

## Architecture

### Code Organization

Where each file and directory belongs — read the doc for the process you're touching before adding code or opening a directory. Each process root's top level is a **closed set**: route new code into an existing category, never a new top-level directory ([Naming Conventions §4.8](docs/references/architecture/naming-conventions.md)).

A directory's `index.ts` is a **barrel** — an enforced encapsulation boundary re-exporting one cohesive public API (internals private, outsiders import through it): re-export only (no logic / `export *`), no nesting, and it exists only if lint can seal off deep imports — else no barrel. `index.tsx` is always banned ([Naming Conventions §6.4](docs/references/architecture/naming-conventions.md)).

- [Renderer Architecture](docs/references/architecture/renderer.md) — `src/renderer/` two-axis (type × domain) layout and downward-only layering.
- [Shared Layer Architecture](docs/references/architecture/shared-layer.md) — what belongs in `@shared` (cross-process + no mutable runtime state) and its closed top-level set.

### Data

**MUST READ**: [docs/references/data/README.md](docs/references/data/README.md) for system selection, architecture, and patterns.

| System                                                     | Use Case                            | APIs                                                       |
| ---------------------------------------------------------- | ----------------------------------- | ---------------------------------------------------------- |
| [BootConfig](docs/references/data/boot-config-overview.md) | Early boot settings (pre-lifecycle) | `bootConfigService.get()`, `usePreference('BootConfig.*')` |
| [Cache](docs/references/data/cache-overview.md)            | Temp data (can lose)                | `useCache`, `useSharedCache`, `useSharedCacheValue`, `usePersistCache` |
| [Preference](docs/references/data/preference-overview.md)  | User settings                       | `usePreference`                                            |
| [DataApi](docs/references/data/data-api-overview.md)       | Business data (**critical**)        | `useQuery`, `useMutation`                                  |

Scope:

- **BootConfig**: sync file-based; direct in main (pre-lifecycle), via `usePreference('BootConfig.*')` otherwise
- **Cache**: memory / shared (cross-window) / persist tiers; memory + shared on both main and renderer; persist on both too but as **independent** stores (renderer = localStorage, main = JSON file at `{userData}/cache.json`), never shared — main additionally relays renderer persist sync between windows
- **Preference**: cross-process (main + renderer); auto-syncs across windows
- **DataApi**: SQLite-backed; no auto-sync, fetch on demand from renderer

Shipped SQL lives under `migrations/sqlite-drizzle/`. Do not wipe or rewrite a migration that has already shipped.

**DataApi boundary rule**: DataApi is for SQLite-backed business data only. No database table → no DataApi endpoint; use IPC instead. See [Scope & Boundaries](docs/references/data/api-design-guidelines.md#dataapi-scope--boundaries).

### IPC (IpcApi)

**MUST READ**: [docs/references/ipc/README.md](docs/references/ipc/README.md) — paradigm boundary (RPC vs REST), schema/router/preload/facade layering, `IpcContext`, error model, security.

Non-data command IPC (window/system/shell/notification/external/file) goes through **IpcApi** — the fifth subsystem alongside BootConfig/Cache/Preference/DataApi, RPC-over-IPC with single-point schemas (`schema + handler` to add a route; `ipcApi.request('namespace.action', input)` to call; `IpcApiService.broadcast`/`send` + `useIpcOn` for events). Legacy command IPC still coexists, so you'll encounter both. Decision: SQLite data → DataApi; user setting → Preference; losable/shared → Cache; everything else imperative → IpcApi.

### Desktop Window

The desktop window is the Tauri ToWork host. The renderer reads window init data via `useWindowInitData`.

## Schema & Migration Rules

Shipped SQL under `migrations/sqlite-drizzle/` stays. Do not wipe, rewrite, or renumber a migration that has already shipped. There is no Electron schema tree to generate a migration from.

### Data Classification Toolchain

`scripts/data-classify/` generates the preference and boot-config schemas; `classification.json` is the source of truth (see its README). Two files are **auto-generated — NEVER edit them by hand**: `src/shared/data/preference/preferenceSchemas.ts` and `src/shared/data/bootConfig/bootConfigSchemas.ts`. To change them, edit `classification.json` or `target-key-definitions.json` (both in `data/`), then run `pnpm data:generate`.

## Local Instructions

If `CLAUDE.local.md` exists in the repository root (gitignored, may be absent), read it in full before acting on anything in this file — it holds the developer's private instructions and **OVERRIDES this file wherever they conflict**. Tools that auto-load it (e.g. Claude Code) need not re-read it.
