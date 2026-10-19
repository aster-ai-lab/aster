---
description: >-
  Environment CLI installs through a3s-use tool-acquire, with snapshots and IPC
sources:
  - src/shared/data/presets/binaryTools.ts
  - src-tauri/src/binary_tools.rs
---

# Binary tools

The shell installs environment CLIs with `a3s-use` tool-acquire. The renderer sends `binary.*` IPC. The shell command is `binary_ipc`.

Accepted specs are a bare catalog name, `github:`, `npm:`, `pipx:`, and `aqua:`. `pipx:` installs with `uv`. Python stays on that `uv` install. `npm:` downloads a registry tarball, checks `sha512`, and does not run lifecycle scripts.

A checksum mismatch writes a failure receipt and leaves `current/` unchanged. The tool root is `~/.a3s/tools`. Layout: `installs/<name>/<version>`, `current/<name>`, `shims/<executable>`, `receipts/<name>.json`.

Fixed names come from the tool-acquire catalog. Custom tools are stored in `custom-tools.json` under that root. `availability.source` for a published shim is `use`. `bundled` and `system` stay external.

`get_tool_snapshots` is the availability read. `get_latest_versions(false)` reads the session cache. `get_latest_versions(true)` asks the source. A latest-version failure leaves the install unchanged.
