# Honor reply-only Word report requests

Issue: [#216](https://github.com/aster-ai-lab/aster/issues/216). Studio change: `a379e3aa8acd69664d2e713bd4720dbdbbe62e22`; before source `26d3915a3`. Parent base: `29650ead2`.

## Result

Removed the unconditional XLSX and extra-image directive from normal home-composer turns. The existing standing delivery contract already says the user's explicit scope and output format take precedence and requires real files for requested file tasks. It remains unchanged. No request classifier, keyword heuristic, permission exception, or replacement prompt layer was added.

The actual `homeTurnContext` outputs were captured from the TypeScript implementation before and after: four default directives become three. Reply formatting, genuine conflict clarification, source quotations and typed review/research settings retain their existing behavior. The user request itself is unchanged by this repair.

## Verification

- The new regression fails on the old context because it forces a new workbook and extra image.
- Capability and home-options suites: 15 passed across 2 files.
- Scoped ESLint, formatting and staged whitespace checks pass.
- The formatted final source produces exactly the frozen `after-context.json`.
- A real installed Aster 0.1.6 ACP binary was invoked twice with the advertised and selected model ID `codex/gpt-6.1-sol`, medium reasoning effort and a fixed 180-second turn bound. The model catalog and each opened session confirm that selection. No mock model server was used.
- Both requests use the same synthetic report prompt, standing delivery contract and model settings, in fresh sessions with isolated runtime state. Only the frontend-generated per-turn directives differ. The frozen probe spec hash is `161b1e43ebf1855f082dfe649a66de9504629767920f2e71cd2fd5c9c106d58f`.
- Both before and after return a full report on the first turn: all requested headings, two tables and the requested end marker; no ACP tool-call events. The workspace contains session metadata and no document output file.
- Reopened both complete replies. The production Markdown parser (`unified` + `remark-parse` + `remark-gfm`) finds two tables with two data rows each. Project identities, status, responsible people, budget quantities, missing actual-spend values, staffing quantities and reporting period match the synthetic source. Parsed records are retained in `reply-table-audit.json`.

The before and after replies both pass this synthetic probe. The historical extra-clarification turn was not reproduced on the current installed runtime. These observations verify the fresh reply-only path and removal of the confirmed conflicting default; they do not prove a performance or reliability improvement. Each model request ran once. Prompts, model, timeout and acceptance criteria were not adjusted after seeing the answers.

Commands: `pnpm test:renderer src/renderer/pages/agents/workbenchHomeCapabilities.test.ts src/renderer/pages/agents/__tests__/useHomeComposerOptions.test.tsx`, scoped ESLint/Oxfmt, and the retained `probe.py` using JSON-RPC `initialize`, `session/new` and `session/prompt`. Evidence log copies trim trailing empty stdout lines only. `manifest.json` records the fixed inputs, reply hashes and observed installed binary fingerprint. No account credentials or private user session state were copied.

## Explicitly not claimed

The original user's full private prompt and historical session were not replayed. The installed runtime is newer than the 0.1.4 environment in the issue. Backend service identity beyond its selected advertised model ID is not attested. A new installed application build containing this frontend source change, DOCX export, full Studio lint/typechecking, and a full end-to-end replay through the desktop UI are not claimed. Existing history was not rewritten.
