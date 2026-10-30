# Research and analysis

Reviewed the specification, plan and task coverage before implementation on 2026-10-28.

| Finding | Severity | File/task | Resolution |
| --- | --- | --- | --- |
| Standalone SeeCode HTML contains inline scripts/styles and cannot be embedded under current site rules. | note | plan / T001 | Use its real SVG renderer, extract CSS, and add controls through the existing external script. Preserve the bans. |
| GitHub README does not execute interactive JavaScript. | note | spec FR-006 / T004 | Export settled SVG with alt text and link into interactive documentation. |
| Source comments and runtime behavior differ for default style selection. | note | T005 | Describe the actual primary-session path, distinguishing the public heuristic helper. |
| Other sessions have uncommitted root edits. | note | all | Work in an isolated detached worktree. |
| All requirements have producing and verification tasks. | note | FR-001–007 / T001–009 | Ready for implementation; no blockers. |

Primary references: `https://github.com/Aryanutkarsh/SeeCode`, `https://codehike.org/docs/concepts/code`, `https://codehike.org/docs/layouts/scrollycoding`, `https://github.com/A3S-Lab/stc-answer`.


Scope re-audit: the later user goal explicitly authorizes publication and latest
release defaults. Added requirements FR-008–012 map to T010–012 and the existing
verification tasks. GitHub's global latest is currently the standalone v9.1.6
ACP release, so using that redirect for application downloads is incorrect.
The fixed website copy command now supplies the selected CLI version explicitly.
The existing publisher workflow names are `Release aster-cli` and
`Release aster-desktop`; both are subscribed by the Pages workflow.
No constitution blocker was found in this expanded scope.
