# Build the TUI and its ACP companion into the same target directory.
build:
    cargo build --locked

# Start the terminal, forwarding arguments without losing shell quoting.
[positional-arguments]
code *args: build
    #!/usr/bin/env python3
    import json
    import os
    from pathlib import Path
    import subprocess
    import sys
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"]))
    executable = Path(metadata["target_directory"]) / "debug" / ("aster.exe" if os.name == "nt" else "aster")
    os.execv(str(executable), [str(executable), *sys.argv[1:]])

# Build the two companion executables shared by Studio from this workspace.
build-companions:
    cargo build --locked -p a3s-code-acp -p a3s-office-cli

# Legacy Work source build; current AsterDream ships from apps/studio.
build-work:
    cargo build --locked -p a3s-code-acp -p a3s-office-cli
    cargo build --locked -p aster-dream

# Build Studio with its ACP and Office companions in the shared target directory.
build-studio $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    #!/usr/bin/env bash
    set -euo pipefail
    feature="$(python3 apps/studio/scripts/studio_product.py "$slug" feature)"
    export A3S_TENANT_SLUG="$slug"
    export TAURI_CONFIG="$(python3 apps/studio/scripts/studio_product.py "$slug" tauri-config)"
    cargo build --locked -p a3s-code-acp -p a3s-office-cli
    cargo build --locked -p a3s-studio --no-default-features --features "$feature"

# Start Studio with renderer hot reload on 127.0.0.1:4318.
# Default tenant is aster. `just studio toyou` starts the original ToWork product.
studio $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    #!/usr/bin/env bash
    set -euo pipefail
    if ! [[ "${slug}" =~ ^[a-z][a-z0-9-]{0,31}$ ]]; then
      echo "invalid tenant slug: ${slug}" >&2
      exit 1
    fi
    bun scripts/check-tenant.ts "${slug}" >/dev/null
    export A3S_TENANT_SLUG="${slug}"
    export STUDIO_RENDERER_DEV="${STUDIO_RENDERER_DEV:-1}"
    cd apps/studio
    exec scripts/studio-shell.sh

[macos]
build-studio-release $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    #!/usr/bin/env bash
    set -euo pipefail
    export A3S_TENANT_SLUG="$slug"
    cd apps/studio
    exec pnpm package:host

# Legacy Work package only; current AsterDream uses build-studio-release.
[macos]
build-work-release:
    cd apps/work && pnpm package:host

# Documented desktop host build. Same result as the shared host recipe.
build-desktop $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    just build-studio {{slug}}

[macos]
build-desktop-release $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    just build-studio-release {{slug}}

# Start the current AsterDream Studio host with renderer hot reload.
dev: studio

# Compatibility entry point for AsterDream; Studio owns this tenant's launch lifecycle.
work: studio

# Middleware plus the host processes that exist in this tree.
# Default tenant is a3s. `just up toyou` selects another public tenant.
up $slug='a3s':
    #!/usr/bin/env bash
    set -euo pipefail
    if ! [[ "${slug}" =~ ^[a-z][a-z0-9-]{0,31}$ ]]; then
      echo "invalid tenant slug: ${slug}" >&2
      exit 1
    fi
    bun scripts/check-tenant.ts "${slug}" >/dev/null
    if ! command -v tilt >/dev/null 2>&1; then
      echo "tilt not found — install from https://docs.tilt.dev/install.html" >&2
      echo "  macOS: brew install tilt" >&2
      exit 1
    fi
    export A3S_TENANT_SLUG="${slug}"
    tilt up

# Non-interactive spine boot. Same tenant parameter as `just up`.
up-ci $slug='a3s':
    #!/usr/bin/env bash
    set -euo pipefail
    if ! [[ "${slug}" =~ ^[a-z][a-z0-9-]{0,31}$ ]]; then
      echo "invalid tenant slug: ${slug}" >&2
      exit 1
    fi
    bun scripts/check-tenant.ts "${slug}" >/dev/null
    if ! command -v tilt >/dev/null 2>&1; then
      echo "tilt not found — install from https://docs.tilt.dev/install.html" >&2
      exit 1
    fi
    export A3S_TENANT_SLUG="${slug}"
    tilt ci

# Stop everything started by `just up`. Leaves Postgres volumes in place.
down:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v tilt >/dev/null 2>&1; then
      tilt down || true
    fi
    bash scripts/stop-web.sh || true
    bash scripts/stop-billing.sh || true
    bash scripts/stop-auth.sh || true
    bash scripts/stop-cloud-containers.sh || true
    bash scripts/stop-search.sh || true
    bash scripts/stop-config.sh || true
    docker compose down || true

# Middleware only (Postgres, Redis, RustFS, Iggy, etcd) from the OS compose.
middleware-up:
    docker compose up -d

# Run the layered backend test suites (hermetic members in parallel,
# Postgres/S3 campaigns serially). Requires the middleware for campaigns.
backend-test:
    cargo test --locked -p a3s-gateway -p a3s-gateway-runtime -p a3s-runner -p a3s-vec
