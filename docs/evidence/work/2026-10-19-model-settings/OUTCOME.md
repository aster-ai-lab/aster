# Model settings persistence and deletion

Status: implementation and focused verification pass; real native-backed browser acceptance passed.

The native model store now retains every field edited by the model drawer,
including input/output modalities, token limits, false streaming flags and
pricing. The bridge forwards these fields and reconstructs saved metadata.
Absent token fields preserve their values; null explicitly clears a limit.
Native closed sets reject unknown capabilities, endpoints and modalities.

DELETE removes a model durably and is idempotent. All three default-model
slots prevent deletion with HTTP 409 and `model_in_use`. Bulk removal and
provider reconciliation use the same deletion path. Historical disabled rows
remain visible and disabled until the user deletes them again. Failed writes
restore the in-memory model rather than reporting an unpersisted change.

Evidence:

- Red: one native restart test lost capabilities; two bridge tests reproduced
  dropped metadata and soft deletion.
- Green: 29 native provider tests; 9 bridge tests. The shared renderer run,
  including the stop notice, passes 101 checks.
- Browser session `bugfix-models-20261019` uses production renderer assets,
  real native stores and a loopback model supplier. Name, group, tools,
  visual input, three token limits, streaming false and two prices were edited
  through the drawer and retained exactly across a full fixture restart.
- Logs: `target/bugfix-models-{rust,renderer}-red.log`,
  `target/bugfix-models-rust.log`, `target/bugfix-stop-model-renderer.log`.

Browser deletion passed provider reselection and a full fixture restart without resurrection. A real HTTP red/green check additionally caught and fixed deletion guards for the device Preference default slots when legacy provider defaults are empty. All three device slots now return 409; read failures refuse deletion. Retained report and final 29-test rerun: `../2026-10-20-native-bugfix-roundtrip/`.

## Explicitly not claimed

Installation, commits and submodule landing are not claimed. The final package is being refreshed after native JSON acceptance exposed a CSP layout defect.
