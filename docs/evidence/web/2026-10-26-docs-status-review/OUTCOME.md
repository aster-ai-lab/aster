# Documentation status review

Date: 2026-10-26. Scope: the current Code 9.1.6 capability index, tutorial delivery page, website README and their generated HTML.

## Result

The capability index still said the tutorial package had never been compiled or run. That statement predated the successful [reference-host validation](../2026-10-26-runnable-docs/OUTCOME.md). It now states the verified scope and links to the reference package and delivery chapter. The delivery page no longer lists confirmation selection as missing, because the reference TUI already selects requests by exact ID. The website README separates interface-inventory coverage from tutorial execution and live-service qualification.

The site remains 79 MDX documents and 130 public documentation routes. This review corrected stale status prose; it did not establish that every documented runtime capability has been exercised.

## Validation

- `npm --prefix apps/web run build`: passed, 79 documents / 130 routes.
- `npm --prefix apps/web run check`: passed; [log](generated-check.log).
- `python3 apps/web/scripts/check_site.py`: passed, 152 files / 130 documentation routes; [result](site-check.json).
- Focused `git diff --check` for the changed prose and generated documentation: passed.

## Explicitly not claimed

- No browser or Rust tests were rerun for these prose-only edits. The previous 59 Node, 19 Python, 148 browser steps, 10 tutorial Rust tests, two ACP tests and 13 native TUI steps remain recorded with their actual tested inputs in the earlier evidence directory.
- Real-provider tutorial quality, full ACP client/companion integration, external services, platform sandbox qualification and a release-tag-pinned historical source archive remain outside the completed validation.
- No deployment or commit was performed. The earlier staging receipt describes its own earlier output; it is not a receipt for the newly rebuilt prose.
