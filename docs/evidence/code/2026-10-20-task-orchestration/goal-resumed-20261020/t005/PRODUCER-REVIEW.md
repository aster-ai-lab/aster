# T005 bounded before-image producer review

The producer change is confined to `crates/code/core/src/porcelain.rs` and its
`porcelain/receipt_tests.rs` sibling. The foreground receipt owner calls the
new consuming `Watch::finish_effects_with_revision` method. Existing PathDelta
fields and consuming finish methods remain compatible.

The method retains the same Watch's already sampled content identities while
running the existing after observer. It adds no workspace walk, content hash,
shell probe or permission outside the existing observation boundary. A
complete non-Git index can establish that a newly created path was Missing.
An existing path without sampled bytes stays unknown. A clean Git path absent
from dirty stamps also stays unknown: HEAD/index is not substituted for the
actual before-image. The original typed observation/canonical-size validation
still fails incomplete when its bound cannot be established.

The independent effect-fold review found no blocker in the bounded patch:
only validated complete effects with comparable identity changes and no
unresolved external operations mark progress; the new counting boundary is
published after the complete model tool batch. Same-byte rewrites, one-sided
mode observations and unknown before-images do not establish progress. Exact
retained suffixes translate the boundary at compaction; unrelated retained
messages conservatively count the complete retained history. The model's
conversation history is retained.

The root's actual
[core-connected-03.log](core-connected-03.log) reports **33 passed, 0 failed**
for `porcelain::receipt_tests::`, including all four new producer regressions:

- Known non-Git and dirty Git bytes, followed by a same-byte rewrite.
- A first clean Git edit with unknown before-image.
- Complete non-Git absence for creation and known bytes for deletion.
- An existing path whose byte baseline exceeded the 64 MiB bound, retained
  as unknown rather than fabricated Missing.

The same combined frozen Core artifact also passed the two actual original
Run producer-to-fold tests in
[core-connected-02.log](core-connected-02.log): changed bytes permit the fourth
identical verification, while same-byte rewrites retain the guard. Artifact
SHA-256: `92486c70ac258c5e1c43b889818e0b00b3438ba27b7ef0cbd42551284db0e3a3`;
commands and timing are in [core-connected.json](core-connected.json).
Producer receipt tests use temporary files and local Git fixtures; the two
original Run tests use a controlled provider and the real built-in write
tool. They do not invoke a paid model or a VM. This worker did not run Cargo.

## Explicitly not claimed

This does not capture every clean Git file's first before-image, establish
actual Windows observation support, prove an earlier T027 refusal's unique
cause, or demonstrate live T01 speedup. No historical task result was repaired
or discarded. Product build and CUA evaluation remain separate root-owned
gates.
