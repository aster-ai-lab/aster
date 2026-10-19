# AIX view command proof

Independent canonical AIX package with one ActivityBar container, two isolated
webviews, a native tree and a mini app. ACL command definitions and menu positions
are separate. The same provider script multiplexes state/tree and command messages
on its document-owned channel. Commands modify only this document's UI; no builtin
command, credential, workspace path, Tool, MCP or Harness authority is received.

The script acknowledges readiness and correlated operations. A tree command also
publishes a normal tree revision. The fixture is independent of earlier finalized
packages; the deterministic builder's `view-commands` mode preserves their bytes.
