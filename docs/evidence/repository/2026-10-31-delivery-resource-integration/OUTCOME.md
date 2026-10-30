# Required delivery resources — source integration

The actual parent merge integrates
`56f876a1ab0456b2466cae36c9956a2f0eee0d85`. Its Studio source
`e624b493322eeda18366dc04c476c0068badf988` is already an ancestor of the
canonical Studio head `149b77ce04b6256535796267e9396fa46f1e572e`.

All 11 conflicted paths were reviewed. The delivery implementation is already
present with later fixes, so the current source, workflows, tests and Studio
gitlink are retained byte-for-byte. This preserves CLI 0.1.5, the three current
platform archives, successful branch qualification for the artifact's actual
source, both installed-archive sandbox and host-mode probes, portable macOS
checksums, desktop bilingual notes and manifest validation. No test assertion
or deadline was changed.

The 25 added historical evidence files match the incoming Git blobs, working
files and index exactly. Every gzip was decompressed; JSON and JSONL were
parsed. The 12 original unittest summaries match `checks.json` and its total
of 122. [Integration receipt](integration.json) records paths, lengths and
SHA-256 hashes. The original outcome remains unchanged: its CLI 0.1.2 version,
four platform channels, 23-file inventory and old archive results describe
that historical branch, not the current product. The canonical desktop runtime
inventory now has 24 entries and includes `capture-helper`.

## Current checks

- `/opt/homebrew/bin/python3.13 tests/release_governance.py`: 9 passed on the
  final full run. The first run had one 10-second subprocess timeout in the
  draft-release refusal case. That case passed alone in 0.572 seconds before
  the full rerun passed in 2.626 seconds; its cause is not attributed.
- `python3 tests/cli_sandbox_smoke_test.py`: 3 passed, including the distinction
  between Darwin host receipts and Linux sandbox receipts.
- `git ls-files -u`: empty after resolution. The final source delta against
  the pre-merge HEAD is empty; additions are historical evidence and this
  integration record.

The successful governance outputs are retained in
[isolated.log](isolated.log) and [governance.log](governance.log).

## Explicitly not claimed

No fresh binary, installed-archive execution, native GUI campaign, DMG,
notarization, push or release is established by this evidence-only merge.
The old 122 checks and old archive probes are not counted as current-build
qualification. Other retained parent source heads still require integration.
