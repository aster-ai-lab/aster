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

# Build Work and the two companion executables from this workspace.
build-work:
    cargo build --locked -p a3s-code-acp -p a3s-office-cli
    cargo build --locked -p aster-dream

# Build Studio with its ACP and Office companions in the shared target directory.
build-studio:
    cargo build --locked -p a3s-code-acp -p a3s-office-cli
    cargo build --locked -p a3s-studio

# Start Studio with renderer hot reload on 127.0.0.1:4318.
# Default tenant is toyou. `just studio a3s` selects another public tenant.
studio $slug='':
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -z "${slug}" ]]; then
      slug="${A3S_TENANT_SLUG:-toyou}"
    fi
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
build-studio-release:
    cd apps/studio && pnpm package:host

[macos]
build-work-release:
    cd apps/work && pnpm package:host

# Start AsterDream with renderer hot reload on 127.0.0.1:4320.
dev:
    cd apps/work && just dev

# Rebuild Work, then start that debug AsterDream in place of an older primary instance.
[macos]
work: build-work
    #!/usr/bin/env python3
    import ctypes
    import json
    import os
    from pathlib import Path
    import signal
    import subprocess
    import time

    names = {"aster-dream", "aster-dream.exe", "AsterDream"}
    # Primary instance only. Other tenants under dev.a3s.studio stay up.
    support = Path(os.environ.get(
        "ASTER_WORK_SUPPORT",
        Path.home() / "Library/Application Support/dev.a3s.studio/aster",
    ))

    def alive(pid):
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return False
        except PermissionError:
            return True
        return True

    def product_pids(areas):
        lib = ctypes.CDLL("/usr/lib/libproc.dylib")
        lib.proc_pidpath.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
        lib.proc_pidpath.restype = ctypes.c_int
        buf = ctypes.create_string_buffer(4096)
        listing = subprocess.check_output(["/bin/ps", "-ax", "-o", "pid="], text=True)
        victims = []
        for token in listing.split():
            if not token.isdigit():
                continue
            pid = int(token)
            if pid == os.getpid():
                continue
            if lib.proc_pidpath(pid, buf, len(buf)) <= 0:
                continue
            image = Path(os.fsdecode(buf.value))
            if image.name not in names:
                continue
            try:
                resolved = image.resolve()
            except OSError:
                resolved = image
            if any(area == resolved or area in resolved.parents for area in areas):
                victims.append(pid)
        return victims

    def stop_replaced(areas):
        try:
            victims = product_pids(areas)
        except OSError:
            return
        for pid in victims:
            try:
                os.kill(pid, signal.SIGTERM)
            except OSError:
                pass
        deadline = time.time() + 2
        while time.time() < deadline and any(alive(pid) for pid in victims):
            time.sleep(0.05)
        for pid in victims:
            if alive(pid):
                try:
                    os.kill(pid, signal.SIGKILL)
                except OSError:
                    pass
        time.sleep(0.1)

    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"],
    ))
    # cargo leaves an unchanged binary's mtime behind, so a newer .app must not win.
    executable = (Path(metadata["target_directory"]) / "debug" / "aster-dream").resolve()
    if not executable.is_file():
        raise SystemExit(f"Build did not produce {executable}")
    if os.environ.get("ASTER_WORK_DRY_RUN") == "1":
        text = str(executable)
        log = os.environ.get("ASTER_OPEN_LOG")
        if log:
            Path(log).write_text(text + "\n")
        else:
            print(text, flush=True)
        raise SystemExit(0)

    roots = [Path(metadata["target_directory"]), Path("apps/work/src-tauri/target")]
    areas = []
    for root in [*roots, support]:
        try:
            areas.append(root.resolve())
        except OSError:
            continue
    # The running build asks the next launch to show its old window.
    stop_replaced(areas)
    intermediate = os.fork()
    if intermediate > 0:
        os.waitpid(intermediate, 0)
        raise SystemExit(0)
    os.setsid()
    if os.fork() > 0:
        os._exit(0)
    devnull = os.open(os.devnull, os.O_RDWR)
    for fd in (0, 1, 2):
        os.dup2(devnull, fd)
    os.execv(str(executable), [str(executable)])

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
