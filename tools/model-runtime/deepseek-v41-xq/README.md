# Managed DeepSeek V4.1 Flash XQ runtime

This directory owns the Python IPC bridge used by the managed Xolotl backend in
`crates/power`. Start it through Power's typed ACL configuration, which owns the
GPU UUID, model identity, native process, readiness proof and request admission.

Build Power from the Aster root with its initialized Studio checkout and pinned
toolchain:

```sh
cargo +1.97.1 build --locked --release -p a3s-power --no-default-features --features server --bin a3s-power
python3 -B -m unittest discover -s tools/model-runtime/deepseek-v41-xq -p 'test_*.py' -v
```

The qualified single-4090D engine is Xolotl commit
`e9d0826d71c59072cbff77d2a31dd51f27ab7a12`. The Linux shared-CUDA build patch is
`xolotl-linux-shared-cuda.patch`. The separate
`engine-dspark-accounting.patch` reports actual speculative draft counts in the
existing DONE fields; it does not change sampling, routing or kernels.
For that accounting patch, `tools/v41/engine/ds41_generate.cpp` has SHA-256
`f21337a48d8d8fafa7408751cc27031dbdef6f30b46770e40b58fdf8d6f2076f` before the
patch and `cc8a236f86a6f01d55e501194d85b414fb44d38a3e3ac63e8fb33d6ecc11ae5d`
after it. Keep the existing CUDA 13 shared-library runtime path when rebuilding.

The measured deployment profile uses V3 mixed-quantization weights, FP4 KV,
1,048,576 context positions, exact expert routing, 24 CPU threads, a 144 GiB CPU
expert arena, prompt batch 8192, prefill chunk 256, decode PCIe allocation 0 and
batch PCIe allocation 0.25. Its optional indexer tile is 67,108,864 elements.
DSpark uses depth 4, confidence 6000 basis points and a 4096-token prompt
eviction threshold. The sidecar path and SHA-256 must be explicitly bound in
ACL; the main model and speculative sidecar have separate validation.

Power validates the sidecar as a bounded regular GGUF file before startup.
The bridge strips inherited engine tuning switches and accepts only the closed
launch envelope. DSpark readiness must be proven from the current process's
startup log. The upstream native engine's silent draft fallback is refused.
TEXT_LOG remains disabled. A speculative sidecar is not a separately published
model.

Review the complete measurements and retained failures in
`docs/evidence/deployment/2026-10-25-deepseek-dspark-prefill/`; integration,
rebuild and publication proofs are in
`docs/evidence/deployment/2026-10-25-deepseek-dspark-main/`.
The exact configuration is selected for this machine and checkpoint.
It is not a universal default or a hardware-limit claim.
