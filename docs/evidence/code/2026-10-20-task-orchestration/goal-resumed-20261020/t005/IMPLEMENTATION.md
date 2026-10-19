# T005 confirmation authority and changed-state revalidation

## Observed failures

Root captured both failures before production changes (see `confirmation-red.*`
and `revalidation-red.*` in this directory). The controlled original Run emitted
`needs_confirmation=false` while its bound provider still required approval. The
typed-effect campaign executed only three identical verifications after real
byte changes; the no-mutation control passed. These findings do not identify the
unique cause of the earlier T027 seq33 refusal.

## Implementation

- Original fact sessions obtain their surface confirmation provider from the
  already bound Agent loop, including the test preparation path. Decision,
  confirmation wait and preparation therefore consult the same Run authority.
  The receipt's approval recheck and argument-rewrite refusal remain in place.
- The effect fold derives a private duplicate-history boundary from validated,
  complete, bound typed effects without unresolved external operations. Progress
  requires a known kind transition, different file/symlink content hashes, or
  different modes known on both sides. Missing before state, newly populated
  metadata and same-byte rewrites cannot create progress.
- A reset becomes effective only after the whole model tool batch settles, so
  mutation does not bypass queued calls or same-batch twin protection. History
  remains available to the model. Compaction translates the boundary only for
  an exact retained suffix; an unrelated retained list uses the conservative
  full retained history.
- The foreground Core receipt uses the same Watch's bounded sampled before
  identities and its existing after observer. Unknown before state stays
  unknown. No extra full-workspace scan, tool-name heuristic, threshold increase
  or execution framework is introduced.

## Before-image boundaries

The observer reuses captured dirty-Git identities and the bounded non-Git
content index. A clean Git file's first write can have `before=None` when the
Watch did not capture its actual earlier bytes. HEAD/index contents are not
substituted for that missing observation. A known non-Git stat entry also keeps
`before=None` when its content baseline could not be captured within the existing
64 MiB budget. Only a complete non-Git index can prove that an absent entry was
Missing. Such unknown before-images cannot reset the duplicate guard.

Adding a before-image still passes the original typed observation validation
and canonical size bound. A validation failure returns an explicit Incomplete
observation. The observer does not turn a read failure into Missing.

## Verification scope

The new tests use controlled model responses and temporary files. They do not
invoke a paid model, VM, GUI or shell. The Core integration path uses the public
AgentSession, builtin write tool, original typed receipt producer and effect
fold; it checks both changed bytes and same-byte rewrites. The effect adapter
campaign uses both memory and file logs, with no-mutation, same-byte and
incomplete-observation controls. Additional small fold tests cover comparable
identity fields, compaction and delayed batch reset. Existing argument-rewrite
and parallel-twin tests are retained.

Root completed the focused and connected GREEN runs recorded in [OUTCOME.md](OUTCOME.md).
Repeated focused runs are not added to the counts of their containing suites.
The implementation and these hermetic tests establish neither an observed T01
speedup nor Windows filesystem observation behavior.
