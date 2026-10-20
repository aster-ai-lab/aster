# Issue follow-up scope

Continue PR #8 with pinned private-source CI checkout, toolchain/front-end/service fixture repairs, restoration of the complete historical tenant data directory, bounded ACP startup, HTTP response-body cancellation, truthful prompt-override help and a source-matched CLI/ACP v0.1.2 release path.

Changes use the isolated worktree at 0fd4689df8f586f2092e5fb6e674957e2caf35c5. The original dirty working tree is not staged, reset or committed. No guard is removed. Product configuration stays ACL and public Rust signatures stay intact.

Reuse the existing session RPC timeout helper for TUI/headless startup. Preserve productive streaming lifetimes and the existing non-streaming generation/retry policy. Full prompt replacement stays explicitly unsupported: issue #3 accepted that contract, and removing harness/permission prompts would violate the host boundary. Append instructions through --rules.

GitHub refused a new Work deploy key (HTTP 422, deploy keys disabled), and organization-secret inventory is unavailable to the current connection (HTTP 403). No deploy key or secret was created. CI instead has a concrete pinned GitHub App action, using CI_SOURCE_APP_ID and CI_SOURCE_APP_PRIVATE_KEY with per-repository Contents:read tokens and no persisted checkout credentials. Configuration and remote success remain unclaimed until owner setup and an actual run.

Published releases are immutable: build all three archives in the same run at its source SHA, create a new aster-cli-v0.1.2 tag/release, and never clobber v0.1.1. Installer links are held in the follow-up branch until delivery is validated.
