#!/usr/bin/env bash
# Run inside the H200 RJob, never on the operator workstation.
set -euo pipefail

ROOT=/nvme/aster-power-xq-20261021
ASSETS=/mnt/shared-storage-user/yangchao/Workspace/aster-h200-xq
XOLOTL_REVISION=b3452666e07a6374d06bb8c2185dffa271e8f65f
LLAMA_REVISION=3cf03257f219afbe7334045ff7c6a06ac68c627d

test -d "$ROOT/source/xolotl"
while ! test -f "$ASSETS/source/SOURCES_READY"; do
    printf 'Waiting for verified upstream sources: %s\n' "$(date -u +%FT%TZ)"
    sleep 30
done
python3 - "$ASSETS/source/SOURCES_READY" "$XOLOTL_REVISION" "$LLAMA_REVISION" <<'PY'
import sys
from pathlib import Path
if Path(sys.argv[1]).read_text().splitlines() != sys.argv[2:]:
    raise SystemExit("upstream source identity does not match the pinned build")
PY

mkdir -p "$ROOT/source/llama"
cp -a "$ASSETS/source/llama/ggml" "$ROOT/source/llama/"
PATCH="$ROOT/source/xolotl/tools/xolotl/ggml/xolotl-ggml.patch"
export GIT_CEILING_DIRECTORIES="$ROOT/source"
if ! git -C "$ROOT/source/llama" apply --reverse --check "$PATCH"; then
    git -C "$ROOT/source/llama" apply --check "$PATCH"
    git -C "$ROOT/source/llama" apply "$PATCH"
fi

# The PJLab image has cuBLAS shared libraries but no cuBLAS static archives.
# Keep upstream's default intact and select dynamic CUDA linkage explicitly.
python3 - "$ROOT/source/xolotl/CMakeLists.txt" <<'PY'
import sys
from pathlib import Path
p = Path(sys.argv[1])
source = p.read_text(encoding="utf-8-sig")
old = '  set(GGML_STATIC ON CACHE BOOL "" FORCE)'
new = ('  option(STRATA_GGML_STATIC "Link ggml CUDA dependencies statically" ON)\n'
       '  set(GGML_STATIC ${STRATA_GGML_STATIC} CACHE BOOL "" FORCE)')
if old in source:
    if source.count(old) != 1:
        raise SystemExit("unexpected upstream static-link configuration")
    p.write_text(source.replace(old, new), encoding="utf-8")
elif new not in source:
    raise SystemExit("unrecognized upstream static-link configuration")
PY

cmake -S "$ROOT/source/xolotl" -B "$ROOT/build-h200" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_CUDA_ARCHITECTURES=90 \
    -DCMAKE_CUDA_COMPILER=/usr/local/cuda/bin/nvcc \
    -DSTRATA_ENABLE_CUDA=ON \
    -DSTRATA_GGML_CUDA=ON \
    -DSTRATA_GGML_STATIC=OFF \
    -DSTRATA_MMQ_KQUANTS=ON \
    -DSTRATA_GGML_DIR="$ROOT/source/llama" \
    -DSTRATA_DSV41_FQ_OP=ON \
    -DSTRATA_DSV41_HC_MIX=ON \
    -DSTRATA_BUILD_TESTS=OFF
cmake --build "$ROOT/build-h200" --target ds41_generate -j 32
sha256sum "$ROOT/build-h200/ds41_generate"
printf 'H200_ENGINE_BUILD_COMPLETE\n'
