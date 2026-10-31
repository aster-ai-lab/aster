# Official OpenAI SDK conformance

This suite runs the pinned official Python SDK against a real
`aster-gateway` process. It applies a complete inference policy through the
native managed snapshot API and uses a controllable HTTP/SSE upstream.

The cases cover:

- the complete Models, Chat Completions, Completions, and Embeddings endpoint
  matrix;
- typed non-streaming responses and the SDK-default base64 embedding path;
- stable authentication and grant error parsing;
- request rewriting and credential stripping;
- chat and legacy completion SSE framing, final usage chunks, and `[DONE]`
  termination while the upstream remains open;
- explicit SDK stream close and cancelled asynchronous consumers;
- graceful completion inside the configured drain deadline; and
- forced termination at a zero-second drain deadline.

Run it from the Gateway repository:

```bash
cargo build --locked --bin aster-gateway
python -m pip install --requirement tests/openai_sdk/requirements.txt
python tests/openai_sdk/test_conformance.py
```

The same commands work on Windows and automatically select
`target/debug/aster-gateway.exe`. To exercise a binary at another location, set
`ASTER_GATEWAY_BINARY` first:

```powershell
$env:ASTER_GATEWAY_BINARY = (Resolve-Path target/debug/aster-gateway.exe).Path
python tests/openai_sdk/test_conformance.py
```
