# aster-cli v0.3.0 + Core/ACP 9.2.0

## Result

- Published `aster-acp-v9.2.0` (Core/ACP 9.2.0).
- Published `aster-cli-v0.3.0` and updated `aster-cli-stable` / Latest compatibility `latest.json`.

## Sources

| Product | Tag | Commit |
| --- | --- | --- |
| Core/ACP | `aster-acp-v9.2.0` | `dbb9f740c9d6e14d6added011abff22dd33ed9d9` |
| CLI | `aster-cli-v0.3.0` | `656cc9811b047e35b8278673b9e772b5862ae56b` |

## Verification

- Qualify aster (`qualify-code.yml`) green on Core/ACP tag SHA.
- Qualify aster-cli green on CLI tag SHA.
- Isolated macOS extract: `aster --version` → `0.3.0 (656cc9811b04)`; matching pair build-id; embedding sidecar present.
- Channel workflow `release-aster-cli-channel.yml` version=`0.3.0`.

## Explicitly not claimed

- GHA-built macOS archive for this CLI tag (local `release-dist` package on aarch64-apple-darwin; linux/windows from run `42476133664`).
