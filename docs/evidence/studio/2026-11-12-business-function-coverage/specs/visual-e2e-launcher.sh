#!/bin/sh
# Launch-Services context: /bin/ps (setuid) works here. Restart the
# agent-desktop host in this context, then run the full visual E2E.
# Prerequisite: grant Accessibility to AsterVisualE2E in System Settings.
LOG=/tmp/aster-visual-e2e/e2e-result.txt
{
  echo "started $(date)  arch=$(uname -m)"
  for p in $(/bin/ps -axo pid=,comm= | grep -i 'agent-desktop' | awk '{print $1}'); do kill "$p" 2>/dev/null; done
  sleep 3
  /opt/homebrew/bin/agent-desktop permissions 2>&1 | head -c 200
  echo ""
  /opt/homebrew/bin/agent-desktop list-apps >/dev/null 2>&1
  export NODE=/opt/homebrew/bin/node
  export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$HOME/.bun/bin:/usr/bin:/bin:/usr/sbin:/sbin"
  cd /Users/roylin/code/aster || exit 1
  /Users/roylin/.workbuddy/binaries/python/versions/3.13.12/bin/python3 \
    docs/evidence/studio/2026-11-12-business-function-coverage/specs/agent-desktop-launch-e2e.py
  echo "E2E_EXIT=$?"
} > "$LOG" 2>&1
