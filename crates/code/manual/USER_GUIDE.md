# A3S Code User and Developer Guide

This repository delivers the local Rust SDK (`a3s-code-core`) and the
`a3s-code-acp` companion used by Aster hosts. Members are private workspace
packages. Build from the repository root with its pinned toolchain.

## 1. Integrate the local Rust SDK

A Rust host inside `apps/<host>` uses a local path dependency:

```toml
[dependencies]
a3s-code-core = { path = "../../crates/code/core" }
```

The public implementation and examples in [Core](../core/src/lib.rs) are the
API authority. No foreign-language SDK or registry publication is part of this
product. ACP is the existing process boundary for external hosts.

## 2. Configure the model with ACL

```acl
default_model = "openai/my-model"
providers "openai" {
  api_key = env("OPENAI_API_KEY")
  base_url = "https://api.example/v1"
  models "my-model" {
    tool_call = true
    limit = { context = 32768, output = 4096 }
  }
}
```

Use the host's supported provider endpoint. Credentials stay in environment-backed
ACL values. A chat model does not implicitly select an embedding route. Only
trusted host/user configuration may authorize source egress or optional backends;
a discovered workspace configuration can only narrow that authority.

## 3. Create, use and close a session

```rust,no_run
use a3s_code_core::{Agent, SessionOptions};

async fn run() -> anyhow::Result<()> {
    let agent = Agent::new("agent.acl").await?;
    let session = agent
        .session_builder("/repo")
        .options(SessionOptions::new())
        .build()
        .await?;
    let result = session.send("Find the authentication entry points.", None).await?;
    println!("{}", result.text);
    session.close().await;
    Ok(())
}
```

`send` and `stream` share a fail-fast single-flight conversation boundary.
Finish or cancel an active stream before starting another turn. `steer` and
`interrupt` address an existing run; they do not change its permission ceiling.
Resume restores supported recorded state and requires the host to re-inject live
resources such as providers and MCP clients.

## 4. Inspect the actual tool surface

`sdk_capabilities()` returns the complete capability contract inventory, including
optional entries. It is not a runtime availability check. Inspect the session's
`tool_definitions()` and `presented_tool_definitions()` for its actual tools.

| Surface | Operations | Boundary |
| --- | --- | --- |
| Workspace | read, write, edit, patch, search, ls, git, bash, download | Backend capabilities, workspace scope and permission |
| Composition | batch, task, program | Shared governed executor, child limits and cancellation |
| Web | web_fetch, web_search | Network policy, bounded input/output, redirects and deadlines |
| Extensions | MCP, Skill, host tools | Exact host selection and scoped lifecycle |

Model-selected calls pass through permissions, confirmations, hooks, budgets,
sandbox enforcement and bounded results. Trusted host-direct APIs are a separate
control-plane entry; use the governed API when host calls must follow the same
permission and confirmation policy. Tool visibility never grants authority.

## 5. Retrieval and memory

`search` is the model-facing repository retrieval tool. It supports exact path
and content search, BM25, semantic search and hybrid fusion. Semantic retrieval
requires an explicitly selected provider. The runtime bounds admitted text,
chunks, provider batches and result sizes, then rechecks source digests before
returning hits. Closing a session cancels and releases its owned resources.

Rust hosts supply `EmbeddingProvider` through `WorkspaceRetrievalOptions` and
`SessionOptions::with_workspace_retrieval`. Stock ACP's trusted `local_cpu`
selection uses the embedded light tier. Its internal Rust composition seam gives
an installed factory precedence and refuses a failed factory; the shipped stdio
agent exposes no environment-based factory loader.
Remote routes require explicit source-egress authorization. See
[Workspace Retrieval Operations](WORKSPACE_RETRIEVAL_OPERATIONS.md).

Memory stores retained facts; repository retrieval indexes current source.
They have distinct provenance, retention and authorization boundaries. See
[Durable Memory](DURABLE_MEMORY.md) and
[Durable Engineering](DURABLE_ENGINEERING.md).

## 6. Host and build profiles

Core defaults to `local-code`. Advanced workflows, S3 and telemetry remain
explicit features; see [Release Profiles](RELEASE_PROFILES.md).

Standalone ACP defaults to API/HTTP search. Build browser search explicitly with
`cargo build --locked -p aster -p a3s-code-acp --features a3s-code-acp/headless-search`.
Desktop build/development/package recipes select that capability explicitly.
Browser preparation starts only when the search cascade uses its browser tier.

Office tools are an ACP host capability. A client requests them on `session/new`
with the typed request metadata below; this is protocol metadata, not a product
configuration file:

```json
{"_meta":{"a3sHost":{"officeTools":true}}}
```

Missing or false does not register native Office tools. Malformed values refuse.
Studio declares its Office support explicitly. Source-review sessions expose no
tools even when Office is requested. Registration does not grant read/write
permissions. `output_check` remains available to ordinary sessions for bounded
text and numeric checks. Office source reads currently cover DOCX/XLSX/PPTX;
unavailable PDF/OCR operations are refused explicitly.

## 7. Verification and evidence

A tool return or assistant claim alone does not prove completion. Keep the actual
command receipt, current source identity, required checks and mutation-bound
verification evidence. Record remaining refusals and model failures separately.

Run focused tests from the root workspace. Build and distribute `aster` and
`a3s-code-acp` together; Studio recipes also build Office. A test result applies
to its recorded source and binaries, not later working-tree changes.

## 8. References

- [Rust SDK API design](SDK_API_DESIGN.md)
- [Capability contracts and feature boundaries](SDK_CAPABILITY_MATRIX.md)
- [Advanced host integration](ADVANCED_DEVELOPER_MANUAL.md)
- [Scoped capabilities](SCOPED_CAPABILITY_ARCHITECTURE.md)
- [Workspace Retrieval Operations](WORKSPACE_RETRIEVAL_OPERATIONS.md)
- [Capability Verification](CAPABILITY_VERIFICATION.md)
