# Read-only permission-source freeze drift audit

The current two sources differ from the earlier permission handoff freeze only
by a plan-first instruction helper and the plan switch's visible styling/hint.
The current source hashes are stable and the relevant three test files pass
**185 tests, 0 failures**. No production or test source was written in this audit.

## Exact bounded delta

The earlier frozen sources were reconstructed entirely in memory by removing
only the following current additions. Their SHA-256 values exactly match the
previous recorded freeze, proving there is no other byte delta in these files.
See [source-audit.json](source-audit.json) and the complete bounded
[frozen-to-current.patch](frozen-to-current.patch).

| Source | Earlier frozen SHA-256 | Current SHA-256 |
| --- | --- | --- |
| WorkbenchPermissionPicker.tsx | ee09dff155ea94898fbc45076f62e4bef269f52e96d831bfe61c3554c9d669fc | 9011c21162bf44ec2fb6b19636b845250667701e6f957a61e3ae64f90e72a23c |
| workbenchPermissionPostures.ts | 181f961ac7aa0cf35d3e4339cee93f67cb0aa804a04b2b0a115e5fbb5688686c | fe37c2175dcd2035f09041806835ab9e6bc07070dd069778439a02cdd5f24f09 |

- Postures adds only `PLAN_FIRST_DIRECTIVE` and `withPlanFirstDirectives`.
  Disabled returns a copy. Enabled prepends the existing bounded turn directive
  saying this turn should write steps/reasons, avoid file edits and commands,
  and await confirmation, then caps the list at eight. The helper does not
  change an approval mode, member scope, task identity, storage key, or policy.
  The current Composer calls it when constructing a plan-first turn context.
- Picker adds only the existing localized hint title, a visual separator, and
  the visible switch track/thumb DOM and styles. The switch's role, checked
  state, owner-scoped record read/write, mode mapping, commit and click callback
  are unchanged. Both English and Chinese hint keys exist in current locales.

The responsible writing process/author cannot be established from file contents
or modification times, so no author is inferred. This agent did not write these
two files after its recorded freeze. Its later busy-scope implementation touched
only AgentComposer, AgentPage and their sibling tests. The newer helper and
styling were already present when that slice began. No hunk was reverted.

## Verification

Command from `apps/work`:

```sh
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/agents/__tests__/WorkbenchPermissionPicker.test.tsx src/renderer/pages/agents/__tests__/workbenchPermissionPostures.test.ts src/renderer/components/composer/variants/__tests__/AgentComposer.test.tsx
```

Actual counts in [focused.json](focused.json):

- Permission picker: 19 passed.
- Permission postures/storage/directive: 5 passed.
- AgentComposer, including permission handoff, member checks, plan-first
  transport, home activity isolation and existing follow-up behavior: 161 passed.

Total **185 passed, 0 failed, 0 skipped**. Two-source `oxfmt --check` and
`oxlint --deny-warnings` both exited 0; see [format](format-check.log) and
[lint](lint.log). Before/after source hashes are identical. Composer remained
`6c661326d083c398ddcc71525a8bd6a04a4a10df7e5f80fef7ef7c34a1a3ab25`.
No unrelated project checks were rebuilt or repeated.

## Explicitly not claimed

- No product/test source edit, build, deployment, restart, UI, model, provider,
  account, credential, task artifact, or historical session was accessed or run.
- A model instruction is not proof that a real model will obey plan-first mode,
  nor proof of a new execution-policy gate. This audit verifies construction,
  scope and renderer regressions only.
- The unused v6 renderer build is not adopted by this audit. Root owns the new
  complete source baseline and subsequent v7 build/deployment decision.
