# Tasks: Shared ACP agent

1. Write this spec and the research inventory before treating a switch as done.
2. Switch headless `spawn_agent` to `spawn_required_sidecar`. Test the missing-executable error from `spawn_agent`.
3. Switch `AgentRuntime::run_stdio` to `run_sidecar_on_stdio`. Update the boundary needles in that change.
4. Switch `models` and `worktree` to `spawn_required_sidecar`. Shrink the shell-spawn file set to empty.
5. Lock the initialize advertisement set and the ordinary prompt meta with hermetic tests.
6. Run the boundary unittest, the missing-sidecar test, and record the live prompts separately from scripted conformance.

Recorded: boundary 23 OK; missing-sidecar and the two extension locks passed; scripted conformance 1 passed. Two later live prompts against `deepseek/deepseek-flash` each returned a non-empty reply and usage tokens greater than zero. Earlier `boyue/deepseek-v4-flash` prompts failed with `boyue API account has no balance (HTTP 401)` and were not repeated.
