# Parent object-fixture source integration

Integrate preserved parent head `3bb286de7f2570093cc714b683ecd71bf56911a9`
through an actual merge, including its retained ancestor source. The sole
gitlink conflict retains canonical Cloud
`73a532a986604e38d5897c595e32a9cf733461ec`; incoming Cloud
`896857126000661fbdae4af10db220423e0275c9` is already its ancestor.

The backend campaign replaces the retired MinIO producer with RustFS and
creates the existing bucket only after its health check succeeds. RustFS
1.0.0 and AWS CLI 2.31.18 use the same immutable image digests as the already
qualified canonical Cloud fixture. The existing 60-attempt health bound,
campaign commands, credentials, endpoints and serial execution are retained.
The backend check job runs the new small hermetic startup regression.

The other 79 automatic-merge file changes were reviewed and reconciled to
the existing bytes. These are fixture identities and derived hashes/sizes,
signed registry metadata, test inputs, source descriptions and historical
records. The alternate host-capabilities fixture also adds two surfaces;
it is a different frozen input, not merely a label correction. Original
fixture contracts and their tests remain coherent and unchanged. Incoming
variants remain reachable through the merged source ancestry. The preserved
UI working-tree patch and historical performance reports are not rewritten.

## Verification actually completed

- `python3 tests/backend_object_fixture.py`: 3 passed. The tests execute the
  actual workflow startup script with command stand-ins. They cover health
  before bucket creation, pinned image identities, refusal after exactly 60
  failed checks with 59 sleeps, and bucket failure exit-code propagation.
- The workflow parses as YAML and every `run` step passes `bash -n`.
- Both image references equal canonical Cloud's references exactly.
- All 79 retained files match their pre-merge SHA-256 values; no unmerged
  index entries remain.

[Test output](startup.log) and [static qualification](qualification.json)
are retained. Actual RustFS health, byte-preserving put/get, orphan cleanup
and PostgreSQL process-death proofs were previously recorded in the owning
Cloud repository at
`docs/evidence/maintenance/2026-10-31-object-fixture-source-merge/OUTCOME.md`.
They are not recounted as new tests in this parent integration.

## Explicitly not claimed

No complete Ubuntu backend campaign or fresh container campaign ran in this
merge. The local Docker daemon was unavailable; the earlier canonical Cloud
fixture qualification remains separate. No production Rust behavior or
frozen evaluation input was changed. No push, DMG or release is claimed.
