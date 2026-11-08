# aster-cli v0.3.1 + Core/ACP 9.2.1

## Result

- Published `aster-acp-v9.2.1` (Core/ACP 9.2.1).
- Published `aster-cli-v0.3.1` and updated `aster-cli-stable` / Latest compatibility `latest.json`.

## Sources

| Product | Tag | Commit |
| --- | --- | --- |
| Core/ACP | `aster-acp-v9.2.1` | `055d543db19ad4dc99cef6df7955c439f35b025a` |
| CLI | `aster-cli-v0.3.1` | `25b6d339ce79ed7463e022a9baaedecf4ce2db84` |

## Verification

- Qualify aster (`qualify-code.yml`) green on Core/ACP tag SHA (`42501691478`).
- `release-code.yml` published linux/macos/windows ACP archives (`42504090048`).
- Qualify aster-cli green on CLI tag SHA (`42507236853`).
- `release-aster-cli.yml` built platform archives (`42508493549`); publish job skipped without `qualified_runner`, so archives were assembled with `scripts/publish-aster-cli.sh` from that run.
- Isolated macOS extract: `aster --version` → `0.3.1 (25b6d339ce79)`; ACP `9.2.1`; matching pair build-id; embedding sidecar present.
- Channel workflow `release-aster-cli-channel.yml` version=`0.3.1` (`42520555301`).
- Hermetic A1/B1 (`tui_913_hermetic`) green on the 0.3.1 candidate tree.

## Explicitly not claimed

- GHA self-hosted `qualified_runner` publish path (no runners registered; local draft assembly used).
- Full Layer C visual re-matrix on the published archives (prior DeepSeek visual evidence remains from the 9.2.1 candidate worktree).
