# Scrawl source provenance

- Upstream: `https://github.com/kitlangton/scrawl`
- Revision: `7b184c67767a1aee6c1d44640d94d8446cd5fed1` (`0.3.13`)
- License: MIT, retained at `crates/capture/LICENSE`
- Source archive SHA-256: `99f3f9e962eebeef1711047a590c512e18d3dbfc25f3fe00dfab213c8f73daa2`
- Vendored path: `crates/capture`
- Specification: `specs/027-scrawl-evidence/`

The source is internalized in the existing repository and root Cargo workspace. The second upstream Cargo lockfile was not copied. The independent Scrawl.app installer, updater and recipient model are not used as Aster integration points.

## Test-only dependency

The host has Command Line Tools rather than full Xcode, so the toolchain's Swift Testing module is unavailable. `Package.swift` uses the official `swift-testing` package to run the unchanged upstream tests:

- Repository: `https://github.com/swiftlang/swift-testing`
- Tag: `swift-6.3.3-RELEASE`
- Revision: `48d727cc1cf4eda667c858c501495f1018f69d21`
- License: Apache-2.0 with Runtime Library Exception

This dependency belongs to the CaptureKit test target. It is not an A3S product SDK or a second product configuration system.

## Integration adjustments

- Root workspace membership and `publish = false` were added to the vendored crate.
- Existing `a3s-code-voice` now shares `cpal 0.18.2`, avoiding two ALSA link versions in one dependency graph. Its device-description, sample-rate and stream-config calls were updated to the actual crate API.
- The evidence library and the optional GPUI executable use separate module roots. Studio links the evidence library without GPUI; the vendored app keeps its own UI modules.
- The upstream `site` sources and feature-map fixtures are retained so the upstream feature-inventory test can run in this workspace.
- Aster-owned integration is specified separately. Workspace membership alone is not proof of native recording, signing or platform support.
