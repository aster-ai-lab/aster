# Shell bounded continuation integration

The preserved PR #21 source head
`7f858616a89c2fe2058ff43a5011cd7f266976b3` is integrated into the current parent
checkout. Shell sessions use their existing two-continuation default when no
explicit setting overrides it. Both explicit disable paths, the Cursor tier,
bounded exhaustion, ordered partial text and single reminder remain covered.
This does not change ordinary ACP/Core or Studio truncation behavior.

## Current verification

On macOS arm64 with pinned Rust 1.97.1, from the repository root:

```sh
env -u GROK_LENGTH_SALVAGE cargo test --locked -p a3s-code-shell \
  --lib length_salvage -- --test-threads=1
cargo clippy --locked -p a3s-code-shell -p a3s-code-config-types \
  --all-targets --no-deps
```

The real actor/sampler/turn-loop suite passed **22 tests**. Clippy exited zero
with warnings; no clean warning baseline is claimed. The environment override
was removed from that child test process only. No test prompt, model, timeout,
retry, assertion or process-global environment was changed.

The original `2026-10-30-length-salvage/checks.tar.gz` was reopened: its archive
size/hash and all **10 entries** match the original manifest exactly. Those
historical evidence bytes remain unchanged. The merged CI YAML parses and
retains the exact bounded-continuation command in its existing Rust matrix.

## Explicitly not claimed

This is local merged-source qualification. PR #21 still has failing remote
checks at the inspected head; their disposition and final main publication
remain open. No full workspace format pass, other-platform runtime result,
installed sandbox archive, DMG or GitHub Release is claimed here.
