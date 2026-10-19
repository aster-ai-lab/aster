# Form recovery and shared OpenUI contract

Status: focused native, generated-contract and actual ACP/browser form round-trip checks passed.

## Implemented

- Invalid owned OpenUI forms return a typed `render_error` outcome through
  the existing bound form submission route, with a Close retry if delivery
  fails. They do not start a second user conversation turn.
- Native form replies now distinguish submitted values, renderer failure,
  user decline and session cancellation. An exact request/owner/revision
  match is required. Error/decline closes the persisted form part and returns
  an ACP error so the model's tool can finish.
- Native recovery uses the accepted `streaming` status and clears waiting
  approval anchors. The old `running` string was outside the execution-status
  enum. The same correction covers agent-parameter forms.
- `skill.list_local` now has a native workspace route and typed IPC results.
  Workspace skill discovery respects the canonical workspace boundary and
  reads exact stored skill names; it does not infer names from spelling.
- The existing `a3s-form-core` owns the component enum, positional argument
  catalog and bounded native validator. Core tools and the Work host use it
  before parking. Both renderer catalog artifacts and the bundled ask-form
  parameter table are generated from that contract. Tool documentation uses
  the same parameter table, including Grid and the correct Select ordering.
- The prompt only requires loading ask-form when that skill is actually
  listed, and otherwise directs the model to the tool's parameter table.

## Evidence gathered

- Renderer red test: invalid Select source displayed an alert but never
  called the native response channel.
- Native recovery test initially exposed the invalid `running` status.
  After correcting status and waiting-anchor handling, native form tests
  passed the answer, render-error, decline and cancellation paths. Cancellation
  uses the existing `paused` terminal contract, not a new status.
- Refreshed native form run: 34 passed, 1 explicitly ignored artifact producer. The producer was subsequently run explicitly and its native artifacts were consumed by renderer checks.
- Six focused renderer files: 115 passed with two artifact-dependent cases initially skipped. After actual native artifact generation, the two consumer suites passed all 5 cases with no skips.
- Intelligent UI parser/surface tests: 42 passed. Its package type check
  passed. The full renderer type check hit Node's default 4 GiB heap limit;
  no full-renderer type-check pass is claimed yet.
- Workspace skills: 44 renderer/bridge/hook checks and 5 native checks passed.
- Core ask_user_form checks: 7 passed, covering submitted values, missing
  values, rejected renderer, source validation and shared tool documentation.

## Decisions and adversarial scope

The user explicitly approved automatic file attestation, removal of the independent evidence ceiling,
terminal medium defaults, preservation of old disabled model rows, and
no automatic timeout for a successfully rendered form. There is no new form
timeout. Foreign owner/revision, repeated close, cancelled turn and unknown
outcome tests retain the existing fail-closed boundaries.

The rejected recovery alternative was a blanket timeout: it would interrupt
valid long-lived forms and would leave the renderer failure unreported.

## Native browser acceptance and remaining scope

Session `bugfix-native-flows-retry-20261020` used the production renderer,
actual host, ACP/core and a loopback scripted model. `ask_user_form` rendered a
valid OpenUI card; the user changed Topic, submitted it, and the native tool
receipt retained that exact value. The next model step completed and the
composer unlocked. The same submitted card was visible in the signed native
WKWebView. No frontend fetch mock stands in for the form route.

Evidence: `../2026-10-20-native-bugfix-roundtrip/`.

## Explicitly not claimed

The full renderer type check still has unrelated broad-tree errors. No full
renderer type-check pass, user-system installation, publication, commit or
submodule landing is claimed. The final DMG is being refreshed after an
independent JSON/CSP acceptance failure.

Additional actual core/browser adversity: `bugfix-invalid-form-20261020` returns
`unsupported OpenUI form component: Unknown` with `ok: false`; inference
continues, the composer unlocks and the following user turn also completes.
The report, error receipt and screenshot are retained in the round-trip anchor.
