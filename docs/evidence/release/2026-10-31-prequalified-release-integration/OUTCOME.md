# Prequalified release source integration

The retained PR #37 head `52741fee0a831bd9ed81cc223da13d61a184a7f7`
is reconciled with current product release governance. Expensive branch
qualification and product-specific dependency caches are retained. Tag builds
require a successful completed branch qualification for their exact source
commit, rather than repeating the Rust test suites during publication.

## Preserved requirements

- Desktop's existing release/signing/notarization workflow is retained exactly;
  the incoming deletion is not adopted.
- Core/ACP keeps `a3s-code-v*`, version matching for both Cargo packages,
  version-specific detailed notes and `--latest=false`. The incoming bare `v*`
  trigger and generic notes conflict with the documented desktop namespace.
- CLI keeps a cheap governance preflight on normal and reused-artifact paths.
  Its publisher remains on macOS and runs both actual installed-archive sandbox
  modes before creating or uploading assets. Reused archives additionally
  require branch qualification for their producing commit.
- Missing sandbox, pair-identity and governance checks are retained in the
  branch qualifier. Core/ACP retains Windows coverage, Windows format/dependency
  checks and the macOS `just` prerequisite.
- Core publication refuses missing notes, mismatched versions and incorrect
  three-platform archive membership, with explicit reasons.

The [GitHub workflow-runs API](https://docs.github.com/en/rest/actions/workflow-runs#list-workflow-runs-for-a-workflow)
documents the commit filter used by the gate and the required Actions read
permission. Local tests execute the actual shell gate with controlled GitHub
responses; they do not establish that the current commit has passed remote CI.

## Verification

With the existing Homebrew Python 3.13 interpreter on macOS arm64:

| Check | Result |
| --- | --- |
| `tests/cli_release_workflow.py` | 11 passed |
| `tests/code_release_workflow.py` | 5 passed |
| Original `tests/release_governance.py` | 9 passed |
| Final YAML parse and job dependency references | 6 workflows passed |
| Actual Bash command syntax | 86 steps passed |

The executable cases cover successful, absent and wrong-commit qualification,
reused-artifact admission, installed-smoke failure before create/upload,
published or mismatched drafts, Core version/tag/notes identity, and wrong
archive membership. Test commands invoke the production workflow scripts.

An initial run exposed two conflicts with the existing governance assertions.
Both failed alone before the workflow correction and pass afterward. The
governance test file and desktop workflow remain byte-identical to HEAD.

The incoming contract tests also assumed the retired CLI 0.1.4 artifact names,
bare Core tags, deletion of the desktop publisher and removal of every Python
release check. Those assumptions were reconciled with the authoritative current
CLI 0.1.5 manifest and release rules. The original incoming commit remains in
merge ancestry. No model evaluation, prompt, timeout or frozen grader changed.

## Explicitly not claimed

These are local workflow/assembly tests with synthetic archives and GitHub
responses. They do not qualify real final binaries, a GitHub-hosted runner,
Windows/Linux runtime behavior, a notarized DMG, current remote CI or a release.
Remaining parent workspaces and final publication are still pending.
