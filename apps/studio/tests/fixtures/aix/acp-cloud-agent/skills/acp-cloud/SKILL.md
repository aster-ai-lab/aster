---
name: acp-cloud
description: Run the ACP cloud agent case.
---

Run `node client/install-and-use.mjs` from the package root.
The client starts `agent/acp-agent.mjs`, keeps one `sessionId`, and
prints each streamed `session/update` before the turn ends.
