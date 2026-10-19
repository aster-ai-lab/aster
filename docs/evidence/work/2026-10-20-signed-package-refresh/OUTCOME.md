# Fresh signed AsterDream 0.1.1 package

Result: accepted on 2026-10-20. Freshly rebuilt the current working tree for
macOS ARM64 using Rust 1.97.1 and the existing package-host.sh path. No product
code or dependency was changed for this packaging request.

Artifact: `target/packages/asterdream-signed-20261020-0413/aster_0.1.1_aarch64.dmg`.
Final post-staple SHA-256: `28ad33e57c424f824b206d65920a04ff6cb2d69c2fdd6c15baab10a353b5793e`.

## Executed checks

- Fresh Office and renderer build, release ACP/Office build, then Tauri
  custom-protocol app and DMG bundle: exit 0, [build log](build.log).
  STUDIO_RENDERER_DIST was ../../target/asterdream-signed-20261020-0413-renderer.
- The fresh emitted renderer contains Monaco's native document nonce binding:
  [compiled asset proof](renderer-nonce-proof.json).
- Developer ID Application: Lijin Zhu (5L9P7TS6DM), hardened runtime, bundle
  dev.a3s.studio, version 0.1.1 and executable architecture arm64 verified.
  App deep/strict and DMG strict signatures pass before and after stapling.
- Apple notarization `c0711a01-9bb6-4a09-9e5b-74910e716300` is Accepted with no
  issues. Apple's submitted SHA-256 matches the exact pre-staple DMG:
  [submission](notary-submit.json), [Apple log](notary-log.json).
- App and DMG tickets stapled and validated. Gatekeeper accepts both as
  Notarized Developer ID. DMG integrity verification passes after stapling:
  [app Gatekeeper](gatekeeper-app.log), [DMG Gatekeeper](gatekeeper-dmg.log),
  [staple validation](stapler-dmg-validate.log), [integrity](dmg-verify-after-staple.log).
- Read-only DMG mount: app, ACP and Office executable hashes match the built
  signed application. Mounted app deep/strict signature and Gatekeeper pass:
  [binary proof](mounted-binary-proof.json), [Gatekeeper](mounted-gatekeeper.log).
- DMG detached successfully and hdiutil's mount inventory confirms no owned
  mount remains. [Unmount log](unmount.log) includes an internal settle timeout
  followed by successful ejection; command exit is 0. Sampled packaging source
  hashes remain unchanged: [source snapshot](source-snapshot.json).

The [manifest](manifest.json) and [SHA256SUMS](SHA256SUMS) bind the final file.
The earlier [eight-issue acceptance](../2026-10-20-native-bugfix-roundtrip/COMPLETION_AUDIT.md)
remains retained for the repair scope.

## Record checks

Final artifact hash, retained manifest and local evidence links pass; ROADMAP
whitespace diff check passes. `pnpm docs:check` still reports the same 182
pre-existing broken-link targets as the preceding run. ROADMAP line numbers
shifted by the added gate row; no new broken link was introduced. See
[docs gate output](docs-check.log).

## Explicitly not claimed

No fresh GUI campaign, full repository suite, user installation, publication or
commit was performed in this packaging-only task. Earlier unrelated full-tree
lint/typecheck/docs failures are not certified as fixed by packaging.
