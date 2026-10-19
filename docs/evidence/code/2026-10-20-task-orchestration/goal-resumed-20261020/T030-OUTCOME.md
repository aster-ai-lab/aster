# T030 controlled candidate qualification

Five controlled groups finished with exit 0 and no timeouts: 13 distinct tests.
These are loopback-provider and owning-contract checks, not a paid model run or
a T01 speed result. The real ACP process used the signed candidate executable
cloned from the staged app, and the Office companion came from that same stage.

| Group | Passed | Command wall time | Raw log |
| --- | --- | --- | --- |
| Program/native artifact delivery, cancellation, approval refusal, required-check failure and repair | 5 | 94.821 s | [Program](t030-program.log) |
| Actual adaptive grep through Work → ACP → Core without embeddings | 1 | 16.566 s | [Adaptive grep](t030-adaptive-grep.log) |
| Trusted launcher capability projection and bounded/refused inputs | 4 | 0.021 s | [Launcher](t030-launcher.log) |
| Directory opening path and symlink boundaries | 2 | 0.013 s | [Directory](t030-open-directory.log) |
| Directory-open authority and preserved native refusal | 1 | 0.230 s | [Authority](t030-open-directory-authority.log) |

[t030-results.json](t030-results.json) retains the exact argv, exits, timeout
flags and executable checks. The five groups have disjoint test names; no
earlier T029/T005 repeat is added to this count. The durations include fixture
and process work and are not measurements of task/model/tool performance.

The controlled path checks presentation and tool availability. Grep is actually
executed and its result reaches the provider. Code intelligence is checked for
registration only; indexing quality, LSP behavior and task effectiveness are
not established. Optional embeddings, real sandbox/VM execution and paid model
planning are outside this qualification. Retained sandbox-unavailable warnings
must not be interpreted as successful native command qualification.

## Candidate identities and preparation

The frozen Work libtest SHA-256 is
`5221893e6ab6d951e477720518ebe0bb8fb6ccbc18b9da661c38e7bb86c3ab22`.
The signed staged ACP and Office clone identities are:

- ACP: `61a06ab66f5f43ea89b2d1a909da5ef146bef2802b9526e75e8792a61d9e09f7`.
- Office: `31c6caec6406e0aa1039cabff0651583784b642b493d6f1f8d8cfa082fac0a25`.

All three retained hashes were unchanged after the five groups. Their source
and clone paths are in [t030-binaries.json](t030-binaries.json).

The [paired root build](paired-build.json) of terminal, ACP, Office and Work
passed in 445.136 s; [just build-work](build-work.json) passed in 102.507 s.
These are build durations. The [renderer build](renderer/OUTCOME.md) and
[signed development stage](stage.json) retain their own artifact identities.
The app is staged separately at
`target/development-stage/task-optimization-r2-20261020/AsterDream-TaskOptimization-R2.app`.
The prior installed app was not modified.

The initial stage used port 4322, which the existing renderer allowlist refused.
Its evidence remains in [stage-initial-4322.json](stage-initial-4322.json) and
the [initial launch record](stage-launch.json). The stage was corrected to
already allowed port 4319 and its signature reverified; see `port_correction`
in [stage.json](stage.json). This correction did not widen permissions. The
4322 launch record is not evidence of a successful corrected-port launch or
paid task submission.

## Source lineage and remaining gates

The [post-build source guard](owned-source-after-paired-build.json) records
42 of 43 owned source hashes unchanged. `crates/code/core/src/porcelain.rs`
changed from frozen
`b081ecf4f5cfb9bde5efc4dfa03975984925b2449beac3256672f4b0acf85572` to
`0372c33a9515668a549364eeee4ab64ef48cf02866483374ee0d881658e9199c`.
Independent review identified concurrent semantic changes, not only formatting.
The binary candidates are frozen by SHA; the latest entire live source tree is
not asserted to be identical to the earlier qualified source snapshot. Those
new porcelain semantics are not included in the earlier T005 GREEN proof.

The existing [T029 controlled summary](t029-summary.json),
[T005 regression result](t005/OUTCOME.md) and
[workspace CI gaps](WORKSPACE-CHECKS.md) keep their original bounded scopes.
Corrected-port CUA observation and T032's fresh T01 submission, independent
oracle, final paths and timing remain separate root-owned evidence. This
document does not mark T032 complete or assert a task-speed improvement.
