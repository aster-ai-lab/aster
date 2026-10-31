# Aster retrieval fixture v1

This synthetic Rust, TypeScript, TSX, Markdown and Chinese corpus qualifies
lexical evidence, exact identifiers, displayed snippets, and strategy/resource
accounting. Query relevance and required evidence are authored separately from
results. The negative control has no relevant document.
`require_first` explicitly labels the three exact-identifier first-place gates;
split-name and natural-language queries retain their measured ranks. Failed
quality gates remain in the emitted report before the example exits nonzero.

Run from the initialized repository root:

```sh
cargo run --locked --release -p aster-core --example workspace_retrieval_aster
```

The example executes the real public session, hybrid search and tool-rendering
paths for Lines/Recursive × RRF/MMR.
The lexical engine is fixed to Portable for this controlled strategy comparison;
the shipping native aster-vec engine has separate locked release and persistent
production qualification. This corpus does not prove equal native-engine quality.
It exits unsuccessfully on missing evidence,
displaced identifiers, manufactured negative-control hits, excess provider
requests or retained vectors after close. It reports the fixture digest,
per-query Recall@5/10, reciprocal rank, nDCG, repeated-file hits, source/chunk
sizes, independent provider request counts and Core batching counters, lifecycle
timing, query latency, vector bytes and rendered response bytes.

The provider returns valid, orthogonal document/query unit vectors so it cannot
manufacture semantic recall or favor an annotated document. The harness switches
from document to query vectors only after readiness; it does not classify text
or look up relevance labels. These are lexical/ranking control measurements,
not real embedding quality or model CPU measurements. Query timing includes
the control-provider callback and current-source verification, excludes rendering,
and has no remote service latency. Response bytes are not converted into tokens.

`legacyPrefixEvidencePresent` applies the previous sixteen-line prefix display
rule to the same returned chunks. It is a display ablation, not a second live
agent run or an estimate that an agent reads complete files.

Existing locked `workspace-retrieval-v1` labels and release resource thresholds
remain unchanged. Real-model or future strategy qualification must record its
own version and cannot overwrite this corpus to hide a regression.
