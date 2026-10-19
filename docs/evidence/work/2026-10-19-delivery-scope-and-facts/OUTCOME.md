# Delivery scope and factual labels

Status: **source checks, frozen combined transport, deployment and three UI qualification gates PASS; original-case retry 0/22, handed off at user request**.

The independent Qwen v8 replay exposed extra missing-value worksheets in A07,
author inferences under a facts label in A30, and unrequested probe files in
A09. The Work delivery contract now explicitly keeps supplemental explanations
and missing-value lists in the reply unless requested as worksheets, separates
source observations from analysis, avoids unrequested probe files, and keeps
implementation details out of the final answer unless requested or needed to
explain a limitation. Source-sheet cleaning and the user's explicit requested
layout retain precedence. No original prompt, source, delivery or scorer changed.

Only the contract and its two existing transport test modules changed. The
contract is 3,948 characters; its maximum 8,000-character caller context plus
the separator totals 11,950, below the ACP 12,000-character host field limit.
An independent text review found no P0/P1 blocker; its clarification to honor
explicit requests for implementation details was incorporated. A final independent
review also retained legitimate calculations from sufficient inputs by prohibiting
inference from totals **alone**. Checks now explicitly include both reply and file
contents; unrequested analysis is omitted. These generic changes follow source QA
findings in A01/A08/A10/A12/A30/A36, without inserting task-specific answer text.

## Validation

- Effective RED: `cargo test --locked -p aster-dream --lib agent_plane::turns::delivery_contract_tests -- --test-threads=1`
  produced **2 PASS / 1 FAIL**, missing the new source-fact rule.
- Final GREEN: the same three tests passed, including the full caller-context
  boundary and session-parameter propagation.
- The existing real-ACP/loopback-provider delivery test passed **1/1**. It now
  checks the actual provider system message for all four new rules. Its five
  tool calls still delivered four validated Office/image artifacts and final
  text; the provider was scripted, so this is transport evidence rather than
  real-model behavior. It used the pre-native-proof-fix paired v8 ACP binary.
- Two initial commands used incomplete Rust module filters and ran zero tests.
  Their logs remain preserved and are not counted as RED or PASS.

Logs are under `/tmp/aster-task-corpus-cua-20261018-2245/evidence/`:
`delivery-scope-v9-red.log` (invalid zero-test filter),
`delivery-scope-v9-test-discovery.log`,
`delivery-scope-v9-red-effective.log`,
`delivery-scope-v9-green.log`,
`delivery-scope-v9-green-final.log`,
`delivery-scope-v9-transport.log` (invalid zero-test filter), and
`delivery-scope-v9-transport-effective.log`.

The final refinement had its own effective RED (2 PASS / 1 FAIL) and GREEN
(3/3), recorded in `delivery-v9-refinement-{red,green}.log`. The RED first
failed on the revised worksheet rule. The original outputs were already frozen.
`just build-work` then exited 0 using the paired workspace ACP and Office build;
its log is `v9-just-build-work.log`. After shared target drift was detected before any installation mutation, the
paired workspace build was repeated and frozen. The frozen test binary and
frozen ACP passed all three loopback delivery/refusal/truncation checks (3.39s),
recorded in `v9-frozen-real-acp-delivery.log`. All three signed installed files
and main UUID were independently verified. The actual UI passed simple reply,
attachment reading and restart persistence. Formal v9 original-case submission
remains 0/22; baseline and handoff instructions are linked in the main report.

## Explicitly not claimed

No original-case result is upgraded. A07/A28 remain strict FAIL; A30's strict
PASS still has independent semantic findings. This change does not prove a
model will obey the rules, fix A02's token truncation, or repair the separate
nested native-proof handoff in A09. All original v8 cases now have actual UI terminal verification and immutable
collection: 13 strict PASS / 9 FAIL, with independent semantic QA recorded
separately. No original-case result is upgraded by this source change. Original
v9 UI retries and whole-workspace CI are not claimed here.
