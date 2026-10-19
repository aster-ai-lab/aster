#!/usr/bin/env bash
# Print the container to use. Prefer the first name. If that container does
# not exist, use the second name. If neither exists, print the first name.
set -euo pipefail

preferred="${1:?preferred container name}"
legacy="${2:?legacy container name}"

if docker inspect "$preferred" >/dev/null 2>&1; then
  printf '%s\n' "$preferred"
elif docker inspect "$legacy" >/dev/null 2>&1; then
  printf '%s\n' "$legacy"
else
  printf '%s\n' "$preferred"
fi
