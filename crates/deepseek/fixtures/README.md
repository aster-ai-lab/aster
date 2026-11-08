`official-config.json` is unchanged public checkpoint metadata from
`deepseek-ai/DeepSeek-V4.1-Flash-1017`, revision
`1bae0e1088993b9b82de08f59de5be5a34e42170`:
https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/config.json

SHA-256: `8be45ce0476004a3f529fd896115a4a2e800a129ad2d3ec05b16050f52e21879`.
This is test artifact data, not product configuration. Preserve its raw bytes;
native runtime configuration uses ACL. No weights, credentials or user data
are included.

`src/weights/official.rs` contains the 48 LFS file names, sizes and SHA-256
identities returned by the same pinned repository's tree API:
https://huggingface.co/api/models/deepseek-ai/DeepSeek-V4.1-Flash-1017/tree/1bae0e1088993b9b82de08f59de5be5a34e42170?recursive=false&expand=false

The source response SHA-256 is
`077064b4deaeb2189f936e3d23550d47d354e94e7d41e4922f66c5b0e571d313`.
The retained response fields and source URL are in
`docs/evidence/power/2026-10-22-native-deepseek-h200/official-payload-manifest.json`.
These identities constrain accepted checkpoint artifacts. They do not prove
the shared cache's payload bytes have been verified yet.

`official-text-layer-descriptors.json` freezes the independent read-only
header audit of all 40 base-text layers: 93,415 tensor names, dtypes, shapes
and byte counts, hashed in sorted-name order with LF-separated records.
Source: `docs/evidence/power/2026-10-22-native-deepseek-h200/official-layer-descriptor-audit.json`,
bound to the checkpoint revision and index hash above. The fixture SHA-256
is `71ba7ec475bfab18ff37248b81613e08f6165fc3f942257c2958d3ea3cb7519b`.
It contains metadata digests only. Hermetic tests compare the compiled
model-owned parameter schema against these observations without model
payloads, network access or GPU allocation.

`mhc-golden.json` is a tiny independently computed FP64 Sinkhorn fixture:
four rows `[.2,-.5,1,.3]`, `[.1,.4,-.2,1.3]`, `[.8,-.1,.5,-.7]`,
`[.3,.9,-.4,.2]`; stable row softmax plus `1e-6`, column normalization,
then 19 row/column normalization pairs, each denominator including `1e-6`.
It has no model weights. The mHC Rust tensor composition follows
TileKernels' `tile_kernels/torch/mhc.py` at revision
`c1cf914a82e6901c679445f872051ce44ac2cc63`, source SHA-256
`9f37ef9a1a7d99f14c2e878056b5f53a56932275e3ec0fc94de125e79953fd52`.
The original MIT notice is retained in `licenses/TileKernels-MIT.txt`.

`engram-hash-golden.json` is an independent integer-hash fixture over five
current-first compressed-token windows. It uses the official layer-1/layer-14
tables and eight seed-0 multipliers, with XOR prefix products and each head's
exclusive table offset. The metadata came from NumPy 1.26.4 PCG64, matching
DeepInfer revision `9bd9cf243fc53ef9afbf8f04d391d989117d6ab9`; it is retained
in `docs/evidence/power/2026-10-22-native-deepseek-h200/engram-metadata-reference.json`.
The resulting table row totals are 384,006,168 and 384,016,682.

The official tokenizer is public artifact data at the same checkpoint revision:
https://huggingface.co/deepseek-ai/DeepSeek-V4.1-Flash-1017/blob/1bae0e1088993b9b82de08f59de5be5a34e42170/tokenizer.json

Its SHA-256 is `c90dfa01249db1be4245780a052ede752e1361c612ac6d08e2bdada7d599476b`;
its Git blob identity is `6a15814dd25c934028034531da744c689f87ff21`.
The complete raw-ID-to-compressed-ID mapping, serialized as u32 little endian,
has SHA-256 `c60a86322ec17b4142bfef3c57a8d81fb428550cdf487f88a4320cb59fe46481`.
Rust and the independent remote Python reference agree over all 129,280 IDs,
producing 99,092 compressed IDs and compressed PAD=2. Large tokenizer data
is not embedded as a test fixture. The read-only Rust audit example takes an
explicit public artifact path; it does not start inference or allocate a GPU.

Engram hash/gate references are TileKernels' MIT `tile_kernels/torch/engram.py`
at revision `c1cf914a82e6901c679445f872051ce44ac2cc63`, SHA-256
`db7b779e95885f7bd965ff3cb34d20818d013bd5a50be69994ef598da4fcb48c`.
Token compression and image-boundary gathering follow DeepInfer's Apache-2.0
`engram_metadata.py` and `gather_engram_tokens_zos.py`; their original license
is retained in `licenses/DeepInfer-Apache-2.0.txt`. Engram history is request-local
and commits only accepted prefixes. These tests do not validate GPU execution
or the full official embedding-table binding. The later sparse lookup/projection
fixtures are separate from the original hash/gate fixtures.

Main/window KV, shifted-mHC handoff and exact backbone sharing/route boundaries
use additional pinned DeepInfer sources. Their original Git blob identities,
SHA-256, licenses and paths are retained in
`docs/evidence/power/2026-10-22-native-deepseek-h200/kv-topology-router-source-reference.json`.
The source audit distinguishes HF defaults from generic runtime defaults:
SWA projects each layer's own hidden state, query-head norm is disabled,
compressed KV/index-K share sources 2/8/14/20, and router logits remain FP32
despite BF16 projection operands. Tiny fixtures and exhaustive finite-BF16
quantization/scoring tests establish CPU arithmetic boundaries, not GPU kernel
execution, full model generation or H200 performance.

`rope-fp32-cpu-golden.json` contains independent PyTorch 2.8.0+cu128 **CPU**
FP32 frequency and `torch.polar` values, with CUDA explicitly uninitialized
before and after capture. It follows pinned DeepInfer `rotary_embedding.py`
(source hash/revision are in the artifact), using bases 10000 / 160000,
64 rotary channels, and the official YaRN factor 16, original context 65536,
beta_fast 32 / beta_slow 1 and unit amplitude. The Python-double cutoff
calculation gives [15,25]; frequency, ramp and phase tensor operations use FP32.
Sample positions include 65535, 65536 and 1048575 without allocating a full
million-row cache or running model weights. There is no runtime Python dependency.

Budgets were declared before comparing the Rust implementation: CPU powf
frequency differences at most four ULP across math implementations; cache
trig absolute error at most 2.5e-7 **when using identical FP32 frequency bits**.
The latter isolates FP32 outer-product rounding and cos/sin layout at long
positions. Integrated small caches are additionally checked against independent
scalar phase/trig math and prefix invariance. These CPU budgets do not qualify
the CUDA frequency/cache path or excuse frequency drift at long context.
Actual CUDA/libdevice parity, full cache and model quality remain H200 gates.

`xq-codec-golden.json` contains ten small synthetic rows from Xolotl's exact
`tools/v41/v41_formats.py` at revision
`b3452666e07a6374d06bb8c2185dffa271e8f65f`, source SHA-256
`beb83fb64b7790a53ff55b117eb3d0d4db5970ffd3d059ae69b0bfe3bd5083bc`.
NumPy 1.26.4 freezes decoded F32 bits for PTQ1_0, Q2_0, Q3_K, Q8_0,
F16 and F32, including signed zero, subnormal/signed scales, all Q8 codes
and Q3 subscale/hmask positions. Digests also cover every packed byte
value for PTQ1_0 and Q2_0, and all 63,488 finite FP16 encodings.
No model weight payloads are included.

Fixture SHA-256:
`feb2bbc4c07befda07c77f5374d87a4c625abd6c0d6d320464c9721dfc0c8c21`.
The generator is retained as
`docs/evidence/power/2026-10-22-native-deepseek-h200/xq-codec-generate.py`;
source hashes are in `xq-codec-reference-manifest.json` beside it.
The MIT notice is in `licenses/Xolotl-MIT.txt`. Rust tests require only
fixed fixture data, with no Python, NumPy, network or GPU dependency.

The real XQ header audit separately records 1,329 tensor descriptors,
68 metadata keys and 16 KiB alignment. Its 5,865,472-byte header SHA-256 is
`a8f516bb9bc7304eaf02f018fc6055b701e34cc17e939936a90c38f322abf511`.
The Rust diagnostic matches the independent catalog exactly. The complete
header/tokenizer stays in operator scratch space, with the bounded map
extraction below retained for hermetic tests. Header validation does
not verify tensor payloads, register a backend, or prove CUDA inference.

The opt-in XQ H200 projection test consumes the same ten golden rows directly,
rotates whole blocks to give each output row distinct bytes, and compares
warp-reduced F32 dots to independent golden values accumulated in F64.
The frozen absolute tolerance is `2e-6 * max(1, sum(abs(products)))`;
activations are bounded by 0.125 to keep the F32-max fixture finite. Partial
warps, multiple tokens, all six floating formats, signed/subnormal scales and
row/block addressing are included. Successful NVRTC/ptxas compilation does
not execute this test; it remains explicitly ignored without an owned H200.

The subsequent XQ gather test reads distinct, repeated and nonordered rows
of every golden format and requires every returned F32 bit to match the
independent vectors, including signed zero and raw F32 extremes. The text
endpoint/mHC H200 test uses exact F16 embedding/mix operands, two shifted F32
updates and a Q8_0 head, with a separate F64 reference. Its frozen bounds are
8e-6 absolute for carried mixed inputs and 3e-5 for final logits. These are
small component tests, not a 40-layer generation or task-quality campaign;
both H200 tests remain opt-in and unexecuted while allocation is pending.

`xq-engram-token-map.u32le` is the unchanged public Engram compressed-token
map extracted from `deepseek41.engram.token_map` in that pinned XQ header:
129,280 little-endian U32 values, 517,120 bytes, SHA-256
`c60a86322ec17b4142bfef3c57a8d81fb428550cdf487f88a4320cb59fe46481`.
It agrees with the independently pinned official tokenizer's compressed map.
It contains model metadata, with no model weight payload or user data.
Extraction offsets, types and every Engram array digest are retained in
`xq-engram-metadata-audit.json` in the same evidence directory. Production
reads the actual authenticated GGUF array through Power and compares its
complete identity; the fixture is not a runtime substitute.

The XQ Engram H200 test checks original sparse E4M3/E8M0 row decode bits,
including duplicate/nonordered heads, signed zero and scale-code-zero
subnormals. Separate independent F64 projection/gate stages use a frozen
`2e-5 + 2e-5*abs(expected)` budget, with a declared F32 projection boundary
and F32 inverse-square-root scale. It also checks the carried mHC pre-mix
and exact image-trunk bits. It does not qualify giant-table residency,
whole-model logits, throughput or service delivery.

`xq-model-config.json` is the unchanged 2,270-byte UTF-8 value of
`deepseek41.config` in the same pinned header (byte range 343..2613), SHA-256
`d7637228d27528f6bd259781b5a27258068f50bf637c9c83aab784d81579669d`.
It has the same JSON values as the official metadata fixture, while its model
identity, serialized artifact hash, quantized payload and native numerical
profile remain separate. Production accepts it only for the immutable XQ
revision and reads it from the verified GGUF owner. This is model artifact
metadata; runtime configuration remains ACL.

The loaded XQ source/consumer H200 test executes actual shifted attention,
compression/index selection, packed caches and mixed/shared FFN for layers
2/3 and 20/21 of a reduced-width fixture. All other layers explicitly supply
synthetic state to exercise the 53-participant transaction. The frozen
two-layer composition bound is `2e-4 + 2e-4*abs(expected)` for whole versus
chunked output; cache bytes must match exactly after partial acceptance.
The test also refuses a foreign prepared projection and reduced-precision
F32 GEMM, restoring the process-global BLAS flag in a guard. Run hardware
tests serially; production never silently changes BLAS modes. This is not
a full 40-layer model, giant-table, peer-transfer or eight-card qualification.
