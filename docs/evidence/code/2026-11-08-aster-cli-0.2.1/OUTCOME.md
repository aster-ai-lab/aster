# aster-cli v0.2.1 publish

## Result

Published `aster-cli-v0.2.1` and updated `aster-cli-stable` / Aster Latest compatibility `latest.json`.

## Source

- Tag commit: `9891424327465d403477c44d05edf295dcb32c1c`
- Qualify: https://github.com/aster-ai-lab/aster/actions/runs/42438749634 (success)
- Release build run: https://github.com/aster-ai-lab/aster/actions/runs/42438754068 (linux + windows success; macos-bins cancelled at 120m)
- Channel: https://github.com/aster-ai-lab/aster/actions/runs/42448209159 (success)

## Archives

| Asset | SHA-256 |
| --- | --- |
| aster-cli-v0.2.1-macos.tar.gz | 80dcbee6238cce40a68bf9822af75ff65be705c252060fb0f60c0e2c85038199 |
| aster-cli-v0.2.1-linux.tar.gz | 994770ddceff90e7bb22ab21175037510d75f8a621b7e096a9b12812ccec91a6 |
| aster-cli-v0.2.1-windows.zip | ce82d3eff92cbb2863dd93cd7eee4ccecd80577317a2cb2a97f2127c9cff577e |

- macOS archive size: 107 MiB (local `release-dist` package on aarch64-apple-darwin; pair build-id `aster-cli-pair-v1:afb52cbd…`)
- Linux / Windows archives: CI `release-dist` artifacts from the tag run above
- Members verified: `aster`, `aster-acp`, `BUILD_ID`, `SHA256SUMS`, `embedding/potion-code-16M-v2.{safetensors,tokenizer.json,config.json}`

## Verification run

```text
Qualify aster-cli @ 989142432: success
Isolated macOS extract: aster --version → 0.2.1 (989142432746); aster/aster-acp --build-id match; embedding sidecar present; --help OK
publish-aster-cli.sh: draft created then undrafted
release-aster-cli-channel.yml version=0.2.1: success
```

Follow-up on main (not in the tag): `3cd69c2c8` raises macos-bins timeout to 180m so the next tag does not cancel mid-LTO.

## Explicitly not claimed

- GHA-built macOS archive for this tag (macos-bins hit the prior 120m timeout; macOS package was produced on the publish Mac from the same SHA)
- Desktop / Studio release
- Re-run of the retired `tests/cli_sandbox_smoke.py` companion path (ksandbox is in-Core; archive has no sandbox companions)
