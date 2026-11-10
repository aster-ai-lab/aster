# DeepSeek bound Goal through the shipped terminal

The existing opt-in `shipped_bound_goal_real_llm` test passed against
`deepseek/deepseek-flash` on macOS arm64. The private fixture kept only the
DeepSeek provider from the operator ACL, selected that route as its default
and retained the original `max` effort. The source ACL digest was unchanged.
The first profile preflight stopped on an incorrect block boundary assumption
before starting the test. The successful run used the unchanged test and
its existing prompt, goal, plan, and verifier.

- The pager seeded a real session. ACP admitted a review against the
  session's engineering policy and rejected a Goal missing that policy binding.
- ACP bound a plan to the pager session, reported its pending step, and paused
  it. After restart the pager still displayed the paused Goal and zero of
  one completed steps.
- `/goal resume` ran one real slice and displayed completion. The test read
  `report.txt` from the workspace and checked it against the random code in
  the input fixture. The slice took 18.65 seconds.
- A fresh ACP process reopened the completed Goal. Reissuing `/goal resume`
  ran no slice and left the entire goal state identical. `/goal clear` also
  cancelled a separate unstarted Goal.

## Verification

The paired binaries matched source fingerprint
`c00dde195b0f55e43b6c211bf5ccb9bf50985ad9` independently recomputed from the
current checkout. The test executable came from the preceding
[shipped suite](../2026-11-11-resume-model/OUTCOME.md). It ran with the isolated
`ASTER_CONFIG_FILE`, `ASTER_TUI_BIN` and `ASTER_ACP_BIN` set to the private ACL
and the two paired binaries.

```text
shipped_tui_e2e-6a2532b45ef1bf5b shipped_bound_goal_real_llm --exact --ignored --nocapture --test-threads=1
test result: ok. 1 passed; 0 failed; 0 ignored; 24 filtered out; finished in 27.50s
```

Seven checkpoints reported success across the pager and ACP paths.

## Explicitly not claimed

This result covers a Goal created and bound through ACP, paused state recovery
and execution in the pager, and ACP reopen and clear. It leaves the TUI's separate Goal
creation entry point, failure and cancellation of a running Goal, the full
independent verifier matrix and native window interaction unverified.
The diagnostic pair contains the separate uncommitted ACP cancellation change.
