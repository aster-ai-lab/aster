# Releasing Capture

Everything lands on `main` through pull requests, the version bump and the appcast included. A release
is one Mac build (notarized DMG + Sparkle zip) and one Windows zip, built from the same tagged commit.

1. **Version:** bump `version` in `Cargo.toml` and `Cargo.lock` in a `release/<version>` PR, and in the
   same PR change every `"status": "main"` in `site/src/features.json` to `"stable@<version>"`. Pushing a
   `release/**` branch dry-runs the Windows job (`.github/workflows/release-windows.yml`), which also opens
   the welcome window on a fresh profile and keeps a screenshot.
2. **Mac build** (Studio, from that commit): `CAPTURE_BUILD_NUMBER=<n> CAPTURE_SPARKLE_DIR=~/.local/share/capture-release/sparkle-2.9.4 ./scripts/release-app.sh`.
   The build number goes up by one every release (Sparkle compares it, not the version). The script
   builds, signs, notarizes and staples `dist/Capture-<version>-arm64.dmg` and `.zip`. Its own
   `dist/updates/appcast.xml` is throwaway: the published appcast comes from step 4.
3. **Tag and release:** merge the PR, tag the merge commit `v<version>`, and create the GitHub release
   with the DMG. The tag's Windows job attaches `Capture-<version>-windows-x64.zip`.
4. **Appcast** (the Pro, over ssh): copy the Sparkle zip into `~/.local/share/capture-release/updates/`
   and run `launchctl kickstart gui/$(id -u)/dev.kit.capture-appcast`. The job
   (`~/Library/LaunchAgents/dev.kit.capture-appcast.plist` → `~/.local/share/capture-release/appcast.sh`)
   reads the ed25519 key from the login keychain, which only a GUI-session job can, and runs
   `generate_appcast` over that folder, which holds every earlier zip so it can make deltas. Its log is
   `~/.local/share/capture-release/appcast.log` and ends with `exit=0`. Copy `updates/appcast.xml` into
   `site/public/appcast.xml` in a PR and check the new item's `edSignature` against the app's
   `SUPublicEDKey`. For a dry run, run `appcast.sh <folder> <log>` as a one-off launchd job on a copy
   of the folder.
5. **Site** (the Pro's checkout, where `wrangler` is signed in): fast-forward to `main`, copy the DMG,
   Sparkle zip, Windows zip and the new `Capture<n>-*.delta` files into `site/public/releases/` (also as
   `Capture-latest-*`), then `bun run build && bunx wrangler@latest deploy`.
6. **Homebrew:** a PR to `kitlangton/homebrew-tap` bumping `version` and `sha256` in `Casks/scrawl.rb`.

Keep the Sparkle tools and the updates folder under `~/.local/share/capture-release/`, never in `target/`
or `/tmp`: a clean or a reboot would take the tools and every earlier zip with them.
`scripts/setup-sparkle.sh` fetches Sparkle (checksummed) into `$CAPTURE_SPARKLE_DIR`, or `target/` when
it's unset.

Check after deploying: the live DMG, zips and newest delta match the release's sha256s, the live
appcast's signature verifies, `spctl` accepts the downloaded DMG and the app in it, the site shows the
new version for both the Mac and Windows download buttons, and the Windows zip has `capture.exe`,
`transcribe.dll`, `ggml.dll`, `ggml-base.dll`, nine `ggml-cpu-<tier>.dll` (x64, sse42, sandybridge,
haswell, alderlake, skylakex, cannonlake, cascadelake, icelake; see [windows.md](windows.md)), the three
Visual C++ runtime DLLs (`VCRUNTIME140.dll`, `VCRUNTIME140_1.dll`, `MSVCP140.dll`), README and LICENSE.
