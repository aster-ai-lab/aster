#!/usr/bin/env bash
# Tauri owns the window. Node owns the renderer port and the runtime file.
exec node "$(dirname "$0")/studio-dev.mjs"
