# Build the TUI and its ACP companion into the same target directory.
[unix]
build:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "${RUSTC_WRAPPER+x}" != x ] && command -v sccache >/dev/null 2>&1; then export RUSTC_WRAPPER=sccache; fi
    cargo build --locked
    if [[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]]; then python3 scripts/aster_sandbox.py stage; fi

# Windows has no sh on PATH. powershell -File keeps the cargo exit code.
# An already-open terminal does not see a User PATH change, so point PROTOC
# at the installed compiler when `protoc` is not already resolvable.
[windows]
build:
    #!powershell.exe -File
    $knownProtoc = Join-Path $env:LOCALAPPDATA "protoc\bin\protoc.exe"
    $resolved = Get-Command protoc -ErrorAction SilentlyContinue
    if ($resolved) {
        $env:PROTOC = $resolved.Source
    } elseif (Test-Path -LiteralPath $knownProtoc) {
        $env:PROTOC = $knownProtoc
        $env:PATH = "$(Split-Path -Parent $knownProtoc);$env:PATH"
    }
    if (-not (Test-Path Env:RUSTC_WRAPPER) -and (Get-Command sccache -ErrorAction SilentlyContinue)) { $env:RUSTC_WRAPPER = 'sccache' }
    cargo build --locked
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Start the terminal, forwarding arguments without losing shell quoting.
[unix]
[positional-arguments]
code *args: build
    #!/usr/bin/env python3
    import json
    import os
    import platform
    from pathlib import Path
    import subprocess
    import sys
    # A workspace .a3s/config.acl is untrusted unless the host names it.
    # Opt into that local file when the user has not already set A3S_CONFIG.
    if not os.environ.get("A3S_CONFIG"):
        acl = Path(r"{{justfile_directory()}}") / ".a3s" / "config.acl"
        if acl.is_file():
            os.environ["A3S_CONFIG"] = str(acl.resolve())
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"]))
    if sys.platform == "darwin" and platform.machine() in ("arm64", "aarch64") and not os.environ.get("ASTER_SANDBOX_DIR"):
        os.environ["ASTER_SANDBOX_DIR"] = subprocess.check_output([sys.executable, str(Path(r"{{justfile_directory()}}") / "scripts/aster_sandbox.py"), "cache", "--target-dir", metadata["target_directory"]], text=True).strip()
    executable = Path(metadata["target_directory"]) / "debug" / ("aster.exe" if os.name == "nt" else "aster")
    os.execv(str(executable), [str(executable), *sys.argv[1:]])

# powershell.exe -File is one shebang argument, so spaced arguments stay intact.
[windows]
[positional-arguments]
code *args: build
    #!powershell.exe -File
    $ErrorActionPreference = 'Stop'
    $metadataJson = & cargo metadata --locked --offline --no-deps --format-version 1 | Out-String
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $metadata = $metadataJson | ConvertFrom-Json
    $executable = Join-Path $metadata.target_directory 'debug\aster.exe'
    if (-not (Test-Path -LiteralPath $executable)) {
        [Console]::Error.WriteLine("Build did not produce $executable")
        exit 1
    }
    # A workspace .a3s/config.acl is untrusted unless the host names it.
    # Opt into that local file when the user has not already set A3S_CONFIG.
    if ([string]::IsNullOrWhiteSpace($env:A3S_CONFIG)) {
        $workspaceAcl = Join-Path "{{justfile_directory()}}" ".a3s\config.acl"
        if (Test-Path -LiteralPath $workspaceAcl -PathType Leaf) {
            $env:A3S_CONFIG = (Resolve-Path -LiteralPath $workspaceAcl).Path
        }
    }
    & $executable @args
    if ($null -ne $LASTEXITCODE) { exit $LASTEXITCODE }

# Build the two companion executables shared by Studio from this workspace.
build-companions:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ "${RUSTC_WRAPPER+x}" != x ] && command -v sccache >/dev/null 2>&1; then export RUSTC_WRAPPER=sccache; fi
    cargo build --locked -p a3s-code-acp -p a3s-office-cli --features a3s-code-acp/headless-search

# Build Studio and its companions apart from CLI/check/test artifacts.
build-studio $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    #!/usr/bin/env bash
    set -euo pipefail
    feature="$(python3 apps/studio/scripts/studio_product.py "$slug" feature)"
    export A3S_TENANT_SLUG="$slug"
    TAURI_CONFIG="$(python3 apps/studio/scripts/studio_product.py "$slug" tauri-config)"
    export TAURI_CONFIG
    if [ "${CARGO_TARGET_DIR+x}" != x ]; then
      CARGO_TARGET_DIR="$(python3 apps/studio/scripts/studio_product.py "$slug" dev-target)"
      export CARGO_TARGET_DIR
    fi
    TAURI_CONFIG="$(python3 apps/studio/scripts/studio_product.py "$slug" tauri-config --target-dir "$CARGO_TARGET_DIR")"
    export TAURI_CONFIG STUDIO_COMPANIONS_STAGED=1
    if [ "${RUSTC_WRAPPER+x}" != x ] && command -v sccache >/dev/null 2>&1; then export RUSTC_WRAPPER=sccache; fi
    # One feature graph keeps shared dependencies reusable across both stages.
    python3 apps/studio/scripts/document_assets.py "$CARGO_TARGET_DIR/debug/studio-resources/document"
    build_args=(build --locked -p a3s-studio -p a3s-code-acp -p a3s-office-cli --no-default-features --features "a3s-studio/$feature,a3s-code-acp/default,a3s-code-acp/headless-search,a3s-office-cli/default")
    cargo "${build_args[@]}" --bin a3s-code-acp --bin a3s-office
    python3 apps/studio/scripts/studio_product.py "$slug" stage-companions
    cargo "${build_args[@]}" --bin Studio

# Second window on an already-built app: no cargo, so it never waits on the build-directory lock.
fork $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster') port='4321':
    #!/usr/bin/env bash
    set -euo pipefail
    export A3S_STUDIO_INSTANCE=fork
    export STUDIO_RENDERER_PORT="{{port}}"
    exec just studio "$slug"

# Start Studio with isolated renderer hot reload: Aster on 4318, ToYou on 4319.
# Default tenant is aster. `just studio toyou` starts the original ToWork product.
studio $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    #!/usr/bin/env bash
    set -euo pipefail
    if ! [[ "${slug}" =~ ^[a-z][a-z0-9-]{0,31}$ ]]; then
      echo "invalid tenant slug: ${slug}" >&2
      exit 1
    fi
    # The toolchain (node, cargo, bun) lives under $HOME and is only on PATH
    # after the profile block runs, which a shell opened earlier never saw.
    # Add it here so `just studio` works from any shell, not just a fresh one.
    for dir in "${HOME}/.local/node24/bin" "${HOME}/.cargo/bin" "${HOME}/.bun/bin"; do
      if [ -d "${dir}" ] && [[ ":${PATH}:" != *":${dir}:"* ]]; then
        PATH="${dir}:${PATH}"
      fi
    done
    export PATH
    bun scripts/check-tenant.ts "${slug}" >/dev/null
    if [ "$(id -u)" -eq 0 ]; then
      echo "warning: Studio is starting as root; the fcitx immodule cannot commit composed text for a non-session uid, so IME input will not work. Start it as the session user." >&2
    fi
    export A3S_TENANT_SLUG="${slug}"
    export STUDIO_RENDERER_DEV="${STUDIO_RENDERER_DEV:-1}"
    cd apps/studio
    exec scripts/studio-shell.sh

# Start Studio through the X11/XIM input path.
# The host window is a GTK3 WebKitGTK webview that reaches the input method
# through the fcitx GTK immodule. That module is D-Bus bound to the session
# user, so the composed text only commits for the owner of the session bus;
# run any other way (for example as root) and the webview receives raw
# keystrokes, so typing "nihao" leaves "nihao" in the composer instead of
# "你好". The XIM bridge talks to the same input method over X11 instead, so
# it commits composed text for any client that can open the display.
# Measured on this host: fcitx module -> "nihao", XIM -> "你好".
studio-ime $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    #!/usr/bin/env bash
    set -euo pipefail
    if ! [[ "${slug}" =~ ^[a-z][a-z0-9-]{0,31}$ ]]; then
      echo "invalid tenant slug: ${slug}" >&2
      exit 1
    fi
    python3 apps/studio/scripts/studio_product.py "$slug" feature >/dev/null
    if [ "$(id -u)" -eq 0 ]; then
      echo "warning: Studio is starting as root; the XIM bridge still commits text, but the session input method is owned by another uid, so prefer the session user." >&2
    fi
    export A3S_TENANT_SLUG="${slug}"
    export STUDIO_RENDERER_DEV="${STUDIO_RENDERER_DEV:-1}"
    export GDK_BACKEND=x11
    export GTK_IM_MODULE=xim
    export XMODIFIERS="${XMODIFIERS:-@im=fcitx}"
    cd apps/studio
    exec scripts/studio-shell.sh

[macos]
build-studio-release $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    #!/usr/bin/env bash
    set -euo pipefail
    export A3S_TENANT_SLUG="$slug"
    cd apps/studio
    exec pnpm package:host

# Export every Aster icon from apps/web/public/tenant/aster/logo.png.
app-icons:
    python3 scripts/app-icon-assets.py
    python3 scripts/app-icon.py

# Install this checkout as a desktop application: a launcher entry plus the
# hicolor icon ladder under $XDG_DATA_HOME. Without it a dock has no entry to
# match the window against and paints a generic gear instead of the icon.
# On Kylin the compositor (kylin-wlcom) never reads the user tree; there run
# `just studio-desktop install-system` (with sudo), which writes the same pieces
# into /usr/share where the icon matcher is guaranteed to resolve them.
# `just studio-desktop remove` / `remove-system` undo it.
studio-desktop action='install':
    python3 apps/studio/scripts/studio-desktop.py {{action}}

# Documented desktop host build. Same result as the shared host recipe.
build-desktop $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    just build-studio {{slug}}

[macos]
build-desktop-release $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    just build-studio-release {{slug}}

# Start Aster Studio with renderer hot reload. `just dev:toyou` selects that tenant.
dev $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'):
    STUDIO_RENDERER_DEV=1 just studio "$slug"

# Start Aster Studio for this tenant. `just work:toyou` selects that tenant.
work $slug=env_var_or_default('A3S_TENANT_SLUG', 'aster'): (studio slug)

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
