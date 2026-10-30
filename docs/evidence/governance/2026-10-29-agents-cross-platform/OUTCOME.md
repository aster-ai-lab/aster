# Actionable AGENTS.md rules and cross-platform compatibility

Date: 2026-10-29. This follows the user's request to place the optimized
development rules directly in the repository-root `AGENTS.md` and explicitly
preserve cross-platform compatibility.

## Result

Expanded `AGENTS.md` into 12 actionable Rust implementation/review rules and
9 cross-platform rules. Developers can apply the core requirements directly
from that file. The full development standards retain the rationale and source
mapping; their introduction and §17 now point to the root execution rules.
The constitution's workflow points to the same entry point.

Cross-platform coverage includes Windows/Linux/macOS and affected architectures,
OS-specific `cfg`, native path and process values, shell argument handling,
process/PTY and filesystem adapters, explicit text encodings, exact-byte
evidence, CPU/ABI/OS/libc/linker baselines, portable path fixtures, and native
verification with explicit target coverage. The prior line-ending bullet was
consolidated into that section without removing its requirements.

## Verification

Commands ran from `D:/code/aster` on Windows AMD64. `verification.json` records
the exact interpreter, commands, results, and source hashes.

- `git diff --check -- AGENTS.md docs/development-standards.md .specify/memory/constitution.md`:
  passed.
- `python -B -X utf8 tests/line_endings.py`: 6/6 tests passed. These exercise
  real Git staging and checkout under both `core.autocrlf` settings, including
  UTF-8/LF source, CRLF Windows command files, immutable evidence, and binaries.
- `python -B -X utf8 tests/governance_regime.py`: 3/4 tests passed. The existing
  negative mutation test still fails at line 140. Its section-parser defect
  was reproduced on the original HEAD in
  `../2026-10-29-microsoft-rust-guidelines/governance-baseline.json`; neither
  the test nor its expected results was changed here.
- Reopened all three governance files, compared the executable rules with the
  detailed standards and `.gitattributes`, and checked UTF-8 without BOM and
  LF-only bytes. Existing project, regime, evidence, and delivery rules remain.

## Explicitly not claimed

- Native Linux/macOS runtime verification, product-wide cross-platform
  qualification, or new production Rust behavior.
- A passing full governance suite, resolution of the previous code-review
  findings, changes to CI/tools/dependencies, or a commit.
