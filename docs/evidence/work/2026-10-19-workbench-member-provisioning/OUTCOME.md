# Workbench default-agent provisioning by member

Date: 2026-10-19

## Result

The renderer's default-agent cache, runtime-unavailable flag, and pending
provisioning promise now belong to an explicit member ID. The local null scope
is separate from every named member. An absent or empty member scope is
refused instead of using local preferences.

Each asynchronous creation updates only its captured member state. Switching
members starts or joins that member's own provisioning operation, and a late
result from another member cannot open an agent through the current hook
callback. Unmounted hooks also ignore late results. The runtime-unavailable
hook reads the current member's own flag.

Only `useWorkbenchDefaultAgent.ts` and its existing sibling test file changed
in this implementation slice. The parent workstream owns AgentPage caller and
permission-storage changes. The permission fallback remains the existing
runtime default; no global permission promotion was added.

## Verification

The initial focused run found 10 failures among the original 20 tests. All
10 had the same test-fixture cause: the tenant-brand mock did not expose
`studioUsesWorkbenchShell`, although the production module already called
it. The fixture was updated in the same existing test file. This was not
treated as a newly introduced product defect.

Six focused cases were added for local/named-member cache and permission
isolation, overlapping creations for different members, late runtime failure,
member switching with late completion, unmounting, and absent identity.

Commands run from `apps/work`:

```text
node_modules/.bin/oxfmt --write src/renderer/pages/agents/useWorkbenchDefaultAgent.ts src/renderer/pages/agents/__tests__/useWorkbenchDefaultAgent.test.ts
/opt/homebrew/bin/node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/agents/__tests__/useWorkbenchDefaultAgent.test.ts
```

Result: **1 test file passed, 26 tests passed, 0 failed**, in 4.26 seconds.
Existing Vite configuration and route-file warnings remained informational.

The scoped `git diff --check` for the source and test files also passed.

## Explicitly not claimed

- No application build, live-host restart, account change, UI operation, or
  model/API invocation was performed.
- This slice does not claim completion of the original task corpus or repair
  of the independent backend execution-mode issue.
- No task sample, answer, model artifact, credential, or live account state
  was read as part of this fix. Tests use synthetic member IDs only.
- No commit was created; concurrent changes in other files were preserved.
