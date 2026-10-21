---
description: AsterDream desktop host, renderer and local runtime ownership
sources:
  - src-tauri/src/lib.rs
  - src-tauri/src/agent_plane
  - src/renderer
  - src/shared
---

# Architecture Overview

AsterDream uses one Tauri/Rust host and a React WebView. The default `aster` build enables local models and optional organization access. The explicit `legacy` build retains managed organization access. Product capabilities are selected by the build and checked against the installed tenant identity.

## Process Model

- The Rust host owns windows, tenant identity, native storage, organization credentials and the protected loopback API.
- The renderer owns interaction and typed projections. It calls the existing host bridge; it does not receive organization secrets.
- `a3s-code-acp` is the workspace coding/runtime companion. It owns tool execution and goal semantics through ACP.
- `a3s-office` is the workspace Office companion. Both companions must be present in the same sealed delivery generation.

## Data Flow

A task submission crosses the typed native agent-plane boundary, enters ACP and returns structured events. The renderer projects planning, approvals and goal state from those events. It does not infer completion or authorization from assistant prose.

## Four Data Systems

The host owns task/message storage, native preferences, provider records and the member session. Each uses the selected product profile. The new `aster` profile is separate from legacy data. `legacy` retains its existing profile compatibility. WebView storage, instance locks and runtime discovery follow the same tenant/instance identity.

## Service Lifecycle

The native host creates owned stores and runtime services before opening the main window. Local identity and organization membership remain distinct; a local preference scope never provides an organization token or membership.

## AI Core Architecture

Core/ACP execute the task. The host serializes exact approval bindings and reports unavailable goal snapshots truthfully. The renderer supports plan review and revision before execution; execution retains the submitted permission posture.

## Monorepo Structure

The host is under `src-tauri`, the frontend under `src/renderer`, and shared transport/types under `src/shared`. Root Rust workspace membership and a single lockfile are authoritative. The retained legacy vendor tree supplies the Office frontend kernel build dependency.

## Further Reading

- [Shared contracts](./shared-layer.md)
- [Runtime and sessions](../ai/agent-session-runtime.md)
- [Preferences](../data/preference-overview.md)
- [Build and upgrade](../../contrib/app-upgrade.md)
