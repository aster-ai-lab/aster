---
description: AsterDream native Linux generation and input-method packaging
sources:
  - scripts/kylin-flatpak
---

# Linux Packaging

Use the native packaging pipeline under [Kylin packaging](../../scripts/kylin-flatpak/README.md). It builds the shared Rust host and workspace ACP/Office companions, binds the selected tenant identity and stages the matching renderer and icons.

The `aster` and `toyou` builds use separate declared product identities. Packaging refuses an unknown tenant, a renderer stamped for another product, missing companions, changed generation files or missing input-method modules.

The launcher preserves the qualified Fcitx/IBus chain and display backend. It does not silently bypass a required sandbox. A successful protocol fixture or container build does not prove a physical Kylin desktop result.
