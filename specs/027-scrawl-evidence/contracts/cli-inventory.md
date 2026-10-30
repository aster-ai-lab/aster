# Native command inventory

Derived from the pinned upstream `src/main.rs` and README. Every public operation needs a direct test or a documented native refusal. Hidden developer diagnostics remain outside the product UI and are preserved only where the vendored engine supports them.

## Public `scrawl` commands

- `app [--record|--iphone]`: native capture/review shell; Aster owns the shell integration instead of launching a second branded app.
- `demo --out … [--clipboard]`: generate and optionally paste a deterministic evidence bundle.
- `record [--window …|--iphone] [--duration …] [--note …] [--out …]`: bounded native recording.
- `import [recording] [--timeline …] [--note …] [--out …] [--clipboard] [--transcriber …]`.
- `devices`: list supported cabled devices.
- `retranscribe <bundle>`: refresh transcript and keyframe narration in place.
- `reprocess <bundle> --out … [--transcriber …] [--experiment …]`.
- `recover [bundle|--captures …]`: compile interrupted captures once each.
- `mic list|set|watch`: enumerate, select and observe native microphones.
- `hex status` and the pinned Hex subcommands: inspect connectivity/models and configured transcriber.
- `inspect <bundle> [--json] [--marks] [--experiment …]`.
- `frames <bundle> --from … --to … [--fps …]`.
- `zoom <bundle> [--at …|--mark …] [--frame …] [--span …] [--frames …] [--full]`.
- `export <bundle> [--out …] [--crop …] [--size …] [--zoom] [--no-ink] [--no-clicks] [--hevc] [--fps …]`.
- `clean [--dry-run]`: execute or preview eligible retention cleanup.
- `pin <bundle> [--off]`: preserve or release a bundle from cleanup.
- `autocrop <bundle> [--open] [--serve] [--port …]`: inspect or tune activity cropping.
- `send <bundle> [--session …] [--reroute] [--clipboard]`: deliver only to an authorized recorded/explicit destination.
- `route [--url …|--browser …] [--explain <bundle>]`: inspect routing without sending.
- `preview <bundle>`: open evidence in the integrated Aster review surface.
- `link --scheme … [--url …] [--session …] [--port …]`: create a device-pairing setup link.
- `serve [--port …] [--usb]`: run authenticated headless ingest; recipient transport is Aster-owned.
- `skill`: print the compatible evidence-reading skill.

## Native diagnostics

- `sck-probe [--watch …]`: check whether Screen Recording permission reaches the current process.
- `lab`, `tune`, `fixture`, `capture-sim`, `autocrop-eval`, and `windows`: preserve as developer-only diagnostics where applicable; never expose them as composer features.
- `scrawl-helper devices`, `permissions`, `transcribe`, `frames`, `shrink`, `export`, `permission-panel`, `onboarding`, and `settings`: test the corresponding native protocol directly. They do not become a second product configuration or a public SDK.

The implementation may use the native `scrawl` binary target for these operations. Aster UI and agent tools call the same library instead of spawning a shell for each operation.
