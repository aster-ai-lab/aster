# Research

- ReMe public Studio URL is an SPA; the available browser reports unavailable.
  Visible UI is not verified. Official Studio source documentation describes
  browse/detail/edit workflows over workspace Markdown and local disk refresh.
  Reference revision: c773fa7b771d6b72657b5e6e2cc3dc56ed412089.
  Sources: https://reme.agentscope.io/studio/?lang=en and
  https://github.com/agentscope-ai/ReMe/tree/c773fa7b771d6b72657b5e6e2cc3dc56ed412089/reme_studio.
- Actual ACP session setup uses LazyFileMemoryStore in durable_memory_vec.rs,
  not the separate durable_memory repository API nor ToBot cloud banks.
- Collection has a snapshot iterator/revision, exact primary-key fetch, typed
  delete and flush. Management must not reuse relevance Top-K as a full catalog.
- VecMemoryStore.put has text deduplication and skips unchanged content. Manual
  editing is excluded; delete must invalidate vector/digest caches and serialize
  with put/delete/clear so a stale inspection cannot delete a replacement.
- Studio currently generates provider ACL without memory_dir. New sessions need
  a tenant-scoped explicit memory_dir; existing unscoped processes and data are
  left alone and cannot masquerade as an admitted new owner.
- A live ACP owner retains the engine lock. Route to it; no owner means temporary
  same-SDK access, and an external lock remains a visible refusal. Do not hide a
  lock by opening a second database or stale copy.
- Studio's agent-plane root now resolves the existing storage slug, including a
  fork instance, so it uses the same private profile as the native preference
  host. Required profile failure cannot launch with an unscoped memory fallback.
- Real ACP binary startup retains its existing configured-catalog requirement.
  Provider-free desktop inspection uses Core directly when no owner is admitted;
  the host extension itself also works on an empty model/provider LaunchConfig.
  The real subprocess test uses a declared but unused fixture catalog.
