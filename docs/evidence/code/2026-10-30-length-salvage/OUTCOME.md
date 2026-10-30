# Default bounded continuation for Shell sessions

PR #21 changes the Shell `SessionActor` default when neither an environment
setting nor a rollout budget is supplied. The existing partial-answer commit and
continuation state machine now gets its built-in budget of two requests. Explicit
environment false and remote budget zero still disable it, including for Cursor;
Cursor otherwise retains five, and explicit rollout budgets retain their tier.

Scope is `aster agent` Shell paths, including agent headless, stdio, leader and
server. Ordinary `aster --headless`, the normal TUI and Studio use the ACP/Core
path; this change does not modify that path's truncation contract. A misleading
claim that every standalone/headless process has no rollout configuration was
removed. Config-type documentation describes the actual fallback.

## Validation

Base main: `56e0a88f2df07d4a6bfd5dc33686610353aa2014`. Checks ran on macOS arm64
with the pinned Rust 1.97.1 toolchain. The child process environment unsets
`GROK_LENGTH_SALVAGE`; Rust tests do not mutate process-global environment.

| Check | Observed result |
| --- | --- |
| New default-flow scenarios with only the resolver fallback restored to old behavior | 2 failed with `max_tokens_truncation` |
| Each failure rerun alone with the recorded counterfactual binary | Both failed independently |
| `cargo test --locked --offline -p a3s-code-shell --lib length_salvage -- --test-threads=1` | 22 passed, 0 failed |
| `cargo clippy --locked -p a3s-code-shell -p a3s-code-config-types --all-targets --no-deps` | Exit 0; existing warnings retained |
| `cargo fmt --all -- --check` | Exit 0 |

The new tests run the real session actor, sampler and turn loop against the
existing loopback inference fixture. One response ends in `length`, followed by a
complete response: two requests, ordered partial/final text and one continuation
reminder. Three consecutive cuts exhaust the two-request continuation budget,
return `MaxTokens`, retain one reminder and never request the fourth sentinel
response. Existing kill-switch, context/compaction, rate-limit, tool and sampler
state tests remain in the 22-case result.

The frozen test prompt (`write out the numbers`), model (`test`), context window
(256,000), timeout (60 seconds), retries (zero) and assertions did not change
between the counterfactual and candidate runs. Only the resolver fallback changed
for the counterfactual. The final run follows an assertion-message wording cleanup;
Clippy preceded that wording-only change. No grader was changed.

`checks.tar.gz` retains raw logs, the counterfactual binary identity and command
receipts. `manifest.json` records every size/hash. The archive was reopened and all
10 entries compared byte-for-byte with the collected inputs. Source hashes in the
receipt bind the three changed Rust files to this qualification.

After integrating main `19300db7df6fa799441fd455c5949b3f875176eb`, the three
Rust source hashes still match the passing 22-case receipt. The Rust CI matrix now
runs the actual Shell continuation suite on macOS and Linux, with the override
removed only from that child command. `ci-contract.json` records the final
workflow hash, source comparison and successful YAML parse. Remote results are
pending; the ten-entry test archive remains unchanged.

## Explicitly not claimed

- Continuation support for the ordinary ACP/Core headless or Studio path.
- A live model campaign, new release, installed CLI, sandbox-distribution smoke,
  Windows/Linux runtime acceptance, or a completely green remote CI run.
- A clean warning baseline. No lint allowances, prompts, models, graders, timeouts
  or process-global environment were changed to get a pass.
- GitHub recognition of the existing GPG key; local signing and DCO are checked
  separately from GitHub's `unknown_key` account association.
