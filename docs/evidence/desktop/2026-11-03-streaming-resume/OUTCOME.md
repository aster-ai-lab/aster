# Resumed Studio streaming investigation and TUI exit analysis

Session: `a8d2c73e-4ec4-47b4-a500-47b0a1d2ac4a`.
Host: macOS arm64, Node 26.8.1, pnpm 12.6.0.

## Recovered task

The last user request was how to fix Studio code streaming. The original session stopped after preparing a temporary Vitest probe (event 565); its result and a final answer were absent. That probe used literal `\\n` characters rather than Markdown line breaks and did not establish the proposed performance diagnosis. Its original is preserved privately; the temporary test was replaced with a regression against the actual renderer.

## Fix

`StreamingMarkdown` now forwards active generation independently of visual animation. `AppMarkdown` supplies the actual streaming state, including when its existing 64 KiB limit disables animation and when generation settles.

Streamdown 2.5.0 already parses the new blocks but, without an animation plugin, renders a separate block state updated through a React transition. The regression demonstrated an old first line still visible after the new chunk committed. The existing pnpm dependency patch now renders parsed blocks directly during active streaming and avoids scheduling their state copy as a transition. The package version and parser remain unchanged.

The Aster task renderer already displays an open code fence as source and defers syntax highlighting until closure. That path was retained. Stream settlement continues to preserve an edited code draft.

The Studio dependency patch, generated lockfile, caller change and long-message test were incorporated into concurrent local Studio commits `91fc5b18b` and `ef50d5957` during this investigation. Shared UI changes are recorded separately. Raw session and debug data are excluded from this evidence.

## Checks

| Check | Result |
| --- | --- |
| New regression before the fix | Failed: new second line was absent |
| Shared Markdown tests | 92 passed in 7 files |
| Studio AppMarkdown and TaskConversationMessage tests | 145 passed in 2 files |
| Shared UI typecheck | Passed |
| Changed Studio files ESLint | Passed |
| Scoped formatting and whitespace checks | Passed |
| Dependency lockstep check | Passed |
| Renderer production build | Passed; existing dependency warnings remain |
| Studio web typecheck | Failed with 355 diagnostics; none in the changed Markdown files |
| Existing browser Markdown syntax suite against the isolated production preview | 2 failed before Markdown assertions: duplicate session-button locator in light mode; theme setup timeout in dark mode |

The browser failures are retained without changing the existing tests to get a pass. Component checks exercise the actual Streamdown implementation, including a message above the animation cutoff. They do not establish throughput or frame-rate improvements under a live model stream. No GLM or other model request was made for these deterministic renderer checks.

The full mutating `pnpm lint` command was not run against this concurrently edited checkout. Scoped ESLint, formatting, UI typecheck and Studio web typecheck were run instead. No Rust code was changed by this fix.

## TUI exit

The original process completed terminal teardown at 08:23:27 local time, then recorded `session_end.worker_join` with `outcome: failed`. In the implementation, `failed` means the worker returned `Err`; a thread panic has a separate outcome. The original log omitted the error itself. Worker failure during shutdown can also follow channel closure, so this event alone does not identify the initial exit trigger.

The 174 memory samples peaked at 83.53 MiB RSS and ended at 69.47 MiB. They do not support sustained memory exhaustion, but cannot exclude an unsampled transient spike. No matching Aster panic stack or macOS crash report was found. The user no longer has the original terminal error.

A controlled PTY probe resumed the same session using the installed matching CLI/ACP pair (`aster 0.1.7`, build `ca162c96e9d4`). It sent only `/quit`, exited with code 0 after 10.05 seconds, and recorded `worker_join: joined`. It did not replay the interrupted tool or issue a model request.

**Conclusion:** the unfinished Studio issue is addressed and covered by regressions. The original TUI exit cannot be assigned a precise cause from the surviving evidence; the resume-and-quit path did not reproduce it. The current source distinguishes worker errors from panics and includes worker error details in its shutdown log. Those diagnostic changes were already present and were not added by this fix.

See `tui-exit-summary.json`, `browser-result.json`, compressed check logs and `source-hashes.json` for the bounded evidence. Original session files, terminal bytes and full debug logs remain private under `.git/`.
