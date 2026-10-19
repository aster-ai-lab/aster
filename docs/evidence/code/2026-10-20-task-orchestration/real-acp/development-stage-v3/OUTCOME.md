# Signed development stage

The authorized side-by-side app is prepared at:

`/Users/roylin/code/aster/target/development-stage/task-orchestration-20261020/AsterDream-TaskOrchestration-Dev.app`

`stage.json` is the completed preparation record. The earlier `development-stage-preparation.json` remains a historical metadata-only snapshot. No application or utility VM was launched or stopped by this preparation. No Cargo command, production source edit, credential copy, or user-state migration occurred.

Three ordinary executable files were copied into `Contents/MacOS`: Work as `AsterDream`, ACP as `a3s-code-acp`, and Office as `a3s-office`. Copy hashes match the final build identities before signing. The plist retains its independent bundle identifier and existing per-app environment, and adds exactly the missing public fields from the current `macos_app::info_plist` template: dictionary version 6.0, the two existing ATS flags, and `NSPrincipalClass=NSApplication`. Host capabilities remain adaptive/retrieval-disabled for this experiment; product defaults were not changed.

## ACP source drift handled explicitly

Attempt 1 stopped before copying ACP because `target/debug/a3s-code-acp` had changed from the five-case-qualified `b5a88347…` to `21ddc2c1…`. Only the hash-matching Work file had been copied; no plist completion or signature command ran. `stage-attempt-01.json` and `prepare-stage-attempt-01.py` preserve that failed attempt. Its own Work copy was removed only after verifying the exact expected hash.

The preserved ordinary artifact `/Users/roylin/code/aster/target/debug/deps/a3s_code_acp-0f895f913ee015e2` has the exact final-build/five-case SHA-256 `b5a88347d4e6770d8ef2dc8f348ff6903674aeb30bc7726a38616d99670d72e0`. The completed stage copies this artifact. It does not use or rewrite the drifted top-level debug executable. No rebuild or model rerun was performed to recover the identity.

## Signing and verification

The reproducible operation source is `prepare_stage.py`; its SHA, source/copy/final hashes, exact command arrays, exit codes, timings, stdout/stderr references, and signature identities are retained in `stage.json`.

The execution command was:

```text
python3 docs/evidence/code/2026-10-20-task-orchestration/real-acp/development-stage-v3/prepare_stage.py
```

The script validates the plist with `plutil -lint`, signs ACP and Office first with their explicit adhoc identifiers, then signs the app. `codesign --verify --deep --strict --verbose=2` returned **0**. No bundle symlinks exist. Signing changes executable bytes and file sizes, so the signed identities below are distinct from the qualified pre-signing copy hashes.

| Executable | Signed SHA-256 | CDHash |
| --- | --- | --- |
| Work | `52d9e2ff4076fb48877c350062edb78291d2c76e4eff82746493b3a10ecd49f4` | `94f16f2f228cfa19bdccfec1870825ee1d7b787b` |
| ACP | `fe16431112e890f17058b14a5e1b9fe3c933f714638f49d8f681302d5a60bb7c` | `8bded62ba4054354f98070a06946b12e068bb397` |
| Office | `7fcd1c0be82453b05ac2f9fbf80d69164b18ea7605af36a40c58d81e38ba35af` | `07a9c75eb9b7a6e13f91d4aa19d17f6f0199c461` |

The source executables, existing old-app executable/plist, icon, and mode-0600 host ACL retain their before/after identities. The external frozen OCI directory is referenced through the existing `ASTER_SANDBOX_DIR` plist value only; it was neither changed nor signed.

This is an adhoc-signed development stage, not a notarized distribution. Root owns the subsequent CUA switch, actual process and inherited-environment observations, renderer identity, real child ACP identity, and app-level rollback check. Different bundle identifiers still share the primary `aster` instance lock; the planned normal shutdown of the verified old primary process must precede the new main-instance launch. Preparation does not claim those live gates passed.
