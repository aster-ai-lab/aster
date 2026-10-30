# Issue 205: formatted Word export

Date: 2026-10-30. Issue: [#205](https://github.com/aster-ai-lab/aster/issues/205).

The only open Aster issue in the inventory is a confirmed Word export defect.
The shared export bridge wrote Markdown lines into plain Word text. It now
uses the existing Office Markdown parser and DOCX exporter, preserving headings,
numbering and bold spans as editable Word structure. Existing filename,
directory selection, cancellation and write-error handling stay at the shared
save boundary. No dependency, endpoint or schema changed.

## Source and delivery

Aster base: `1c82300b4c1f8729a2cf9efef5aebd9a7bda1f29`.
Previous Studio pin: `497300b348c04a40775205eb98648768f1cbf0d5`.
New Studio pin: `f6d648642fc5f4f3999ec2fd537e4e2e42cb45bc`.

[Studio PR #8](https://github.com/kense-labs/studio/pull/8) is merged.
Its complete tree `fbc829d21640be130212bcfebe652d2f46c0d88a` equals the
locally tested candidate `2388970f9bf3033e152c7f0953dd95648ffb1029`.
The six-path difference from Aster's old Studio pin contains the shared exporter,
bootstrap, preload, two regression test files and private evidence. The old pin
was one naming commit beyond Studio main; that already-pinned naming content
was preserved byte for byte. Studio source remains in the private submodule.

The first local candidate was GPG-signed and DCO-signed, but GitHub reported
`unknown_key`. The replacement was created with GitHub's commit API and the
same complete tree. Both its commit and merge are GitHub Verified with DCO;
no account key, email or token scope was changed. PR #7 was closed as superseded.
[Merge verification](studio-merge-verification.json) records the new pin.

The Aster issue remains open at this pre-merge evidence snapshot. The public
pin-update PR must merge before the issue is marked completed. Final merge,
issue state and comment receipts are retained locally after publication.
Only the isolated candidate and this evidence are staged; the original root
and Studio working trees' unrelated changes are preserved.

## Validation

- The exact issue sample first failed because the old DOCX had no Heading1.
- [Focused renderer tests](logs/renderer-tests.log.gz): 134 passed across the real
  bridge, export actions, service and bootstrap. Escaped Markdown and fenced
  code remain literal.
- Existing EX-03 save/cancel passed in Chromium and WebKit in the earlier
  isolated run. That run's new sample case was blocked by a task chooser.
  Dismissing the chooser with normal Escape fixed the test precondition.
- [Final exact-sample run](logs/browser-exact-sample.log): two passes, Chromium
  and WebKit. It uses the real message menu, bridge and Office conversion.
  These and the earlier two passes are separate runs, not one fresh four-pass run.
- [Independent ZIP/XML inspection](docx-validation.json) checks CRC, all six
  source paragraphs, Heading1 style and its definition, three native numbered
  paragraphs with valid numbering IDs, all four bold spans and the final marker.
  Both final synthetic DOCX files and browser screenshots are in `artifacts/`.
- [Native Quick Look rendering](artifacts/native-quicklook.png) independently
  shows the correct heading, numbered list and bold spans with no raw syntax.
- Scoped ESLint, Oxlint, Oxfmt and diff checks passed. Parent locked/offline
  Cargo metadata passed (167 packages), as did workspace rustfmt.
- Office was rebuilt from tracked parent sources. Installed dependencies were
  cloned locally for the isolated checkout; no dependency was installed.

[Check results and raw-log hashes](checks.json) distinguish passes from limits.
Full web types have 380 diagnostics outside the added helper/bootstrap/tests;
full E2E types have six in unchanged code. Studio docs have 361 existing broken
links and nine missing source references; structure and index pass. No full
workspace or full lint pass is claimed. The `pnpm` executable was unavailable;
the corresponding installed tools were invoked directly.

The public base's [Rust CI run](https://github.com/aster-ai-lab/aster/actions/runs/40995146058)
already fails `test_svgs_match_the_generator` for stale app-icon SVG sources
on both OS jobs. Its desktop frontend jobs pass for both tenants. The baseline
failure log is retained; no check, workflow or protection is bypassed.

## Failed attempts and limits

The initial new browser locator used the wrong accessible name and was corrected
to the observed `更多操作`. Missing modules during a dependency copy required
restarting only the owned renderer. An exploratory check expected an explicit
Heading1 outline property; inspection showed the built-in style is defined and
native rendering is correct. That extra assumption was not part of a frozen
grader. These attempts are not counted as passing acceptance runs.

No installed app, installer, release or updater was replaced. WPS and Microsoft
Word interactive editing were not exercised. Quick Look is a native rendering
check, not WPS certification. No live model, user document or account data was
a fixture. Publication API payloads, source patches and backups remain private.
