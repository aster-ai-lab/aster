# Work forms, attachment names, and Chinese UI

The screenshot exposed separate failures in skill discovery, form validation,
rendering, attachment metadata, and conversation state. These changes remain
in the working tree; this record does not announce a release.

## Changes

- Repair dead bundled skill links left by a moved checkout, preserving custom
  skills and unrelated broken links. The local `ask-form` link now resolves to
  this checkout. Built-in question tools no longer require that optional skill.
- Reject malformed OpenUI containers, nested component calls, duplicate nodes,
  and unsupported components before waiting for a response. Invalid or unbound
  forms show a localized error instead of dumping their source.
- Display the waiting-for-form hint before queue shortcuts. Preserve the real
  extension when a display label becomes an attachment filename in either send
  path. The previous XLSX and DOCX regression cases failed before the fix.
- Default missing translations to Chinese. Localize composer key labels,
  permission button hints, known skill/question/program/document tool labels,
  and citation/error accessibility labels. Native menus use the same locale
  catalogs as the renderer, with the language selected at application startup.
- Name a new conversation from user-authored text, falling back to its file
  name. Internal attachment scaffolding no longer becomes its title.
- Treat the configured reply language as the fallback. User-facing prose
  follows the latest user-authored request; attachments, tools, host context,
  and internal skill prompts do not choose that language.

## Validation

Commands run from the workspace root or `apps/work` as appropriate:

- `cargo test -p a3s-code-core --lib ask_user_form`: 6 passed.
- `cargo test -p a3s-code-core --lib prompts::tests`: 46 passed.
- `cargo test -p aster-dream --lib builtin_skill_tests`: 2 passed.
- `cargo test -p aster-dream --lib conversation_title_tests`: 2 passed.
- `cargo test -p aster-dream --lib menu_labels`: 1 passed.
- Focused renderer language/form/attachment tests: 54 passed.
- Focused composer form and file tests: 5 passed, 146 unrelated cases filtered.
- Permission, task-conversation, and citation tests: 111 passed.
- `pnpm i18n:check`: passed, 19,376 translations checked.
- Scoped ESLint: no errors; the existing composer dependency/layer warnings
  remain. Renderer production build passed.
- `cargo build -p a3s-code-acp -p aster-dream --offline --locked`: passed.
- Real ACP/model greeting probes with a `zh-CN` host fallback: Chinese,
  English, and French requests each produced a reply in that language and
  ended normally. Generated final replies and the ACP binary digest are
  retained in `live-language.json`; no private reasoning is retained.
- A3S Test 1.0.3 attached to the exact AsterDream bundle process and retained
  semantic/window evidence locally. The existing task's approval dialog
  prevented an isolated form submission. The test finished with status
  `failed` and successful cleanup, without approving or answering that task.

Full `pnpm lint` remains blocked by existing lint failures. The full web type
check, retried with an 8 GiB heap, completes and reports existing errors in
unrelated files. Those checks are not reported as passing. Private desktop
observations and user files are not copied into this evidence directory.

## Explicitly not claimed

No successful installed-app form-submit E2E, comprehensive model language conformance,
native-menu live language switching, release, push, or CI run. The running
desktop process is not replaced by a source change. Bash qualification and
binary Office reading remain separate work; keeping upload extensions fixes
the demonstrated filename loss but does not make `read` an Office parser.

## Chinese UI follow-up

MCP tool details now use the existing localized arguments/output headings and
localized image descriptions. The regression uses real locale catalogs,
failed in both locales before the fix, and now passes: 28 focused MCP,
resolver, and composer-key tests passed. ESLint, Oxlint, scoped formatting,
translation validation (19,378 entries), workspace Rust formatting, and the
renderer production build passed. Raw logs are retained beside this record.

The shared system and turn contracts now treat the host language as a
fallback. Short greetings and non-English Latin-script requests follow the
actual human request. The rule covers visible reasoning, plans, goals, and
status prose; attachments and internal skill/host prompts do not select the
language. Code, paths, and quoted sources retain their original spelling.

The prompt family passed all 46 cases. The broader documentation gate still
reports 120 existing broken links in migrated documentation; it is not
reported as passing.

The updated ACP and Work debug builds passed. With that exact ACP executable,
a real model in one session followed Chinese → short English greeting `Hi`
→ French → Chinese again, with all four turns ending normally. Final replies,
the executable digest, and prompt source digests are retained in
`live-language-switch.json`. This checks language selection; the first reply
was longer than the requested sentence, so broader instruction conformance
is not claimed. The running desktop instance was not replaced.
