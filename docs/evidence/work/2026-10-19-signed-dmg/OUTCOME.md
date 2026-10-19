# AsterDream signed and notarized DMG — 2026-10-19

## Corrected delivery: notarized and Gatekeeper accepted

Use `/Users/roylin/code/aster/target/packages/asterdream-20261019-102647/aster_0.1.0_aarch64.dmg`. The original `_signed.dmg` is superseded.

- Apple submission `42ae68a7-255a-4bdf-9089-864cfcacace2`: `Accepted`, with no issues in `notary-log.json`.
- Existing iCloud Keychain profile `AsterDream-notary` validated and used without exporting its credentials. The earlier claim that local notarization credentials were unavailable was incorrect: the initial checks inspected environment variables and the default file-based keychain, and missed the data protection/iCloud keychain.
- `stapler staple` and `stapler validate`: passed for the corrected DMG.
- `codesign --verify --strict` and `hdiutil verify`: passed after stapling.
- Read-only mounted the final DMG; the app and both companions passed signature and arm64 checks.
- `spctl --assess --type open --context context:primary-signature --verbose=4` on the DMG: `accepted`, `source=Notarized Developer ID`.
- `spctl --assess --type execute --verbose=4` on the app inside the final DMG: `accepted`, `source=Notarized Developer ID`.
- Final size: 68,946,966 bytes. SHA-256: `a96e66763d37859b58917fb88c8a1653108bcded033cab14ea3fab4ad366aac3`.
- Submission, service log, final verification, and checksum records are retained in `/Users/roylin/code/aster/target/packages/asterdream-20261019-102647`.

The initial delivery was incomplete: it was knowingly delivered after Gatekeeper rejected it as unnotarized. Signing alone was incorrectly used as the delivery threshold. The corrected artifact above passes the missing notarization and Gatekeeper checks.

## Initial build

Built AsterDream 0.1.0 for Apple Silicon from the current workspace with Rust 1.97.1 and pnpm 12.6.0, using `just build-work-release`. Office WASM, the renderer, ACP, Office CLI, and the Tauri host completed successfully. The version number remains 0.1.0.

- Initial artifact (superseded): `/Users/roylin/code/aster/target/packages/asterdream-20261019-102647/AsterDream_0.1.0_aarch64_signed.dmg`
- Size: 68,945,234 bytes.
- SHA-256: `9d076a82be7a27e73014fa35c475d49f405b9552a44dd81d1b230b0e20676c56`.
- Signer: Developer ID Application: Lijin Zhu (5L9P7TS6DM).
- Build logs and verification records: `/Users/roylin/code/aster/target/packages/asterdream-20261019-102647`.

## Build prerequisite repaired

During the first build, commit `3903a565` changed three Flow dependency paths to the absent `crates/flow-v2` directory. This prevented Tauri from reading Cargo metadata. Corrected only those paths in `Cargo.toml`, `apps/api/Cargo.toml`, and `crates/code/core/Cargo.toml` to the existing `crates/flow`, preserving versions, features, and other edits.

No commit was made for this packaging task.

## Verification

- `just build-work-release`: passed on the second attempt; the first attempt completed ACP and Office compilation and then failed on the missing Flow manifest.
- `/opt/homebrew/bin/python3.13 tests/workspace_dependencies.py WorkspaceDependenciesTest.test_products_share_one_flow_engine`: 1 passed.
- Full `tests/workspace_dependencies.py` preflight: 5 passed, 1 failed because existing `a3s-gateway` metadata has `publish = null` instead of `false`.
- `cargo fmt --all -- --check`: failed on existing working-tree formatting differences; no formatting was applied.
- `codesign --verify --strict`: passed for the DMG, mounted app bundle (with `--deep`), main executable, ACP, and Office CLI. All five code objects have timestamped Developer ID signatures for team 5L9P7TS6DM.
- `hdiutil verify`: passed. Mounted the delivered DMG read-only, checked its actual contents, and detached it after verification.
- `lipo -archs`: `arm64` for all three executables. Bundle version and identifier matched the Work configuration.
- Temporarily moved the local `.env` into ignored build cache without copying it, restored its original bytes after each attempt, and excluded its service credentials from this build. Build logs confirm no bundled search/document-parse defaults. Scanned all 7 regular files inside the mounted app for the three local service credential values: none found.
- `SHA256SUMS`, `verification.json`, `verification.log`, and build logs are retained beside the DMG.

## Installed startup verification after the reported flash exit

The two reported launches at 10:51:18 and 10:51:27 reached normal startup and exited because the existing development instance held the `aster` single-instance lock. Their launch logs (`aster-dream-20261019-105118-25673.log` and `aster-dream-20261019-105127-25768.log`) explicitly record `another instance holds aster; it was asked to show itself`. No crash was established for those launches.

- Confirmed the old development instance, PID 5132, was running from the support-directory app and serving the renderer from the development HTTP origin.
- Quit that instance normally through the application. Verified its process had exited before installing the release.
- `/Applications/AsterDream.app` did not exist. Installed it from the notarized DMG with `ditto`; signature and Gatekeeper checks passed before and after copying.
- Launched the installed app. PID 26233 ran from `/Applications/AsterDream.app/Contents/MacOS/aster-dream` and remained alive after 31 seconds. Native accessibility confirmed the main window and renderer were loaded from `tauri://localhost/windows/main/index.html`.
- Launch log `aster-dream-20261019-105317-26233.log` records the installed executable and main-page load, with no panic or fatal startup entry during the observation.
- The installed app was left running. No source repair or new build was required for this incident.

## Installer filename

Renamed the final notarized installer to `aster_0.1.0_aarch64.dmg` as requested. This changes only the artifact filename; application identity and installed bundle are unchanged. Updated `SHA256SUMS` and the final verification record. SHA-256 remains `a96e66763d37859b58917fb88c8a1653108bcded033cab14ea3fab4ad366aac3`. Signature, stapled ticket validation, and Gatekeeper assessment passed under the new filename; `renamed-verification.log` retains the commands and results. No rebuild or functional regression run was needed for this rename.

## Published GitHub Release

Published [aster v0.1.0](https://github.com/aster-ai-lab/aster/releases/tag/v0.1.0) to `aster-ai-lab/aster` at `2026-10-19T03:12:31Z` after uploading and verifying a draft.

- Release id: `416860855`. Tag target: `3903a565a2cd6442f3e5816bdf8b2d1f8a68e949`, matching the host build identifier. Release notes disclose the local working-tree changes included in this binary; no local source changes were committed or pushed by this publication task.
- Uploaded only `aster_0.1.0_aarch64.dmg` (68,946,966 bytes) and `aster_0.1.0_aarch64.dmg.sha256` (90 bytes). The checksum file refers only to the delivered notarized installer.
- Both GitHub asset SHA-256 digests and sizes matched their local files before publishing.
- After publication, both unauthenticated public download URLs returned HTTP 200 with the expected content lengths. This was an HTTP HEAD check, not a second full download.
- Reused the already notarized, installed-startup-verified artifact. No rebuild or new functional regression run was performed for the upload.
- GitHub release metadata, public download checks, and release notes are retained beside the installer.

## Explicitly not claimed

- The original `_signed.dmg` remains unnotarized and is retained as the initial build record. Only `aster_0.1.0_aarch64.dmg` is the final delivery.
- Full workspace CI, a complete regression run, installation over an existing app, Intel compatibility and testing on a second Mac were not performed.
- No product version bump and no user session state were packaged.
