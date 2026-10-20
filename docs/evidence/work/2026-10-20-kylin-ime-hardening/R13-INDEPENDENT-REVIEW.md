# Independent R13 review

**Verdict: no remaining blocking finding in the final two-file R13 change.** This is a read-only source, caller and test review by the independent GUI reviewer. It does not replace final installed WebKit acceptance.

The change preserves the optimistic reservation write and follows it with one existing SWR revalidation without awaiting the read. It repairs the concrete late-ack/terminal-fetch overlap demonstrated by the real-SWR regression. The detailed reproducer, before/after identities and logs are in [FAST-COMPLETION-HANDOFF.md](FAST-COMPLETION-HANDOFF.md).

## Acceptance and failure flow

The actual controller awaits `historyAdapter.seedReservedMessages`, then returns accepted `true` (`useConversationTurnController.ts:83`). The actual composer clears the draft when `sendQueuedPayload` returns true (`AgentComposer.tsx:2210`). The new read is `void mutate().catch(...)` (`useAgentSessionParts.ts:303`), so a slow or rejected database read cannot enter the controller's catch/rollback branch and misreport an already accepted send as a failure. The optimistic reservation remains visible before that read resolves. The final regression explicitly keeps the send accepted and its two reserved rows after the read rejects.

Empty reservation input returns before any mutation or read. A duplicate nonempty reservation still invokes SWR's local mutation, which can invalidate the timestamp of a concurrent terminal read even if the updater returns unchanged pages. Its follow-up revalidation is consequently required and is tested with real SWR. Existing duplicate final content is not replaced by pending parts.

## Session ownership

The pinned SWR source has a stable bound mutator that reads `keyRef.current` (`node_modules/swr/dist/index/index.mjs:505`). SWR Infinite's callback retains its own key but delegates to that bound mutator (`dist/infinite/index.mjs:245`). An old callback cannot therefore be assumed to stay bound to the originating session merely because the React callback dependency includes `sessionId`.

This is relevant to the real caller: `AgentChat` owns the runtime hook and is not keyed by session; only its inner `AgentChatSessionCenter` is keyed. The final seed checks the committed owner token before its optimistic mutation and again after the local await (`useAgentSessionParts.ts:277`, `:301`). These checks prevent both an old A seed from writing into B and an A continuation from refreshing B after selection changes during its await.

The owner uses the existing per-session token but updates its separate imperative ref in a layout effect (`:197`). The original projection ref remains unchanged. A suspended, uncommitted B render therefore cannot steal ownership from the still visible A. The maintained Suspense regression preserves A's reserved rows and final reply while B remains suspended. Session A→B→A also obtains a new token, so a retained callback from the first A selection is not mistaken for the current selection.

## Test and evidence review

The eight maintained cases use real SWR/infinite pagination, retained conversation query, reactive cache hooks, the actual controller and terminal handoff. Only the IPC DataApi and cross-window cache service use the repository fixtures; `mutate` is not mocked. Cases cover both ack/terminal orders, duplicate overlap, late old refresh, retained old seed, selection during seed await, a suspended alternate selection, and accepted send with a failing background read. The selection-await assertion excludes the normal key-change fetch baseline rather than misclassifying it as the old continuation.

I inspected the final source, all direct seed callers, the accepted-send/clear path, the pinned SWR implementation, all eight maintained cases, and the recorded receipts. The recorded focused result is 8/8; the adjacent five-file result is 58/58. The byte-copy receipt matches the isolated tree. The complete TypeScript result remains the same 278 baseline diagnostics, with zero new diagnostics; it is not described as a complete type-check pass. I did not rerun these completed checks during the host build.

The prior GUI's empty assistant area is consistent with this independently reproduced race. Its in-memory mutation order was not captured, so this review does not claim that R13 uniquely explains every earlier blank screenshot. The unchanged instant-SSE fixture and the final installed application still need visible reply verification together with the IME key path.
