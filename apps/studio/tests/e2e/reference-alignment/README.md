# Reference material live evaluation

This lane uses the real AsterDream `TurnService`, a real ACP process in an owned
Ubuntu MicroVM, and exactly `boyue/deepseek-v4-flash`. It never feeds task
solutions or graders to the model. It does not qualify the signed-in product
gateway or the Tauri GUI. External integrations, ToBox and ToBot product behavior are
outside this lane.

Build the ignored native test and an aarch64 Linux ACP binary with Rust 1.97.1.
Use `llvm-strip --strip-debug` on a copy of the ACP executable if needed. The
driver accepts the exact executable paths, and records their hashes. Credentials
come from `BOYUE_API_KEY` or the existing gitignored `scripts/harbor/.env`; the
model and base URL come from `scripts/harbor/native-tb.acl`, parsed by `a3s-acl`.

The prepared local image `a3s-studio-reference:ssh-ubuntu24.04` is Ubuntu 24.04 with
Bash, Git, Python, bubblewrap, curl, OpenSSH server and CA certificates. Prepare it through the
installed complete Box bundle, using an owned temporary box, then commit its
filesystem as the local image. No Docker execution is used. The original verifier
still installs its own pinned Python/pytest tools via its unchanged `test.sh`,
with `HOME=/root` in a separate fresh verifier VM.
ACP uses standard SSH stdio with a per-job key and a host key pinned through the
trusted VM control channel. Password login and forwarding are disabled. Private
keys are removed at cleanup.

From the repository root:

```sh
python3 apps/studio/tests/e2e/reference-alignment/run.py \
  --task regex-log \
  --box-cli <complete-installed-bundle>/a3s-box \
  --native-test-bin <compiled-studio-lib-test> \
  --linux-acp <compiled-aarch64-linux-acp>
```

Repeat with `--task git-leak-recovery`. Each invocation creates a fresh private
job, native state and MicroVM; input instructions are byte-identical. Only the
workspace is mounted. The ACP executable and generated private model
ACL are copied separately and the VM is removed in cleanup. The Git setup executes
inside that VM only. Before admitting the grader, the driver removes the private
model ACL and destroys the entire agent VM. It then mounts the resulting workspace
in the fresh verifier VM, without the agent executable or credentials. Both VMs
are removed during cleanup. Only the fixture repository is marked safe for the
root Git verifier; no wildcard trust is granted.

Read `task-result.json`, `turn-result.json` and `verifier/` in the reported job
directory. Both native success and original reward `1` are required. A successful
grader is reported as `grader-pass`, not full benchmark certification: the current
host workspace mount has no enforced 10 GiB quota. Keep this resource difference
visible until the acceptance environment supplies that quota. Preserve failed jobs
and rerun only after fixing the recorded execution/harness defect.

Synthetic feature fixtures use `--source tests/e2e/reference-alignment/cases`
(from AsterDream) or the corresponding repository-root path. Tables, content and
web artifacts have separate structural/data checks; real UI journeys remain
separate acceptance work in the alignment plan.

The eight Word `review-*` cases exercise the actual Word extraction and segmented
review host. They publish the local review package into a fresh test-owned
ReconcileStore, select its existing `legal-expert` agent, and copy a hash-checked
snapshot into the ACP VM. This is controlled test authority, not signed registry
or installed-product acceptance. Instructions substitute only the fixture's
`{{WORKSPACE}}` path; original benchmark instructions remain byte-identical.
Review responses, including rejected responses, are retained without reasoning.
The verifier receives those responses and expected answers only after the agent
VM is destroyed. Use the prepared verifier image; review tests run its pinned
Python/pytest cache offline.

`build-review-fixtures.mjs` creates the short synthetic Word inputs with the
already-installed `docx` dependency. Goldens and graders stay outside `inputs/`.
The fixtures cover a selected local whitelist, correct text, selected official
checks with an unknown invalid date, substantive security flaws, irrelevant
text, mixed review and ambiguous document selection. They do not certify the
UI, document-version conflict handling or formal package publication.

The ambiguous-document case may validly stop at a typed ACP user question. Its
report keeps `requires-input`; the driver checks that the host has selected no
document and produced no findings or other tool actions, then closes the owned
runtime. This negative guard alone does not prove answering the question or completing a
review. The separate `review-selected-b` case supplies a private explicit choice
for the second document, then checks the actual native DOCX extraction, exact
finding quotes and whitelist. Its final candidate also retains ACP tool updates
and requires none. Source identity reaches the renderer as a native
`data-review-source` document part. Native-window recovery and source-version
conflict handling remain separate acceptance.

`studio-question-choice` and `studio-question-text` use the native owned form
binding to answer a real ACP `ask_user` call. The choice case explicitly selects
the second option, and the text case has no synthetic choice. Their private reply
fixtures never enter the model workspace. They prove answer transport and a final
Boyue echo; they do not prove reading or reviewing the selected document.
AsterDream's question bridge requires both `allowFreeText` and `hasChoices` in the ACP
question metadata, so deploy the paired AsterDream/ACP changes together.

`review-text-basic` and `review-text-clean` submit `inputs/source.txt` as explicit
pasted text through `turnContext.reviewSource`, with the task kept separate.
They use the same controlled review publication, segmented host and strict finding
checks as Word review. The verifier checks the native source digest, empty Word
binding, exact input preservation, whitelist/correction behavior and clean `[]`.
Renderer tests and browser inspection remain separate from native model acceptance.

Review validation permits one visible correction request for a rejected segment.
It reuses the same source, task and worker; it does not relax the finding parser
or grading checks. A second rejection fails the turn. Live reports retain all
finished responses and `reviewRetries` events. A passing first response is not
evidence that the live correction branch ran.

The driver freezes grader, launcher and expected data in private `verifier-inputs/`
before the model starts. Those files enter only the separate verifier VM after
the agent VM is destroyed. Hash checks still refuse a changed source grader or
changed verifier inputs; keep that result as infrastructure failure and run a
fresh job against the intended source revision.

Generate only the selected-document fixture with
`node tests/e2e/reference-alignment/build-review-fixtures.mjs review-selected-b`.
Its first candidate is correct text and its second contains the target typo.
The private `tests/reply.json` is copied only to the native harness, never the
model workspace. Read receipts cover host DOCX extraction; input hashes and ACP
tool-update receipts are checked separately.

Word review now records `documentRevision` in native review memory and `sha256`
in the persisted document-source part. This is a content digest of the exact
bounded ZIP bytes, distinct from the workspace snapshot's identity-bound CAS
revision. The Word grader independently hashes the input and requires the native
source part, memory and extraction receipt to agree. Renderer comparison uses
Web Crypto over its bounded stable-snapshot bytes before importing or annotating.
Historical messages without a content digest do not gain a current-file binding.

A structural grader pass does not accept extra rewriting under basic proofreading.
Retain the separate PM scope audit: one real reply corrected the typo but also
shortened otherwise correct words. Its source-version proof remains valid while
its business-quality result is failed. The tightened package rules were evaluated
again on the same Word and text inputs; no response was post-filtered or relabeled.

Question cases now rebuild the native event subscription before answering, replay
`TurnService::attach`, and require its form to equal the persisted checkpoint and
original request. They submit the replayed binding and retain
`questionReattachments`; no answer is inferred from permission approval. This
qualifies native resubscription and checkpoint recovery, not a Tauri window reload.
The separate native expiry campaign advances Tokio's test clock through the
unchanged 1,800-second question deadline. Its actual closed chunks are retained
for SDK/renderer replay. Terminal and orphaned requests become `closed` while
answered values and source remain unchanged; old terminal rows are repaired on
message recovery when no owned turn is live.
