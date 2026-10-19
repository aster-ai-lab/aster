# Aster v0.1.0 — Kylin V10 x86_64 offline installer

Result: built the Work desktop host, ACP and Office with Rust 1.97.1 from the root workspace. Delivered `aster_0.1.0_kylin-v10_x86_64.run` for the repository's Kylin V10 SP1 desktop / apt-deb / glibc 2.31 target. No commits or pushes were made.

Artifact directory: `target/packages/aster-0.1.0-kylin-v10-x86_64-20261019-111709`.

- Size: 564,201,012 bytes.
- SHA-256: `da94a76227f19be2775b2ff7bf383e6fa8d8d9324f5d36f9c75da465b062f454`.
- Source baseline: `b4eb912be092d18df66e86d02e5dbdb3978d27d4` with existing local changes.
- CPU target: `x86-64`; no AVX2 target flag. Runtime libc reports x86-64-baseline and Linux ABI 3.2.0.
- Runtime: GNOME Platform 50, GL.default 25.08, Python 3.13.16, Node 24.22.0. ACP, Office, npm, uv, pinned Python packages and wheels, Chinese font, tray libraries, skills and offline Flatpak deb dependencies are included.

## Changes

- Build from this root workspace and isolate Linux Cargo outputs under `target/kylin-x86_64`. Use `TAURI_CONFIG` without rewriting the source configuration. Mask the checkout's `.env` during compilation and packaging.
- Replace the retired GNOME 47 / GL 24.08 runtime pins together with GNOME 50 / GL 25.08 and matching Python 3.13 wheels.
- Name the delivery `aster_0.1.0_kylin-v10_x86_64.run`. Interpret padded self-extraction offsets as decimal. Use a gzip payload so the base-system installer does not depend on xz-utils. Exclude macOS extended attributes from the outer archive.
- Generate `.flatpak.commit` files from the actual bundles via Flatpak import/repository commands, include them in SHA256SUMS, skip only exact installed commit matches, and verify the deployed commit after installation. Prefer `--reinstall` for different local bundle commits.
- Collect the host's launch log during the smoke cleanup. Use a short fixture instance ID that fits Unix socket path limits.

## Verification

- `python3 tests/kylin_onefile.py`: 1 test passed; exercises the actual self-extracting installer and rejects truncated payloads.
- `bash apps/work/scripts/kylin-flatpak/install-on-kylin.sh --self-test`: passed. Covers intact/corrupt/missing payloads, identical/different commits, invalid/missing commit files, and mismatched deployed commits.
- `bash -n apps/work/scripts/kylin-flatpak/*.sh` and scoped `git diff --check`: passed.
- Focused workspace dependency checks (`test_office_kernel_rebuilds_from_the_workspace`, `test_products_share_one_flow_engine`): 2 passed.
- Release build: ACP + Office succeeded, then `aster-dream --features custom-protocol` succeeded. All three are stripped Linux x86_64 ELF files.
- Renderer scan: 2,639 files checked against local secret values, no matches. Host/ACP/Office scans: no matching local credentials. App Flatpak metadata contains no local capability credentials. Source Tauri configuration hash is unchanged.
- Network-disconnected Ubuntu 20.04 amd64 / glibc 2.31: bundled apt repository installed Flatpak 1.6.5 without downloads. User namespaces work.
- Final `.run` on that host, without xz: first install exit 0; repeat install exit 0; fixture user data retained; no macOS xattr warnings. Duration 17.36s. Installed runtime, GL and app commits match their bundled manifests. Installed host/ACP/Office bytes match the checked build outputs.
- Isolated GNOME 50 + GL 25.08 `smoke-bwrap.sh`: all native dependency checks passed; ACP/Office help, Node/npm, channel gateway, Python format packages and uv offline resolution passed. Login page loaded and the process stayed alive until the intentional 60-second timeout (exit 124). Host log markers and screenshot retained here. No sandbox-disabling flags were added to the product.

Build and full test logs remain in the artifact directory. Earlier failed packaging candidates and logs are retained in its `superseded-*` directories.

## Diagnoses and abandoned approaches

- The old padded payload offset was interpreted as octal; the regression fixture reproduced it before the decimal fix.
- The initial xz payload could not unpack on the minimal target ABI container because xz-utils was absent. gzip removed that bootstrap dependency.
- `--or-update` did not handle installed local bundles on Flatpak 1.6.5; `--reinstall` alone also failed for the identical commit. Exact commit comparison fixes the repeated-install case without ignoring install errors or deleting app data. This agrees with the [Flatpak 1.6.5 bundle implementation](https://github.com/flatpak/flatpak/blob/1.6.5/common/flatpak-dir.c#L8616) which rejects an already deployed identical bundle commit.
- The first smoke's stdout-only marker check missed logs redirected to the launch file. The application was running and its page-load markers were present; the test harness now collects that file. A long fixture instance ID also exceeded SUN_LEN; the shortened fixture passes.

## Explicitly not claimed

- No physical Kylin V10 hardware, real GPU, Chinese IME, account login, online chat/search, or non-AVX2 hardware acceptance was performed. ARM, Kylin Server/RPM and other desktop versions were not tested.
- Native `flatpak run` seccomp execution cannot be validated through this Mac's Rosetta environment (`prctl(PR_SET_SECCOMP): Invalid argument`). The installer reports its sandbox postcheck warning there. Startup evidence is the isolated bwrap/Xvfb run, not proof of a native Kylin sandbox launch.
- No local OCR model weights, user accounts, sessions or development credentials are bundled.
- The whole workspace test suite and full CI are not claimed. Known unrelated workspace/format baselines were not changed.
- This Linux artifact has not been uploaded to GitHub Releases in this task.
