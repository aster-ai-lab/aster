#!/bin/bash
# HR L3 — the task-routing certification lane: the official-SDK L2
# suite (a real gateway binary, a real HTTP decision service, managed
# snapshots) plus the routing-facts spool proof, in one gate.
#
# Usage: scripts/run-task-routing-certification.sh
# Requires: cargo, python3 (a venv is created and pinned locally).

set -euo pipefail
cd "$(dirname "$0")/.."

echo "== HR L3: task-routing certification lane =="
echo "1/4 building the gateway binary"
rustup run 1.97.1 cargo build --bin aster-gateway

echo "2/4 preparing the pinned SDK environment"
(
  cd tests/openai_sdk
  if [ ! -d .venv ]; then
    python3 -m venv .venv
  fi
  .venv/bin/pip install -q -r requirements.txt
)

echo "3/4 the official-SDK L2 suite (conformance + task routing)"
(
  cd tests/openai_sdk
  .venv/bin/python test_conformance.py
)

echo "4/4 the L1 routing battery (the full HR family)"
rustup run 1.97.1 cargo test -p aster-gateway --lib hr_

echo "GREEN: the task-routing certification lane passed."
