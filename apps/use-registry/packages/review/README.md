# 合同审查 · A3S Review

Package source `a3s/review@1.3.0` (unreleased). The AsterDream plugin page presents this `.aix` package as **合同审查**. It extends the composer's installed-agent list, with two declared surfaces:

| Surface | Id | Role |
| --- | --- | --- |
| Skill | `review` | The `/review` prompt. The host supplies extracted Word text or explicit pasted text in segments. |
| Agent | `legal-expert` | Reviews one explicit source through the same host. Defaults to contract review; follows explicit basic, official-document, security or mixed scope. |

After installation, type `@合同审查` in the home or task composer and select the
agent. Choose one Word file or enter its absolute path in the inline source
picker, add review instructions, then send. Cancelling selection preserves the
draft; a missing source blocks submission. The plugin has no standalone window
or `ui` surface. Its native launcher and former home mini-app entry are removed.

The task opens the version-bound document, shows host segment progress and
ephemeral attention, and anchors unique findings as document comments during
the run. The comment count reflects actual anchors, never scanning highlights
or model prose. Empty, failed and stopped reviews keep honest status; comments
already placed remain visible. The source stays read-only, and changed bytes
require a new review. Manual review does not enable automatic task review.

Word sources remain bounded to 8 MiB. The internal agent/skill ids stay stable
so existing tasks and installations remain usable. The workbench agent-directory
page is removed; agent extension installation uses the plugin catalog.

The host refuses ambiguous document selection and rejects incomplete or
ungrounded findings. A clean segment returns exactly `[]`. Unknown facts stay
marked `【待核实】`; suggestions must not invent dates, amounts or identities.
Local exact whitelists and selected checks are passed in the user's request.
Basic corrections describe only the minimal necessary replacement; an extra
paraphrase or sentence shortening is not accepted as proofreading.
Word findings carry the SHA-256 of the exact bytes the host extracted. The
workbench checks that digest before placing comments and keeps the source view
read-only. Missing historical versions and changed bytes require a new review.
These source changes do not update a signed publication or an installed package.

Pasted text enters through the native `turnContext.reviewSource` contract, separate
from the review instructions. The host bounds it to 256 KiB, records its SHA-256,
preserves exact source substrings across overlapping segments, and binds no Word
document or writeback action to text findings.

The public registry address is the gateway route `/use-registry`. The process
on `:4873` is only the upstream. Trust stays the pinned `--trust-root`.

This directory is package source. It enters the signed tree through
`admissions.acl` and `a3s-use-registry-tools assemble`. The picker never
receives the shell bearer.
