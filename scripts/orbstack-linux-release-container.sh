#!/usr/bin/env bash
# Runs inside ubuntu:20.04. Builds the aster-cli release-dist pair.
set -euo pipefail

apt-get update
apt-get install -y --no-install-recommends \
  build-essential ca-certificates cmake curl git pkg-config \
  python3 libssl-dev unzip

archive=/tmp/protoc-29.3.zip
case "$(uname -m)" in
  aarch64|arm64) protoc_asset=protoc-29.3-linux-aarch_64.zip ;;
  x86_64|amd64) protoc_asset=protoc-29.3-linux-x86_64.zip ;;
  *) echo "unsupported arch $(uname -m)" >&2; exit 1 ;;
esac
curl -fsSL --retry 3 \
  "https://github.com/protocolbuffers/protobuf/releases/download/v29.3/${protoc_asset}" \
  -o "$archive"
unzip -q "$archive" -d /tmp/protoc
export PATH="/tmp/protoc/bin:$PATH"
protoc --version

curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
# CARGO_HOME is set by the outer docker run; rustup installs there.
# shellcheck source=/dev/null
. "${CARGO_HOME:-$HOME/.cargo}/env"
rustup show active-toolchain

version="$(getconf GNU_LIBC_VERSION | awk '{print $2}')"
echo "container glibc ${version}"
test "$version" = "2.31"

env -u GROK_VERSION cargo build --locked --profile release-dist -p aster
env -u GROK_VERSION cargo build --locked --profile release-dist -p aster-acp \
  --no-default-features --features svg-png,account-import

file /target/release-dist/aster /target/release-dist/aster-acp
/target/release-dist/aster --version
/target/release-dist/aster-acp --version
