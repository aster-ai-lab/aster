# Independent issue review

All seven GitHub issues remain open, with no comments or open pull requests
at inventory time. Their existing bug label has since been added.
Remote main is `eb17c6ce5aa4c9ef785c12d56515e146866b1506`.
The existing 41-file repair snapshot matches the current files exactly.
This review reuses that completed work; it does not claim to have authored it.

| Issue | Accepted resolution and boundary |
| --- | --- |
| [#1](https://github.com/aster-ai-lab/aster/issues/1) | Default unattended continuation stops after three no-progress opportunities. Progress comes from the current slice's settled original mutation receipts. A verified turn alone does not prove the goal complete; a stalled unfinished goal writes a failed terminal result with needs_input. Explicit ACL, deadlines and old fact replay retain their contracts. The report's unconditional StepAccepted reset was already absent in this baseline. |
| [#2](https://github.com/aster-ai-lab/aster/issues/2) | Headless and TUI share the non-Grok empty-auth-method policy. Advertised authentication and interactive-login refusals remain. The separately reported 180-second hang was not reproduced and has no claimed independent diagnosis. |
| [#3](https://github.com/aster-ai-lab/aster/issues/3) | Strictly validated initialize rules reach common session options for new, load and replacement. A nonempty full system prompt override now refuses explicitly; append rules cannot replace hard host permission contracts. |
| [#4](https://github.com/aster-ai-lab/aster/issues/4) | The existing catalog supplies descriptions, parameter names/defaults and exact template execution. Template values enter the existing QuickJS runner as JSON data. Scoped nested tool governance, filters, cancellation and budgets remain. Code presentation retains the catalog. |
| [#5](https://github.com/aster-ai-lab/aster/issues/5) | Lazy adapters share a canonical-directory Arc through a weak registry. Open/import/publication and last release use one mutex; dead entries are pruned, failed initialization remains retryable. Direct-store exclusivity and on-disk formats remain. |
| [#6](https://github.com/aster-ai-lab/aster/issues/6) | Gateway's three internal dependencies resolve locally at the original versions; Gateway/Sentry/Updater are not publishable. Work ownership is checked through real Cargo metadata rather than treating a legitimate gitfile as a second workspace. Source/publish guards remain. |
| [#7](https://github.com/aster-ai-lab/aster/issues/7) | Unknown workspace gating names the unconfirmed feature gate and existing local override. Enabled/Disabled distinction, server/env priority and refusal behavior remain. No hidden feature or real leader is enabled. |

## Delivery isolation

The candidate is based on remote main in a detached worktree inside this
repository. Thirty-seven files match the existing repair snapshot verbatim.
Only the two reviewed workspace-gate hunks enter main.rs. Cargo.lock removes
the three registry A3S duplicates while excluding six unrelated CLI updater
dependencies. No unrelated Gateway implementation, updater feature, desktop
change or submodule revision enters the patch.

Independent review found the durable-memory tests still inline in production
source. The isolated candidate moves that complete module to the owning sibling
test file. The runtime prefix and suffix are byte-for-byte preserved, and test
module paths stay the same. The candidate therefore contains 42 files.
The new program wire type also uses a private enum with parse/as_str; schema,
execution, capability checks and metadata share its closed script/template set.

Work source is exported from the exact gitlink commit
`9aad91f1e934c731e540cf99be25733e25c3a411` for workspace validation. It contains
tracked source, not a copied Git directory, credentials or a user session.
The locked offline metadata proof finds 161 members of this root workspace.
The patch passes a reverse applicability check, and isolated workspace format
checking passes. The full Python guard reproduces the existing missing
`apps/web/public/tenant/legacy/INFO.md` assertion (5/6); it is not weakened.

## Closure condition

Local files, tests and a prepared patch are not a published fix. Close each
issue only after its reviewed fix is available on the default branch, linking
the actual commit or PR and stating the validation limits. The repository
constitution says “Do not commit unless the user asked.” The current request
to handle all repository issues covers the necessary issue-fix commits and PR
workflow. Its scope is this reviewed, isolated candidate, not the unrelated
working tree. The isolated fix commit has been pushed and PR #8 published. No resolution
comment or issue closure has happened at this pre-merge capture.

The existing main Rust workflow is already red before this candidate: Ubuntu
stopped at Format, macOS at Work launcher, and both frontend jobs at dependency
installation. See [the baseline run](https://github.com/aster-ai-lab/aster/actions/runs/39406671663).
No CI setting or guard is bypassed by this patch.
