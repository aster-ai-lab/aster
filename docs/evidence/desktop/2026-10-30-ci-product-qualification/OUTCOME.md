# Restore product qualification after Capture integration

Base Aster: `56e0a88f2df07d4a6bfd5dc33686610353aa2014`.
Base Studio: `bbee62af5317c40f9bea6e2458d28edaf6e2b5c8`.
Qualified Studio: `999db5ac8627bdbc966ba86f117c3689167b994e`.

## Result

- The real package-host fixture now supplies the Capture helper and copies the
  owning microphone-usage plist into both Aster/ToWork test bundles. It reopens
  each final ZIP and compares the helper bytes, identity, source microphone text,
  tenant resources and sealed file hashes. Production rejection of missing helper
  or microphone purpose is retained.
- Replaced the retired `towork-cua` branding case with released `aster`/`toyou`
  side-profile identity and asset checks. The retired resource intentionally says
  `Former`; its source is preserved. Closed product-name admission stays tested.
- Shared computer-use tests use each compiled tenant's existing preference store:
  Aster local profile, ToYou member scope. All six permission/admission cases run
  for both. Codex tests and their fixtures compile only for the Aster feature,
  matching the production module. No local API was exposed to ToYou.
- The provider-isolation assertion now accounts for the compiled policy. Aster
  hides the managed provider with `Missing`; ToYou must reject a caller-supplied
  `aster` name with `managed_gateway`. Both retain strict refusal of local keys.
  The identity matrix now runs provider and computer-use cases in CI as well.
- Capture's CPAL dependency is limited to its supported macOS/Windows targets.
  The empty Linux library no longer pulls ALSA into workspace checks. macOS and
  Windows resolved graphs retain CPAL. The root lockfile is unchanged.
- macOS workspace CI installs `lld`, required by the vendored libkrun guest-init
  Makefile's Apple-Clang `-fuse-ld=lld` invocation. Compiler, ABI and linker flags
  are unchanged; no release workflow was replaced.

## Reproduction and validation

Remote failures were read from Rust run [41048729019](https://github.com/aster-ai-lab/aster/actions/runs/41048729019)
(package fixtures on both operating systems), backend run
[41048728987](https://github.com/aster-ai-lab/aster/actions/runs/41048728987)
(Linux ALSA), and earlier Rust run
[41040740743](https://github.com/aster-ai-lab/aster/actions/runs/41040740743)
(retired tenant assertion and macOS linker).

The package fixture's two tenant failures and retired-brand failure were rerun
alone before correction. The ToYou native build separately reproduced seven
feature-mismatch compiler errors. The extended provider suite then exposed one
wrong-tenant assertion (14 pass, 1 fail); the exact failing binary case was rerun
alone (0 pass, 1 fail) before correcting the feature-specific refusal expectation.

Checks used macOS arm64, pinned Rust 1.97.1 and isolated worktree sources.
Tenant environment/config came from the existing product config generator.

| Check | Result |
| --- | --- |
| `python3 -m unittest scripts/tests/studio_delivery_test.py scripts/tests/studio_product_test.py` | 30 passed |
| Native `tenant::`, features `tenant-aster` / `tenant-toyou` | 15 / 15 passed |
| Native `agent_plane::computer_use_tests::`, same features | 6 / 6 passed |
| Native `providers::`, same features | 25 / 15 passed |
| Studio Clippy, each tenant, all targets, `--no-deps` | Both exited 0 with warnings |
| Capture Clippy, all targets, `--no-deps` | Exit 0 with warnings |
| `cargo check --locked --offline -p capture --lib --target x86_64-unknown-linux-gnu` | Exit 0 |
| Resolved Capture dependency graph | Linux CPAL/ALSA removed; macOS/Windows CPAL retained |
| Apple Clang guest-link probes, AArch64 and x86_64 Linux | Both fail without lld; both produce correct ELF machine IDs with lld |
| `cargo fmt --all -- --check` | Exit 0 |
| `python3.13 tests/workspace_dependencies.py` | 6 passed |
| `python3.13 tests/just_work.py` | 10 passed, 1 Windows-only skipped |
| `python3.13 tests/release_governance.py` | 9 passed |
| Ruby YAML parser, CI workflow | Passed |

Native commands, source hashes and full logs are in `checks.tar.gz`. The two
linker outputs were reopened and checked for ELF magic, machine 183/62, size and
SHA-256. The archive was reopened and all 48 entries compared with their
input bytes; `manifest.json` retains every size/hash.

One isolated Cargo attempt stalled before build-script code at dyld startup. Its
valid-on-disk code signature and process sample were recorded; only that owned
process was stopped. That attempt is an infrastructure interruption, not a failed
unit assertion. The separate already-built test binary supplies the independent
failure evidence; subsequent fresh Cargo test/Clippy runs completed. An initial
workspace guard invocation used the system Python 3.9 without `tomllib`; the final
recorded guard uses Python 3.13 and passes all six cases.

## Explicitly not claimed

- A new release, installed app/CLI, final distribution smoke, or changed channel.
- Linux/Windows recording support or runtime acceptance. Linux checking is cross
  compilation; Windows evidence here is dependency resolution only.
- A complete libkrun build or VM boot from the linker probe, or a newly green
  remote CI run. New remote qualification follows the push.
- A warning-free tree or completed frontend/native GUI acceptance for this
  follow-up. No renderer source changed; earlier Capture evidence keeps its scope.
- Changes to account settings, credentials, billing, system permissions, original
  dirty working files, or unrelated worktree sources. No frozen eval was changed.
- GitHub recognition of the existing signing key. Local GPG signatures and DCO
  are verified; GitHub can still report `unknown_key`.
