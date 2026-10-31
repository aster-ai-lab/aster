#!/usr/bin/env bash
# Rebuild office-kernel.wasm from crates/office. Skip when every copy is newer
# than the kernel sources and Cargo.lock. Packaging sets OFFICE_KERNEL_REQUIRED=1
# so a stale kernel cannot ship when the wasm32 target is missing.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKSPACE_ROOT="$(cd "${ROOT_DIR}/../.." && pwd)"
TARGET="wasm32-unknown-unknown"
PRIMARY="${ROOT_DIR}/generated/office-kernel.wasm"
SOURCE="${CARGO_TARGET_DIR:-${WORKSPACE_ROOT}/target}/${TARGET}/release/aster_office_web_kernel.wasm"

outputs=("${PRIMARY}")

needs_rebuild() {
  local wasm="$1"
  [[ -f "$wasm" ]] || return 0
  local path
  while IFS= read -r path; do
    [[ -n "$path" ]] && return 0
  done < <(find "${WORKSPACE_ROOT}/crates/office" \
    \( -type d -name target -prune \) -o \
    \( -type f \( -name '*.rs' -o -name '*.toml' \) -newer "$wasm" -print \))
  [[ "${WORKSPACE_ROOT}/Cargo.lock" -nt "$wasm" ]]
}

if command -v rustup.exe >/dev/null 2>&1; then
  RUSTUP=rustup.exe
  CARGO=cargo.exe
  if command -v cygpath >/dev/null 2>&1; then
    MANIFEST_PATH="$(cygpath -w "${WORKSPACE_ROOT}/Cargo.toml")"
  else
    MANIFEST_PATH="$(wslpath -w "${WORKSPACE_ROOT}/Cargo.toml")"
  fi
else
  RUSTUP=rustup
  CARGO=cargo
  MANIFEST_PATH="${WORKSPACE_ROOT}/Cargo.toml"
fi

sync_dist() {
  local wasm="$1"
  local dist
  dist="$(dirname "$(dirname "$wasm")")/dist/office-kernel.wasm"
  if [[ -d "$(dirname "$dist")" && ( ! -f "$dist" || "$wasm" -nt "$dist" ) ]]; then
    cp "$wasm" "$dist"
    echo "Copied office kernel to ${dist}"
  fi
}

pending=0
for wasm in "${outputs[@]}"; do
  if needs_rebuild "$wasm"; then
    pending=1
    break
  fi
done
if [[ "$pending" -eq 0 ]]; then
  for wasm in "${outputs[@]}"; do
    sync_dist "$wasm"
  done
  echo "Office kernel is current"
  exit 0
fi

if ! "${RUSTUP}" target list --installed | grep -qx "${TARGET}"; then
  if [[ "${OFFICE_KERNEL_REQUIRED:-}" == 1 || ! -f "$PRIMARY" ]]; then
    echo "Missing Rust target ${TARGET}; run: rustup target add ${TARGET}" >&2
    exit 1
  fi
  echo "Office kernel wasm left in place; install ${TARGET} to rebuild it from crates/office" >&2
  exit 0
fi

"${CARGO}" build \
  --locked \
  --manifest-path "${MANIFEST_PATH}" \
  --package aster-office-web-kernel \
  --target "${TARGET}" \
  --release

for wasm in "${outputs[@]}"; do
  mkdir -p "$(dirname "$wasm")"
  cp "${SOURCE}" "$wasm"
  echo "Built ${wasm}"
  sync_dist "$wasm"
done
