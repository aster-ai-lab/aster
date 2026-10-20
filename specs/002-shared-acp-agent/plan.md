# Plan: Shared ACP agent

The spec in this directory is the acceptance. Implementation order is headless, then `aster agent stdio`, then `models` and `worktree` together.

Each entry calls `spawn_required_sidecar` or, for stdio, `run_sidecar_on_stdio`. Both resolve the executable with `a3s_acp_executable` and return `MISSING_AGENT_BACKEND` when it is absent.

The boundary baseline drops the three shell-spawn files in the same change that removes the calls. Stdio stops requiring `run_stdio_agent`.

## Constitution check

Product path. Hermetic tests live in `a3s-code-pager`. The live prompt is an additional gate. No new ACL key. No crate rename. The leader cluster stays on the in-process agent.
