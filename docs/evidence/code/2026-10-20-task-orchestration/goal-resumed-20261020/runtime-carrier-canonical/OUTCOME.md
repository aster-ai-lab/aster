# Runtime carrier attachment canonicalization

Status: real RED reproduced; minimal production repair passed the owning
attachment suite with and without `preserve_order`; the root-owned real R3
ACP-to-Linux-guest carrier regression also passed. Cargo remains owned by the
root agent.

The original CUA run reported
`rootfs, mount, or process I/O attachment inventory differs from config.json`.
Only its `tool.result` error/receipt was inspected: sequence 29 reported
`sandboxed=true`, `sandbox_available=true`, `command_started=false`, outcome
`failed`, and an empty complete effects observation. No reasoning was read.

The failure originates in the public SDK's `CreateAttachments::validate`.
It rebuilds root/mount configuration descriptors and compares their value
digests before runtime reservation or Bash payload execution. The old SDK
copy has the same root/mount/process-I/O comparison and `digest_json` encoder;
the current copy additionally validates storage-path restrictions. The error
is not evidence of a missing rootfs file or a failed shell command.

[investigation.json](investigation.json) binds both actual binary inputs to
their existing Cargo dependency fingerprints:

- The staged unsigned ACP, SHA-256 `073ad2aee0e0977ebea537dbfe689bf79a1e3387142ba846494ee0998f9c496c`,
  exactly matches `target/debug/deps/a3s_code_acp-8d1de50a22ef8f81` and the
  stage manifest. Its Core and OCI SDK dependency chain uses serde_json with
  `preserve_order`.
- The qualified unsigned Host, SHA-256 `7dacc9fcd9d91b85f79de8b01c294215eeb7f60d5d41d122c096ddc54e9652fe`,
  exactly matches `target/t013-sha-build/debug/a3s-oci` and the retained asset
  qualification. Its SDK dependency chain uses serde_json without
  `preserve_order`.

`digest_json` currently calls `serde_json::to_vec(Value)`. This retains object
insertion order with `preserve_order`, but sorts keys with the default
BTreeMap representation. Identical JSON values can therefore produce
different root/mount digests across these actual client/Host builds. The Core
carrier derives its manifest from its prepared bundle; the Host reconstructs
the baseline from the unchanged transmitted JSON. No guest entry is needed
to reach the failed comparison.

The SDK already owns `fingerprint::canonical_json_bytes`, which recursively
sorts object keys and preserves arrays/scalars. The production repair
reuses that helper at `attachment::digest_json`, retaining every
pointer, schema, transport requirement, digest comparison, and resource
classification. This restores the documented canonical-value SHA-256 rule
and the old default-encoding bytes; it does not add an alternate accepted
digest. A historical manifest produced with raw unsorted object digests is
not silently normalized, accepted, or migrated. Existing immutable bundle
raw-byte digests are not changed.

## Frozen regression scope

[source-freeze-red.json](source-freeze-red.json) records the production,
owning-test, and existing canonical-helper hashes before RED. New tests:

- `attachment::tests::canonical_attachment_value_digest_matches_legacy_sorted_objects`
  pins the old sorted nested-object bytes/digest while varying object order.
- `attachment::tests::canonical_attachment_wire_accepts_legacy_sorted_inventory_across_object_order`
  derives and serializes a legacy sorted v1 manifest, then revalidates it
  against semantically identical root/mount objects in a different order.
- `attachment::tests::canonical_attachment_inventory_rejects_value_array_order_and_raw_digest_drift`
  keeps scalar changes, array reordering, and a second raw-order digest
  rejected.

Root-owned RED command:

```text
cargo test --locked --offline -p a3s-oci-sdk --features serde_json/preserve_order --lib attachment::tests::canonical_attachment_ -- --nocapture
```

[red.json](red.json) and [red.log](red.log) record the root-owned result:
exit 101 in 31.8068 seconds, 3 selected tests, 1 passed and 2 failed. Compilation
took 29.38 seconds; tests took 0.02 seconds. The nested digest mismatch was
reproduced, and cross-order public manifest revalidation failed with the exact
CUA inventory error. The value/array/digest rejection test passed. This is
the real test RED, not an infrastructure or compilation failure.

[source-freeze-green.json](source-freeze-green.json) freezes the repair:
`attachment.rs` SHA-256
`bf6034482083538721bb72a1ba589f79cf8c16fff227b1c2cb4bc56632e08cb0`.
The test file and canonical helper remain identical to the RED freeze.
After both GREEN runs, all three files were rehashed and exactly matched
`source-freeze-green.json`: production `bf603448...e08cb0`, tests
`c1244c7d...114d602`, helper `839b4444...74cae74` (full hashes are retained in
that manifest). No source file was edited during this evidence update.

Root-owned GREEN commands covered the owning attachment group in both feature
graphs:

```text
cargo test --locked --offline -p a3s-oci-sdk --features serde_json/preserve_order --lib attachment::tests:: -- --nocapture
cargo test --locked --offline -p a3s-oci-sdk --lib attachment::tests:: -- --nocapture
```

- With `preserve_order`: [green-preserve-order.json](green-preserve-order.json)
  and [green-preserve-order.log](green-preserve-order.log), exit 0 in 7.0919
  seconds, 29 passed, 0 failed; test execution 0.02 seconds.
- Default feature graph: [green-default.json](green-default.json) and
  [green-default.log](green-default.log), exit 0 in 8.9744 seconds, 29 passed,
  0 failed; test execution 0.02 seconds.

These are the same 29 distinct tests in two graphs, not 58 distinct tests.
Both include the three new canonicalization cases. The pinned sorted-byte
digests and wire revalidation establish compatibility with an old sorted-key
peer at the public SDK boundary; value, array-order, and raw-digest drift stay
rejected.

## Real R3 carrier regression

The root agent subsequently ran the existing owning fixture against the R3
signed ACP and the unchanged qualified `target/t013-assets-hardware` Host:

```text
/Users/roylin/code/aster/target/qualification/t031-r3-cua-20261020/work-tests agent_plane::turns::local_acp_tests::delivery::sandbox::real_acp_default_bash_runs_in_linux_guest_and_reaches_provider --ignored --exact --test-threads=1 --nocapture
```

[t031-r3-default-bash.json](../t031-r3-default-bash.json) and
[t031-r3-default-bash.log](../t031-r3-default-bash.log) record exit 0, 1 passed,
0 failed, wall 23.6629 seconds including identity hashing, process observation,
and cleanup; the test itself took 21.74 seconds. The frozen test artifact SHA
is `fb2efde6fa4dad819991821194be5ad37470d27fb1bd74c6a8fa304e064c8aa0`.
The frozen ACP exactly matches the signed R3 stage artifact, SHA
`7df56d80d617722e4e3afa92335c0a853fd6ce7cb6b5e3d3b8c889aa6edabeb2`.

The controlled loopback provider issued the sole default Bash invocation;
the actual Linux guest result reached its next request (`main_requests=2`,
`guest_kernel=Linux`, `host_platform=macos`). `sandbox_permissions` was omitted
(`use_default`), `host_escalation_requested=false`, and paid model calls were
zero. Process observation bound ACP PID 54900 to the qualified OCI Host PID
55035 and shim PIDs 55051/55052. The terminal record reports
`owned_processes_remaining=[]` and unchanged frozen test/ACP/Office binaries.

This closes the public canonical-digest test to the actual rebuilt ACP,
retained old sorted-key Host, default Bash route, real Linux guest, and next
controlled-provider result. A further source rehash after this regression
again matched all three entries in `source-freeze-green.json`. Only this
OUTCOME was updated; source files, running-task inputs, assets, configuration,
and stage binaries were not modified by this update. The original R2 CUA
evaluation remains separate and was not replaced or redirected.

Static check already run:

```text
rustup run 1.97.1 rustfmt --edition 2021 --config skip_children=true --check crates/oci-runtime/crates/sdk/src/attachment.rs crates/oci-runtime/crates/sdk/src/attachment/tests.rs
git diff --check -- crates/oci-runtime/crates/sdk/src/attachment.rs crates/oci-runtime/crates/sdk/src/attachment/tests.rs
```

Both returned exit 0; see [static-checks.json](static-checks.json).

## Explicitly not claimed

No Cargo test has been run by this agent; the RED and both GREEN runs above
were root-owned, as was the real R3 carrier regression. This agent performed
no VM, paid model, GUI action, running-task mutation, asset/config rewrite,
or frozen-binary replacement. One native regression does not establish a
T01 completion, speed improvement, or cross-platform runtime qualification.
The original R2 CUA failure evidence remains intact.
