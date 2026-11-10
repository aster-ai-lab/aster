# Restore frozen CLI environment references

The Aster all-target check could not compile `env_manager`. Twelve `include_str!`
paths referred to reference JSON deleted by `e4914338e` during the repository
migration. The test and its assertions remained in the workspace.

The twelve required files are restored byte for byte from the deletion's parent,
`bcde2cc89274fff2f853093e19a4a44a7b51e638`. The [fixture manifest](fixtures.json)
records every path, original Git blob and SHA256. Each file parses as JSON and
its staged blob matches the historical blob.

The same all-target Clippy invocation passed after restoration. The existing
`prune_matches_twelve_frozen_native_cases_including_a_running_executable` test
passed all twelve cases in one Rust test. The original compile failure and the
successful commands are retained in [checks.json](checks.json).

## Explicitly not claimed

This verifies fixture restoration, test compilation and the prune contract on
macOS arm64. Runtime validation of the other environment-manager reference cases
and platforms remains pending.
