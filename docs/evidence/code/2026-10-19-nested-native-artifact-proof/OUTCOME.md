# Nested native artifact verification

The failed A09 turn exposed a native proof handoff gap at the PTC boundary. The
completion gate correctly rejected the proof it actually received; the native
validation performed inside `program` had not reached that gate. This change
preserves the gate and carries live native checks across the governed invocation
boundary.

## Observed failure

Only session `01a151b6-9b7c-712c-9fad-b16a90e27a9e`, assistant anchor
`01a151b6-9c35-7277-a89c-09266011f65b`, and its runtime
`10ccc202-9f9f-4257-8b2a-061f6296c654` were inspected. The bounded, allowlisted
projection is [a09-scoped-native-proof-observation.json](a09-scoped-native-proof-observation.json).
It excludes tool arguments, prompts, file contents and provider configuration.

- Committed receipt 31: top-level `office_file` created `_font_probe.png`; native
  validation and a bound Passed report were present.
- Receipt 46: top-level `program` created `_probe2.png` through a successful
  nested `office_file`; the nested metadata recorded native validation and its
  saved-byte digest, but the outer receipt had no report.
- Receipt 49: top-level `program` created both formal TC08 PNGs through successful
  nested `office_file` calls. Both had saved-byte digests and `validated=true`;
  the outer receipt again had no report.

The old binder asked only the top-level tool for its native hook. `ProgramTool`
has no such hook. Consequently, treating the probe files as unverified merely
because of their names would misdiagnose the failure.

## Change

Five Core source files change; public tool APIs, wire schemas and Office creation
stay unchanged. Each prepared invocation receives a private in-memory
`MutationLedger`. Governed nested calls allocate their own child ledger, use the
existing bounded workspace observer and consult the registered tool's native
hook. Child checks transfer only when their path and full file identity match
the caller's actual final typed effects. The existing aggregate report still
requires every recorded mutation, complete observation and current saved bytes.

No Program JSON field supplies verification authority. Failed calls cannot
transfer positive proof; another writer invalidates it, including a rewrite of
the same bytes. Independent prepared calls never share or inherit evidence.
No waiver, fabricated receipt or terminal-state override is introduced.

## Validation

Commands run at the repository root with the pinned toolchain and no live model
credentials. Exact commands and measured counts are in
[test-observations.json](test-observations.json).

- **RED:** exact new real-Harness test: 0 passed, 1 failed, 0.51 s. All four PNGs
  existed and passed `NativeOfficeImage::inspect_bytes`; the aggregate gate
  still refused them. See [before-fix-exact.log](before-fix-exact.log).
- **GREEN:** `cargo test --locked --offline -p a3s-code-acp --lib
  office_file::tests::nested_verification -- --nocapture`: 3 passed, 0 failed,
  2.28 s. Seven scenarios cover the actual four-PNG sequence; nested
  `program → batch → office_file` and parallel nested calls; and rejection of
  forged metadata, an unverified failed writer, another writer with identical
  bytes, tampering and a failed enclosing Program. Successful cases inspect
  the real PNGs and require the matching aggregate Passed report in the
  original committed receipt. See [after-fix.log](after-fix.log).
- Same fresh ACP test binary, `office_file` group: **19 passed, 0 failed**, 2.49 s.
- `cargo test --locked --offline -p a3s-code-core --lib native_artifact --
  --nocapture`: **5 passed, 0 failed**, 0.02 s. This includes four existing native
  proof acceptance/refusal tests and the new concurrent prepared-call isolation
  case with a pre-existing owner's evidence.
- Scoped pinned Rust formatting and `git diff --check`: 10 source/test files,
  exit 0. Full workspace checks are left to the root agent's combined run.
- Independent read-only review by `/root/native_execution`: no required safety
  changes; production sources frozen after the checks above.

The successful real-Harness scenarios made 5 and 4 model calls respectively;
each rejection scenario made 3. These are observed synthetic-provider calls,
not live model latency measurements. The initial fixture omitted Work's
`with_assistant_final_response(true)` and ended after its first verified tool;
that was corrected before the valid RED. An initial unqualified `--exact`
filter selected zero tests, and one unsupported multi-filter Cargo invocation
did not run tests; neither counts as validation. Their logs remain. The initial
post-fix fixture overasserted four model calls instead of the observed five;
that assertion was replaced by terminal, file and durable-report checks.

## Patch and provenance

[production-only.patch](production-only.patch) contains only this repair's five
production-file changes. It excludes the pre-existing schema-rejection
`ToolEnd` hunk in `fact_control/receipts.rs` and preserves it in the working tree.
[source-sha256.json](source-sha256.json) records production before/after hashes
and final test-file hashes. Before bytes were mechanically reconstructed from
HEAD plus that preserved pre-existing hunk after editing; they were **not**
hashes measured before editing. Reconstructed backups are under
`/tmp/aster-nested-native-artifact-proof-before-20261019/`. No source or index
was reverted to create them.

## Explicitly not claimed

During the implementation stage, no GUI operation, model submission, deployment,
live case rerun or change to the frozen A09 failure/outputs occurred here. The original A09 remains failed. PNG
structure checks prove valid persisted artifacts, not visual correctness or
compliance with every task requirement. This change does not fix the existing
gate message's Bash-only next-step suggestion. No other sessions, account
configuration, credentials or ACL contents were inspected.

## Appendix: frozen v9 candidate built and loopback verified; original UI pending

The root agent subsequently completed `just build-work` (session 8209, exit 0),
confirmed all 49 source entries against its frozen baseline, and copied the
main executable, ACP, Office and delivery-test executable into an isolated
`frozen-build-v9` directory. It then deployed the three runtime executables as
existing-discovery `Contents/MacOS` siblings. Those build, install and smoke
operations were performed by the root agent; this follow-up only inspected
their evidence and installed code files.

Evidence root: `/tmp/aster-task-corpus-cua-20261018-2245/evidence/`.

- Final frozen manifest: `build-v9-frozen-runtime-manifest.json`, SHA-256
  `99c700b85817776164281416a0ebb3a6d6a78642d6c9011f9d65f079ad9a9bb2`.
- Installation identity: `build-identity-v9.json`, SHA-256
  `5058e522f1fa874cfbe43fa3f8a7715b08458486108a0092dd89b91d9722549b`.
- Frozen real-ACP delivery log: `v9-frozen-real-acp-delivery.log`, SHA-256
  `ac2a5524022921ce7685e05b0f3751034350d1983f5770ddf64be243fb76d955`:
  **3 passed, 0 failed**, 3.39 s. The Work test executable and ACP ran from the
  same isolated directory. Cases cover actual native files plus final text,
  tool refusal plus provider failure, and truncation preserving partial text
  while refusing successful delivery.

The earlier shared-target transport run also passed three tests, but it was
not bound to the frozen ACP hash `f0402302…`; it is not evidence for that frozen
binary. The second, isolated three-test run is the evidence for `f0402302…`.
After the initial target snapshot, another shared-target build changed ACP to
`7484eb0f…`. The deployment guard rejected that attempt before backup or install
mutation; `v9-deploy-precondition-rejection.json` retains the rejection. The
original target snapshot remains unchanged and is superseded by the final
frozen-runtime manifest, rather than overwritten.

Independent read-only follow-up rehashed the three frozen executables, linked
them to the installation record's pre-sign hashes, and checked current installed
bytes against its signed hashes:

| Installed executable | Signed SHA-256 |
| --- | --- |
| AsterDream | `b999a0970a723b3d8d0a637cf3cc28164f20050494662d2ce8218f00cf060d54` |
| a3s-code-acp | `0739de0789989e2c90143d97bc537e3a2a96a37f11aa5717daa81706a8b977f1` |
| a3s-office | `7069109477c527903001759231fa35738c11de06f5e35531472c682faf71342e` |

All three `codesign --verify --strict` checks and the bundle's
`codesign --verify --strict --deep` check exited 0. Frozen and installed main
Mach-O UUIDs match: **766FF8E2-09F0-3EFD-ABD4-0736B3A933A2 (arm64)**. Signed
hashes differ from pre-sign hashes as expected; the original pre-sign copy
observation belongs to the root agent's install record. The independent check
does not pretend to repeat that past observation.

The separate noclobber verification record is
`build-v9-installed-independent-verification.json`, SHA-256
`100453fefc8248ffba0440de669fd39f187fe5c57448a1142539122edc1a9cae`.
This follow-up performed no build, signing, install, process or GUI operation.
The original A09 UI/task rerun and acceptance remain pending; the frozen
original failure and task outputs have not been reclassified or replaced.
