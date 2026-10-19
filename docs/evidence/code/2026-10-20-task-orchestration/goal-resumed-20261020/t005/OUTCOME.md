# T005 verified implementation

The two controlled failures have been repaired and their connected regression
suites are GREEN. Run decisions and preparation now use the same bound approval
provider. Identical verification calls can run again after complete, bound
effects prove a byte or object-state change. Same-byte rewrites, missing
before-images, incomplete observations and same-batch twins retain protection.

The root agent executed the builds/tests. This slice did not run a paid model,
VM, GUI evaluation or T01 benchmark, and makes no task-speed claim. It does not
identify the unique cause of the earlier T027 seq33 refusal.

## Evidence

- Original failures: [confirmation RED](confirmation-red.json),
  [revalidation RED](revalidation-red.json), with their adjacent raw logs.
- Core connected runs: [commands, exits and artifact identity](core-connected.json).
  The frozen binary is `target/qualification/t005-cua-20261020/core-tests`,
  SHA-256 `92486c70ac258c5e1c43b889818e0b00b3438ba27b7ef0cbd42551284db0e3a3`.
- Source scope: [12-file freeze](source-freeze.json),
  [independent current-source recheck](source-recheck.json),
  [scoped formatting and diff checks](static-checks.json).

| Connected verification group | Observed result | Raw evidence |
| --- | --- | --- |
| Original Run confirmation snapshot | 1 passed | [Core 01](core-connected-01.log) |
| Original builtin write → producer → fold | 2 passed | [Core 02](core-connected-02.log) |
| Porcelain receipt observer, including 4 new before-image cases | 33 passed | [Core 03](core-connected-03.log) |
| Receipt suite, including hook rewrite and approval binding refusal | 55 passed | [Core 05](core-connected-05.log) |
| Nested governance compatibility | 8 passed | [Core 06](core-connected-06.log) |
| Existing actual fact execution-target compatibility | 4 passed | [Core 07](core-connected-07.log) |
| Two Windows shell guidance pure unit cases, run on macOS | 1 passed each | [Core 08](core-connected-08.log), [Core 09](core-connected-09.log) |
| Governed grep without embeddings and permission denial | 1 passed | [Core 10](core-connected-10.log) |
| Typed effect integration, including 3 revalidation tests and both log adapters | 14 passed | [Effect integration](effect-tool-effects-green.log) |
| Final comparable-identity, compaction and batch-settlement unit cases | 3 passed | [Final Effect unit run](effect-final-unit-green.log) |
| Same-batch parallel twins | 1 passed | [Twins](effect-twins-green.log) |

The separately rerun approval-hook case in Core 04 is already included in the
55-case receipt suite. Earlier focused Core revalidation and Effect unit runs
repeat rows above. No aggregate count adds these overlapping runs together.

## Scope and limits

The 12-file freeze includes the two confirmation source changes, the foreground
receipt/Watch seam, the bounded before-image observer, the private effect-fold
repeat window, sibling regression tests and their module registrations. Shared
files preserve the previously existing changes named in the freeze manifest.
No manifest, lockfile, public receipt shape, threshold or T028 production file
was changed by this slice.

Known dirty-Git and captured non-Git before-images can prove a change. A clean
Git first write without a captured before-image remains unknown; neither HEAD
nor the index is treated as its earlier byte observation. A non-Git existing
file whose content capture exceeded the original budget also remains unknown.
Only a complete non-Git index proves absence. Validation and canonical bounds
remain enforced after before-images are attached. See [implementation details](IMPLEMENTATION.md).

The producer and original-file integration cases were exercised on macOS/Unix.
The two Windows guidance pure unit tests establish string/policy behavior only;
they do not qualify Windows filesystem observation or a native Windows run.
