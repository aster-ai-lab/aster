# T023 single-Run runner qualification

The test-only runner is implemented in
`crates/code/core/tests/extensibility/task_matrix.rs`, registered by two lines
in the existing real-provider integration target. Existing changes in that
target belong to earlier tasks and are preserved. No product loader, new
framework, default, credential file, cache, model, or VM was changed or run for
this runner qualification.

`hermetic.json` and `hermetic.log` record the exact focused command and final
result. The live single-Run test remains ignored. An initial command omitted
the existing target's required `advanced-harness` feature and Cargo refused it
before tests; subsequent commands include that feature. The resulting source
and executable are not yet a frozen performance sample.

## Manifest and invocation

The manifest is strict JSON with schema `aster.task-matrix-run.v1`; unknown and
duplicate struct fields, duplicate frozen path keys, non-normal relative paths,
ambiguous models and undeclared routes are refused. Required fields:

| Field | Contract |
| --- | --- |
| `run_id` | A new session ID, 1–64 ASCII alphanumeric, underscore or hyphen characters |
| `trusted_acl` | Explicit absolute existing regular ACL file; read in place, never copied into reports |
| `route`, `endpoint` | Exactly `deepseek/deepseek-flash` and `https://api.deepseek.com`; provider and effective model endpoint are checked, no fallback |
| `profile` | Exactly `adaptive` or `code` |
| `workspace` | Existing canonical directory, initially containing exactly `inputs` |
| `inputs` | Relative file path to initial SHA-256, including all initial files |
| `prompt` | Nonempty literal user prompt, at most 256 KiB; its hash is reported |
| `source` | `{root,files,symlinks}`; source paths are explicitly declared and verified before and after the Run; source symlinks are refused |
| `assets` | Same shape; all regular files and symlinks must match the entire bounded asset tree |
| `runner_sha256` | SHA-256 of the actual copied test executable; checked against `current_exe()` before and after the Run |
| `build_features` | Explicit frozen build feature metadata; the executable hash identifies the actual program |
| `permissions` | Explicit allow rules over a default-deny policy; ordinary security remains enabled |
| `budget` | All fields below are required; no manifest default budget |
| `submissions` | Unique explicit relative artifact paths; these exact paths may change even if present in `inputs` |
| `evidence_dir` | New absolute directory with an existing canonical parent, outside workspace and runtime assets |

Budget fields are `max_execution_ms`, `llm_api_timeout_ms`, `tool_timeout_ms`,
`max_tool_rounds`, `max_continuation_turns`, `max_parallel_tasks`,
`thinking_budget`, `reasoning_effort`, and `reported_token_limit`.
The token guard is the public reported-token soft cap; unreported provider
usage is not estimated or presented as complete. Delegation, continuation and
planning are disabled. Retrieval and extra MCP/instruction configuration are
fixed to defaults; automatic personal/project instructions are disabled with
`project_doc_max_bytes = 0`. Only the explicitly selected provider is retained.

All hashes are bare lowercase 64-character SHA-256. Asset symlink hashes cover
the raw UTF-8 `read_link` target bytes; guest absolute or dangling links are
never followed on the host. Source verification covers the manifest's declared
files, including the required Cargo/lock/toolchain and integration-test files;
it is not a claim to have snapshotted every repository file or to have proven
bit-for-bit rebuild reproducibility.

After source qualification and the final root build, copy the test executable,
freeze its hash and all manifest inputs, then use a new process for exactly one
cell. The live invocation is documented here, **not executed in this evidence**:

```sh
ASTER_T023_MANIFEST=/absolute/cell.json ASTER_SANDBOX_DIR=/absolute/signed-assets \
  /absolute/frozen-test-executable task_matrix::frozen_task_matrix_single_run \
  --exact --ignored --test-threads=1 --nocapture
```

The runner checks this exact single-test argument set. Rebuild/copy/refreeze if
the runner or required source changes. The current build command is recorded in
`hermetic.json`; adding `--no-run` builds without invoking any tests.

## Measurement and failure handling

Before model access, the real ignored fixture performs one fixed governed OCI
`printf t023-preflight`, verifies its exact output, exit code and timeout state,
and requires exactly one closed eight-phase `oci_command` span. It writes
`backend-preflight.json`, then clears that command from task observations. This
prevents an absent collector from being reported as zero VMs. Every task command
must have exactly one of each observed phase and a closed span; incomplete
observation yields a null VM count and refuses measurement qualification. VM
counts never come from Bash call counts.

`task_ms` covers Core stream admission, event drain and bounded worker/cancel
settlement. Backend preflight, preparation, explicit close and the independent
oracle are separate. The Run and outer watchdog use the frozen wall budget;
cancellation and resource settlement have additional bounded grace periods.
This is a Core Run clock, not a Work GUI submit-to-artifact measurement.
`preparation_ms` specifically measures Agent/session construction. Manifest
hashing, post-Run evidence snapshots and process startup are outside that field;
the matrix controller must retain the outer process wall clock separately.

Metrics recover parent IDs from typed original top-level `tool.result` receipts
bound to the exact admitted Run. Missing, mismatched or duplicate identities
remain errors. Qualification requires known origins, parent/child call counts
and output bytes, complete parent interval unions, each program's timed child
receipts and per-program interval union, and independently observed VM counts.
The global child interval union remains null for program-local clocks; it is
not computed by adding nested or overlapping times. Provider usage availability
is recorded without turning missing usage into zero.

`run.json` is saved before qualification assertions, including failed and
timed-out Runs. `runtime-observation.json` keeps partial runtime evidence before
agent close; evidence I/O failures cannot bypass the explicit close attempt.
The final report requires ordinary Run completion, settled stream/session/agent,
matching actual presentation snapshots, retained event completeness, frozen
source/assets/executable identities, immutable inputs and all declared
submissions. `oracle_passed` remains null until the separate oracle runs.

Original receipt logs stay in the new workspace. The report retains their path,
whole-log digest and validated receipt identities/outcomes, plus output/effect
digests and sizes; it does not copy tool output, model responses, reasoning,
trusted ACL contents or credentials. Submission bytes are snapshotted only from
the host-declared relative paths and within bounds. Initial hashes are checked
for every input; only explicitly declared submissions are exempt from post-Run
immutability checks, supporting both T01's new artifacts and held-out code repair.

The Core global skill registry watches the default personal skill directory
without loading it at startup. The runner requires empty skill registries before
and after the Run and records counts only. These checks cannot retrospectively
exclude an external skill-directory change that was added and removed during a
Run; the controlled matrix must avoid such concurrent configuration mutation.

## Limits

Hermetic checks validate parsing, original-receipt identity, bounded failure
recording, output privacy, mutable submission boundaries and trace completeness.
They do not qualify a live model, governed VM probe, task correctness, actual
speedup, Work delivery, LSP/ONNX readiness or cross-platform operation. The live
runner currently refuses platforms other than macOS/aarch64 and process-host
sandbox overrides. Final T023 sampling remains gated by the owning production
changes, source/runner/assets freeze and independent oracle qualification.
