# R13: final transcript lost after a late reservation acknowledgement

## Confirmed defect and scope

The real Linux GUI had a successful synthetic gateway request and a finalized SQLite assistant row, but some first-session screenshots showed only the assistant heading. Another isolated run visibly rendered the same synthetic reply. The fixture emits a complete OpenAI-compatible SSE response in one flush; no missing required response field was identified. Those observations alone did not identify the renderer state that caused the blank body.

The bounded follow-up found a reproducible agent-session history race. The maintained regression uses the actual SWR/infinite cache, retained conversation query, reactive shared-cache hook, conversation controller and terminal handoff. Only the IPC-backed DataApi service and cache service use the repository's unified fixtures. It does not mock `mutate`, the history hook, the controller or handoff.

1. Native turn completion persists the final assistant parts and publishes terminal status/data-change events. A completion can be observed before the HTTP open acknowledgement has been processed. Native `open` also has a synchronous host-delta path that publishes completion before returning the reservation acknowledgement.
2. Terminal handoff starts an authoritative history revalidation.
3. The late open acknowledgement calls `seedReservedMessages`. Its optimistic `mutate(updater, { revalidate: false })` inserts the pending assistant skeleton. Even a duplicate seed whose updater returns unchanged pages advances SWR's mutation timestamp.
4. SWR deliberately discards the overlapping older revalidation. The terminal handoff resets the overlay, but its terminal identity has not changed, so it does not start another refresh. The visible assistant can retain empty parts despite final content existing in SQLite.

This proves a product defect consistent with the screenshots. It does not establish that this race was the only cause in `run-dTuzP4`; that run did not capture the renderer's in-memory history or mutation order.

## Minimal repair

[useAgentSessionParts.ts](../../../../apps/work/src/renderer/hooks/useAgentSessionParts.ts) still publishes reserved rows optimistically. After that write, it starts the existing SWR revalidation without awaiting its database read. This also runs for duplicate acknowledgements, since their mutation can invalidate an in-flight read. The accepted send and draft handoff therefore remain independent of a delayed or failed background history read. The existing SWR protections against stale fetch results remain enabled; the follow-up fetch restores authoritative parts.

The callback also checks a committed owner token before the write and again after its await. SWR's bound mutator consults its current key, so retaining an A callback after selecting B previously allowed A skeletons to be written into B's cache. A check on the original render-written projection ref would reject the still-visible A session during a suspended B transition. The reservation guard consequently uses a small layout-effect-owned ref of the existing token; the projection cache's existing concurrent-render behavior is unchanged.

The searched production callers are the agent runtime's reserved-message layout seed and the shared conversation controller's acknowledgement seed. The other terminal handoff and reservation helpers on the legacy topic branch were inspected but are not changed or covered by this agent-session repair. No fixture delay, timer, reload, DOM observer or special response content is added to the product.

## Verification and identities

The maintained [handoff regression](../../../../apps/work/src/renderer/hooks/__tests__/useAgentSessionParts.handoff.test.tsx) covers both acknowledgement/completion orderings, duplicate acknowledgement overlap and preservation of final content, a late old-session refresh, a retained old-session seed callback, a selection during the seed await, a suspended uncommitted selection, and acceptance before a background read resolves or fails.

- The exact final eight-case test against the preserved pre-R13 hook: **4 failed, 4 passed**. The failures are the two final-content races, the retained callback writing into the selected session, and the absent background reconciliation. See [exact before log](fast-completion-exact-before.log) and [source identity](fast-completion-before-source.json). Each distinct original failure was also run alone; the earlier smaller five-case log remains historical evidence.
- Fixed focused suite: **8 passed**. See [focused after log](fast-completion-maintained-after.log).
- Adjacent five-file regression: **58 passed**. It includes the eight new cases plus the existing agent-history, controller, terminal-handoff and agent-runtime cases. These counts overlap other renderer checks and must not be added to them. See [regression log](fast-completion-regression.log).
- Scoped Oxlint with denied warnings, ESLint, Oxfmt check and `git diff --check`: passed. See [Oxlint](fast-completion-oxlint.log), [ESLint](fast-completion-eslint.log), [format](fast-completion-format.log).
- Complete Web TypeScript check: exits 2 with the same **278 existing diagnostics**, zero added/removed file-and-error-code counts and zero diagnostics in either R13 file. This is not a complete type-check pass. See [comparison](typecheck-web-r13-comparison.json) and [full log](typecheck-web-r13.log).
- Two product/test files copied byte-for-byte to the isolated worktree. See [SHA256 receipt](fast-completion-isolated-sync.json). No commit or staging was performed.

Commands, from `apps/work`:

```sh
bun x --no-install vitest run --project renderer src/renderer/hooks/__tests__/useAgentSessionParts.handoff.test.tsx
bun x --no-install vitest run --project renderer src/renderer/hooks/__tests__/useAgentSessionParts.handoff.test.tsx src/renderer/hooks/__tests__/useAgentSessionParts.test.ts src/renderer/hooks/__tests__/useConversationTurnController.test.tsx src/renderer/hooks/__tests__/useTopicOverlayHandoffOnTerminal.test.ts src/renderer/pages/agents/__tests__/useAgentChatRuntimeState.test.tsx
bun x --no-install oxlint --deny-warnings src/renderer/hooks/useAgentSessionParts.ts src/renderer/hooks/__tests__/useAgentSessionParts.handoff.test.tsx
bun x --no-install eslint src/renderer/hooks/useAgentSessionParts.ts src/renderer/hooks/__tests__/useAgentSessionParts.handoff.test.tsx
bun x --no-install oxfmt --check src/renderer/hooks/useAgentSessionParts.ts src/renderer/hooks/__tests__/useAgentSessionParts.handoff.test.tsx
node --max-old-space-size=8192 node_modules/typescript/bin/tsc --noEmit -p tsconfig.web.json --composite false
```

The before-only target config points the unchanged maintained test to a preserved hook/query fixture under `target/evaluations/kylin-ime-hardening/fast-completion/before/`. It does not replace production source. That fixture's SHA256 is recorded above so the before result remains reproducible after synchronizing the isolated tree.

## Explicitly not claimed

This subtask did not build, package, install or launch an application, or change native packaging, resources, versions, the synthetic gateway or the frozen installers. It adds one existing bounded history revalidation per nonempty acknowledgement, including duplicates; it does not claim a network-traffic reduction. R4 needs the real WebKit GUI to visibly render the fast synthetic reply with the final native IME module. The prior native IME controls and this hermetic history proof are separate gates.
