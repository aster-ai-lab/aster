# Default AsterDream delivery and entry points

The shared `Desktop` executable now builds and packages two explicit products:
`aster` is AsterDream / `dev.a3s.desktop.aster`; `legacy` is Former / `dev.a3s.desktop`.
Both use version 2.1.3. Legacy AsterDream 0.1.x is not imported or silently replaced.

## Implementation

- `just dev`, `just dev` and `just work` select the new default. Explicit
  `just dev` remains available. The old process-killing Work launcher was
  removed; `build-work` and `build-work-release` remain marked legacy source builds.
- `desktop_product.py` declares the two exact names, IDs and Cargo features. Every host
  build disables default features and selects one tenant feature. The macOS Tauri CLI
  receives Cargo's `--no-default-features` after its `--` runner separator.
- The Tauri default is AsterDream. Selected builds override names, IDs and icons together.
  The main binary remains `Desktop`; ACP and Office remain root-workspace companions.
- The frontend brand loader defaults to `aster`, rejects explicit empty/invalid/missing
  brands or required files, and keeps legal brand-only fixtures readable. Actual host
  activation is separately bound to the compiled native product.
- The macOS sealer requires exact Info.plist identity and the main HTML's exact tenant
  and product attributes. INFO.md plus the three native-required light logo, wordmark
  and icon must exist and be nonempty regular files. Resources are hashed before final
  signing; the final signature covers the host and manifest without a circular self-hash.
  The shared package helper also checks the native INFO front matter, name, exact
  product, declared light logo and optional light theme. Tauri configuration, macOS
  sealing and all three Linux generation boundaries use this check. INFO reads stop
  at 64 KiB; a separately declared valid logo is hashed and shipped with the generation.
- Linux staging uses the same identity map for its Flatpak app ID, desktop name/icon and
  output file. Its build receipt binds three binaries, renderer, seven selected brand
  files and the desktop icon; changes after build refuse staging. The packaged validator
  and identity helper are included in the complete generation hashes. Fcitx/IBus and the
  strict sandbox gates are preserved.
- Development runtime discovery uses the selected tenant/instance directory. Explicit
  runtime overrides remain supported. Maintained E2E discovery callers were adapted to
  the same paths and verified with synthetic temporary descriptors.

## Verification and evidence

Current focused checks contain 43 Python tests (5 root entry/build, 18 macOS sealer and
identity/package entry, 18 Linux generation, 2 runtime discovery) and 17 script tests in
2 files. These counts are distinct within this delivery subtask; they must not be added
to overlapping historical frontend/UX totals. See the latest source receipt and logs:

- [Source receipt](delivery-source-ready.json).
- [Entry/build checks](delivery-feature-entry-after.log).
- [macOS sealer/identity/package fixtures](delivery-feature-package-after.log).
- [Linux generation checks](delivery-feature-linux-after.log).
- Final INFO-preflight closure: [macOS/helper 18](delivery-info-macos-explicit-interpreters-final.log),
  [Linux 18](delivery-info-linux-explicit-interpreters-final.log), and
  [before/after report](DELIVERY-INFO-PREFLIGHT.md).
- [Brand/development scripts](delivery-feature-brand-dev-after.log).
- [Runtime discovery](delivery-runtime-discovery-after.log).
- [Format](delivery-feature-format.log) and [ESLint](delivery-feature-eslint.log).
- [Installed Tauri parser](delivery-tauri-installed-cli.json): the invalid top-level Cargo
  flag is rejected; the explicit runner separator parses successfully. This is help/parser
  evidence, not an application build.

Before evidence is retained: the old default/Work entry failed 3 subcases; the sealer failed
3 product-boundary cases and 1 renderer case; the brand loader failed 7 cases; Linux failed
3 identity cases; missing native brand assets failed 3 subcases; the old feature entry failed
2 selections and the package entry lacked its Cargo isolation flags. The brand-generation
trial also contained an unrelated 3-second daemon fixture timeout and a cleanup-masked second
subcase; only the actual INFO mutation acceptance is a valid finding from that run. Both
relevant final cases were rerun alone, then the complete owning suite passed.

One temporary package-fixture run stalled resolving the generic `/usr/bin/env python3`
interpreter and timed out. Its log is retained. Fixture commands now use the current
interpreter's exact path, run in an owned process session and clean up that session on
failure. The two-tenant package fixture then passed alone and in the owning suite.

## Explicitly not claimed

The package-entry fixtures use synthetic host bytes and stub external compiler/signing
commands. They do not certify a final macOS signature, live upgrade, or application health.
The native/root agents own actual two-feature Cargo builds, cfg policy isolation, final
renderer/host packages and two-tenant GUI verification; those must be recorded separately.

Actual isolated x86-64 Linux ACP and Office release builds completed, with ELF/ldd and exact
ACP build-source identity checks. This only qualifies the companions; see the retained
[companion receipt](../2026-10-21-work-alignment/ime-linux-companions-artifact.json).
The final selected-tenant Linux host/package is still pending. Physical Kylin, native Wayland
and stock seccomp support on this container kernel remain unproven. No user profile, live
release feed, installer, commit, push or publication was performed here.
