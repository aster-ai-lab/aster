# Knowledge vault bootstrap

Status: implementation, focused native/renderer checks and real native-backed browser acceptance passed.

## Result

The first note or folder in a fresh workspace no longer tries to create the
reserved source name `wiki` through the generic entries endpoint. The renderer
uses `PUT /api/local-documents/vault`, a typed, idempotent bootstrap of
`wiki/local`. The host reuses the generic operation's canonical workspace and
directory boundary. Extra request fields, invalid roots and vault symlinks are
refused. An existing file in either directory slot returns 409 with the machine
code `vault_not_directory`; its bytes are preserved.

The generic create endpoint still rejects reserved names. Existing unrelated
working-tree edits were preserved. No files were staged or committed.

## Verification

From `/Users/roylin/code/aster`, Rust 1.97.1:

- Red: the new renderer case, starting without `wiki`, reproduced
  “这个操作没有完成。” and failed to find the new `1234` note.
- `cargo test --locked -p aster-dream --lib local_documents -- --test-threads=1`:
  13 passed, including three new hermetic bootstrap/path/collision tests.
- `cargo test --locked -p aster-dream --lib authority::local_vault_tests -- --test-threads=1`:
  1 passed. Actual HTTP routing authenticates before bootstrap, refuses a
  foreign origin, permits repeated bootstrap, creates and renames the first
  file to `1234.md`, and creates the default folder. Invalid root/body and
  occupied-directory responses were checked through HTTP too.
- Renderer `KnowledgeNotesPage.test.tsx`: 34 passed. The fixture now models
  the native reserved-name refusal and covers an initially absent vault.
- Targeted formatting and `git diff --check`: passed.

Vitest was invoked using the installed `node node_modules/vitest/vitest.mjs`
with `run --project renderer`; the `pnpm` launcher is absent in this shell.
The actual test runner/project is the one specified by the plan.

## Adversarial checks and alternatives

- Files occupying `wiki` or `wiki/local` are never replaced.
- An outside symlink receives no `local` child.
- Arbitrary `path`/`parent` body fields are refused rather than accepted as
  additional write authority. Missing/invalid shell authentication performs
  no bootstrap.
- The rejected alternative was allowing reserved names in generic create:
  the dedicated endpoint keeps that restriction intact.

## Explicitly not claimed

- Browser session `bugfix-native-flows-retry-20261020` created `1234.md` and the default folder from an initially absent vault, edited the note, saved with Command-S and checked its actual bytes. The fixture proxies the real native authority, ETag and streaming change feed. Evidence: `../2026-10-20-native-bugfix-roundtrip/`.
- User-system GUI reproduction and full-workspace certification are not claimed.
- No new signed DMG, installation, commit, or release publication.
- No clean full-workspace formatting or full-workspace test certification.
