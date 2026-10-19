# T013: same-source SHA command-latency comparison

Status: the bounded macOS OCI comparison passed. No application selection,
installed release, original cache or earlier candidate assets were replaced.

## Result

The same-source software SHA build averaged **10.180 s** per command; the
hardware SHA build averaged **3.438 s**, a **66.23% reduction in this OCI
microbenchmark**. Both variants passed all 38 samples. This comparison isolates
the SHA feature change; the earlier cached-runtime comparison changed source
and therefore remains a separate historical result.

| Variant | Samples | Mean | p50 | p95 | Range |
| --- | ---: | ---: | ---: | ---: | ---: |
| Software SHA | 38 | 10.180 s | 9.706 s | 13.043 s | 9.513–13.433 s |
| Hardware SHA | 38 | 3.438 s | 3.426 s | 3.926 s | 3.138–4.179 s |

p50 is the median; p95 uses nearest rank. These small serial samples do not
establish a reliable tail-latency estimate. All samples, including slower
software samples, remain in [t013-comparison.json](t013-comparison.json).

| Mean phase | Software | Hardware |
| --- | ---: | ---: |
| Host connect | 1.667 s | 0.452 s |
| Bundle preparation | 0.345 s | 0.350 s |
| VM create | 7.856 s | 2.317 s |
| Process start | 0.067 s | 0.064 s |
| Collect and wait | 0.077 s | 0.075 s |
| VM delete | 0.165 s | 0.175 s |
| Host shutdown | 0.001 s | 0.001 s |
| Scratch removal | 0.002 s | 0.002 s |

The measured difference is concentrated in Host connection and VM creation.
Bundle copying, command execution and settlement have approximately the same
cost. No integrity check, isolation boundary or settlement deadline was removed.

## Controls and qualification

- Both variants were built from the root workspace with the same pinned
  toolchain, profile and `target/t013-sha-build` directory. The software build
  adds `sha2/force-soft`; the hardware build omits it. Both retain `asm`.
  In sha2 0.10.9, `force-soft` takes precedence over `asm`.
- The 747-file runtime source manifest was checked before both asset
  snapshots. Both signed asset sets contain 950 files; only Host and shim
  binaries differ. The immutable image, manifest, runtime libraries and
  rootfs bytes match. Signing uses the same checked-in HVF entitlement.
- The same copied Core test executable, with a recorded SHA-256, ran every
  sample. No Cargo invocation occurred during timing. Team compilation,
  model calls and other intentional VM work paused for the timing window.
  Background and unrelated user-process activity were not controlled. Only
  start/end load averages were retained; per-process CPU was not sampled
  throughout the rounds. No whole-machine-idle claim or retrospective
  assumption about later-observed workloads is made.
- Each sample executes `printf phase-probe` through a new Host and
  `DedicatedVm`, with a 5,000 ms command deadline. It checks exact stdout,
  empty stderr, exit 0 and `timed_out = false`, retaining the complete receipt.
- Both signed builds passed the three existing real-VM governance tests:
  streaming/receipt and credential/hardlink/Git protection, cancellation and
  settlement, and timeout settlement of a `setsid` descendant. **6 passed,
  0 failed**; their preparation timings are excluded from performance data.
- Four rounds used balanced pairs AB/BA: software-1, hardware-1, hardware-2,
  software-2. Each round passed 19/19. The four timed processes took
  191.120, 65.831, 64.896 and 195.820 seconds respectively.
- First samples are separate in the JSON: software 9.565/9.660 s; hardware
  3.138/3.439 s. Each round verifies assets first, which reads their bytes.
  No cache flush or cold-machine claim is made.
- All 76 receipts and 608 unique, finite, nonnegative phases passed parsing,
  ordering and phase-sum checks. No failed or excluded sample occurred.
- Post-run hashes match the frozen probe, both 950-file asset sets and the
  original 950-file candidate manifest. The original cache's three hashes
  tracked in `baseline.json` also still match. Current `Cargo.lock` changed
  through parallel work after the two snapshots; it is not represented as
  the build-time lockfile identity. Frozen binaries were unchanged.

## Reproduction and retained evidence

Root build commands, in order; snapshot Host, shim and the adjacent
`a3s-oci-krun-runtime` directory after each pair:

```sh
cargo build --locked --offline --target-dir target/t013-sha-build -p a3s-oci-cli --bin a3s-oci --features sha2/force-soft
cargo build --locked --offline --target-dir target/t013-sha-build -p a3s-oci-krun --bin a3s-oci-krun-shim --features sha2/force-soft
cargo build --locked --offline --target-dir target/t013-sha-build -p a3s-oci-cli --bin a3s-oci
cargo build --locked --offline --target-dir target/t013-sha-build -p a3s-oci-krun --bin a3s-oci-krun-shim
```

The build-time lockfile digest was retained, but its contents were not
archived before parallel changes. These commands describe the performed
recipe; they do not establish a bit-for-bit rebuild from the current working
tree. The preserved signed artifacts and frozen probe allow the measured
comparison to be replayed without rebuilding. Future experiments should also
archive their build-time lockfile contents.

The ignored probe extends the existing fixture; it adds no production setting
or benchmark framework. Build it once, copy its executable, and use that copy
for all rounds:

```sh
cargo test --locked --offline -p a3s-code-core --lib real_command_phase_timing --no-run --message-format=json
```

Exact executable paths, SHA-256, environment override and per-round arguments
are in [t013-harness.json](t013-harness.json) and
[t013-round-execution.json](t013-round-execution.json). Each round also has its
unmodified `t013-round-<variant>-<round>.log`. Raw feature trees, build logs,
unsigned/signed binary hashes, file hashes and signing commands are retained
in `t013-features-*`, `t013-build-*` and `t013-assets-*.json`.

Replay validation and statistics without launching a VM:

```sh
/opt/homebrew/bin/python3.13 docs/evidence/sandbox/2026-10-19-command-latency/t013-analyze.py
```

The analysis self-check covers the first round's 19 receipts/152 phases and
known median/nearest-rank examples. Full replay passes all four rounds. The
new ignored-probe block matches rustfmt output; focused `git diff --check`
passes. The already-recorded whole-workspace formatting/dependency failures
were not reclassified or hidden.

## Explicitly not claimed

- No complete T01, real-model, program-tool/profile or end-to-end application
  speed improvement follows from this microbenchmark. No subtraction from
  historical T01 wall time is presented as a predicted result.
- The measurement calls `OciBashSandbox` directly. It does not include the
  entire Bash tool's workspace observation or model/control overhead.
- No Windows/Linux qualification, release packaging, application switch,
  VM reuse, reduced isolation or omitted verification is claimed.
- The initial three-sample candidate result and the controlled 76-sample
  result use distinct evidence and must not be merged into one distribution.
