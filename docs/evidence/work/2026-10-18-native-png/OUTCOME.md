# In-process PNG chart artifacts

`artifact_image::render_png(&str)` converts model-supplied SVG into PNG bytes
with the existing local `a3s-code-mermaid::rasterize` API. It does not interpret
task meaning or publish a file. The `office_file` caller owns workspace
authorization, cancellation, credential-path refusal, and atomic no-clobber
publication.

The adapter rejects empty/malformed SVG, external references, DTDs and XML
processing instructions. It excludes inline CSS and resource/script/filter
elements; ordinary presentation attributes and internal gradient references
work. Root dimensions are numeric or `px`, or come from `viewBox` when width
and height are both absent. Namespace-prefixed lookalike size attributes
cannot shadow the actual root dimensions.

Limits are 64 KiB of SVG, 2,048 elements, 32 levels, 64 attributes per element,
4,096 pixels per axis, 8 megapixels, and 16 MiB of PNG. The underlying renderer
also disables external image resolution. No process execution, external
renderer, font download, or new external library/version is introduced.
The adapter reuses the existing locked `quick-xml` 0.38 parser.

## Validation

`rustfmt --edition 2021 --check crates/code/acp/src/artifact_image.rs
crates/code/acp/src/artifact_image_tests.rs` and `git diff --check --` for those
files passed.

The current source and sibling test module were freshly compiled directly
against the existing Cargo-built dependency libraries:

```sh
rustc --edition 2021 --test crates/code/acp/src/artifact_image.rs \
  --extern a3s_code_mermaid=target/debug/deps/liba3s_code_mermaid-36b99be6c82f9e1a.rlib \
  --extern anyhow=target/debug/deps/libanyhow-03d363a4996c5c32.rlib \
  --extern quick_xml=target/debug/deps/libquick_xml-1f0414f4b57dc195.rlib \
  -L dependency=target/debug/deps -o target/tmp/artifact_image_tests
target/tmp/artifact_image_tests --nocapture
```

Result: **4 passed, 0 failed, 0 ignored**. The cases cover PNG signature and
IHDR dimensions, explicit dimensions and internal gradients, external/file/
relative/data references and XML/CSS resource surfaces, malformed/empty/
oversized/deep/element/attribute/pixel inputs, and namespace size-shadowing.
On this macOS host, Chinese characters `中` and `文` each produced visible,
different glyph images rather than blank output or identical missing-glyph
boxes.

The initial `cargo test -p a3s-code-acp --lib artifact_image -- --nocapture`
attempt was cancelled while waiting for the shared Cargo build lock (exit
130, no tests run). The independent module run above compiles this exact
source; it is not an old test binary and is not a claim that the entire
concurrently edited Cargo workspace builds.

## Explicitly not claimed

No macOS shell execution has been enabled. No live provider or `../tasks`
evaluation was run for this module. The renderer uses existing system-font
fallback for CJK; the bundled primary font alone does not provide CJK, and
this host's glyph test does not establish coverage on every other machine.
Input/output bounds do not provide independent process crash isolation or a
hard wall-clock kill for arbitrary hostile SVG. Publication after
cancellation is governed by the artifact caller, not this pure byte renderer.
