# Studio companion staging build fix

Verified on 2026-11-06, macOS arm64, using the initialized `apps/studio`
submodule. Unrelated checkout changes were preserved; no commit was created.

## Result

- `stage-companions` no longer requests the removed Alpine
  `python3-3.12.15-r0.apk`. The pinned package is now `python3-3.12.16-r0.apk`,
  with size `8093823` and SHA-256
  `d21dd05b578d14c84a735d66d8623e9454bdc18a1b3841815b37948ce3e2aa92`.
  The remote package returned HTTP 200 with the same size and hash.
- The Linux PTY setup uses Tokio 1.53.3's `AsyncFd::register_with_interest`
  instead of deprecated `AsyncFd::new`. The `OwnedFd` safety invariant is
  documented at the unsafe call, and existing error handling is preserved.

## Verification

Commands run from the repository root unless stated otherwise.

1. The failing download was identified with:

   ```bash
   PYTHONPATH=apps/studio/scripts python3 target/kimi-input/trace_stage_downloads.py \
     aster stage-companions --target-dir /Users/roylin/code/aster/target/studio/aster
   ```

   It identified
   `https://dl-cdn.alpinelinux.org/alpine/v3.22/main/aarch64/python3-3.12.15-r0.apk`
   as the HTTP 404 response.

2. `python3 -m unittest apps/studio/scripts/tests/sandbox_assets_test.py`
   passed **3/3 tests**.
3. `cargo fmt --all -- --check` passed after the final Rust change.
4. `cargo clippy --locked -p aster-oci-agent --all-targets --no-deps`
   passed on macOS. The Linux executor is not compiled by this host-only check.
5. `cargo clippy --locked -p aster-oci-agent --all-targets --no-deps --target aarch64-unknown-linux-musl`
   passed, compiling the Linux executor and its test targets without a Clippy
   finding in the changed code.
6. `just build-studio` passed after both final fixes, including release
   compilation of `aster-oci-agent` for `aarch64-unknown-linux-musl`, companion
   staging, and the Studio host build. Existing unrelated Rust and linker
   warnings remain.
7. `git diff --check` and `git -C apps/studio diff --check` passed.

## Explicitly not claimed

- The focused command `cargo test --locked -p aster-oci-agent terminal` ran
  **0 tests** on macOS because the executor is Linux-only. No native Linux PTY
  runtime test or terminal behavior pass is claimed; Linux compilation and
  companion staging are the checks that passed.
- No Windows runtime, release installer, GUI end-to-end scenario, deployment,
  or warning-free workspace build is claimed.
- Unrelated model-selector, message-list, provider, and other existing checkout
  changes were not reviewed or modified as part of this fix.
