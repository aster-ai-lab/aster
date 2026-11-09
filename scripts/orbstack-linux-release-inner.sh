#!/usr/bin/env bash
# Runs inside an OrbStack Linux guest (not inside Docker).
# Builds aster-cli with ubuntu:20.04 (glibc 2.31) and packages the archive.
set -euo pipefail

platform="${1:?platform}"
out_name="${2:?out archive basename}"
repo_root="${3:?repo root on shared mount}"
target_name="aster-cli-release-${platform}"
mkdir -p "$HOME/${target_name}" "$HOME/cargo-home"

docker_flags=(--rm -v "${repo_root}:/src" -v "$HOME/${target_name}:/target" -v "$HOME/cargo-home:/cargo-home")
if [[ "$platform" == "linux" ]]; then
  docker_flags+=(--cgroupns=host --privileged)
fi

sudo docker pull ubuntu:20.04
sudo docker run "${docker_flags[@]}" \
  -e CARGO_HOME=/cargo-home \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e DEBIAN_FRONTEND=noninteractive \
  -w /src \
  ubuntu:20.04 \
  bash /src/scripts/orbstack-linux-release-container.sh

cd "${repo_root}"
bash scripts/package-aster-cli.sh "${platform}" "$HOME/${target_name}/release-dist" "$HOME/${out_name}"
cp -f "$HOME/${out_name}" "${repo_root}/dist/${out_name}"
ls -la "${repo_root}/dist/${out_name}"
ls -la "$HOME/${target_name}/release-dist/aster"
"$HOME/${target_name}/release-dist/aster" --version
"$HOME/${target_name}/release-dist/aster" --build-id
