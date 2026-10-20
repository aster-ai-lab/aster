# Studio Linux/Kylin input-method packaging

2026-10-21. T007 / FR009 mechanisms are implemented separately from Work. Actual paired Studio Linux compilation/export and a physical Kylin Studio GUI remain pending; the proofs below do not substitute synthetic binaries for the real application.

## Product changes

[Studio's new packaging directory](../../../../apps/studio/scripts/kylin-flatpak/README.md) owns build, generation staging/verification and strict Flatpak export. It does not copy Work's installer, feed, host, ACP, Office or user state. Studio retains `dev.a3s.studio`, `Studio`, its own tenant and `/app/Resources` layout. Its existing Linux companion lookup already recognizes `/app/bin/agent/a3s-code-acp` and `/app/bin/a3s-office`, so no Linux Rust lookup patch was needed.

The three binaries and renderer are sealed in one build receipt. Staging rejects a changed host/companion, mismatched renderer, incomplete/modified input module or wrong source archive. The launch command verifies complete membership, content hashes, required executable permissions, tenant marker and `SHA256SUMS` before executing a target. Only explicit brand assets from the selected tenant are copied; a registry JSON does not enter the package.

Input-module/backend/cache and native probe mechanisms were adapted from qualified Work R4. The original source identities are retained in `work-provenance.json`. Native reuse requires the five **exact R4 binary hashes**, which refuses the older snooper-disabled fcitx4 module. Fcitx4 retains snooper ON, the typed modern→legacy portal retry and exact GPL archive/patch/build recipe; fcitx5 includes its exact source archive and LGPL license. Each module has its own actual registered module-id/cache path. The launcher pins Fcitx to the filtered session bus, queries the real GDK backend, and avoids selecting another module when the selected framework's cache is absent.

Packaging requires real Flatpak runtime checks for all binary/module links, `RTLD_NOW`, a real IBus module registration, and the private-bus native GTK gate before export. The production package script has no seccomp fallback and does not install an app into the user's profile.

## Executed proofs

| Check | Result and scope |
| --- | --- |
| [Maintained hermetic contracts](ime-kylin-hermetic-final.log) | 10 tests pass: complete Studio layout, independent cache rewrite, each mixed binary, renderer mutation, missing module dependency, wrong ELF/source, wrong registered ID, tampered/missing/extra/symlinked/nonexecutable generation and daemon selection. Synthetic ELF/archive inputs are explicit test data, not upstream qualification. |
| [Exact R4 import](ime-kylin-source-receipt.json) | Real module/helper hashes and the pinned 4.2.9.9 / 5.1.7 corresponding-source SHA-256 values verified. The downloaded fcitx5 archive is not used until its pinned digest matches. |
| [Native GTK/portal](ime-kylin-native-gtk.log) | Eight real fcitx client/private-bus protocol cases pass, including refusal without legacy retry. Actual staged Studio launcher verifies the generation and runs the GTK probe. Chinese commit, mixed English, Escape cancellation and a/b/c exactly once pass on host `:100` and Flatpak-style mapped `:99.0`. |
| [Runtime module and IBus checks](ime-kylin-native-runtime.log) | Both real staged GTK modules load `RTLD_NOW` in GNOME50; `gtk-query-immodules-3.0` returns the actual IBus registration. This is a test-only direct runtime mount. |
| [Actual launcher refusals](ime-kylin-launch-refusals.json) | Missing Office and changed ACP each exit 1 with the exact generation mismatch reason, before the controlled target is executed. Test-only direct runtime mounts were used. |
| [Stock Flatpak](ime-kylin-stock-flatpak.log) | Refuses this machine's Rosetta kernel with `PR_SET_SECCOMP` EINVAL. No production sandbox success is claimed. |
| [Production strict package refusal](ime-kylin-strict-package.log) | The real package script reaches the runtime gate, fails on that same kernel limitation and produces no `.flatpak`. It does not downgrade the gate or export anyway. |

The native fixture's three application/companion slots are deliberately `/usr/bin/true`, with revision `synthetic-native-gtk-fixture`. They establish packaging, launcher verification and module/protocol behavior only. No real Studio Linux app or local model request was run in that fixture.

The native checks used only container `aster-studio-kylin-ime` (exact ID retained in [source receipt](ime-kylin-source-receipt.json)), its isolated root Flatpak profile, owned temporary Xvfb/D-Bus processes and repository fixture paths. The host's user application profiles were not installed or changed. Container cleanup is pending the root's later isolated Linux-build coordination.

## Remaining gates

- Real Studio/ACP/Office Linux build, with an isolated source/resource tree so Linux build.rs cannot overwrite a simultaneous macOS resource generation.
- Final strict Flatpak export/install on a kernel that actually supports its seccomp gate.
- Real Studio WebKit candidate Enter/Space, deliberate send, rapid reply and plan/approval interactions in the packaged app. Native GTK success does not prove those renderer/native integrations.
- Physical Kylin desktop and native Wayland input. Neither is claimed by Xvfb or Rosetta evidence.

No Linux automatic updater, user installer or workaround for missing kernel sandbox support was added. Full cross-application completion remains the parent integration goal.
