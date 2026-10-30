#!/usr/bin/env bash
# Assemble an ephemeral multi-package signed registry for local gates.
#
# Writes under OUT_ROOT (required):
#   packages/mock-alpha   (skill)
#   packages/mock-beta    (skill)
#   packages/mock-echo    (tool + permission ceiling)
#   packages/mock-compose (tool + mcp + okf + skill + ui, cross-requires)
#   admissions.acl
#   keys/
#   registry/            (signed publication)
#
# Prints JSON: {"rootSha256":"sha256:...","packages":[...],"registry":"..."}
set -euo pipefail

OUT_ROOT="${1:-}"
if [[ -z "${OUT_ROOT}" ]]; then
  echo "usage: $0 <out-root>" >&2
  exit 2
fi

resolve_tools() {
  if [[ -n "${A3S_USE_REGISTRY_TOOLS_BIN:-}" && -x "${A3S_USE_REGISTRY_TOOLS_BIN}" ]]; then
    printf '%s\n' "${A3S_USE_REGISTRY_TOOLS_BIN}"
    return 0
  fi
  local candidate repo
  repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  for candidate in \
    "${repo}/crates/use/target/release/a3s-use-registry-tools" \
    "${repo}/crates/use/target/debug/a3s-use-registry-tools"; do
    if [[ -x "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  echo "error: set A3S_USE_REGISTRY_TOOLS_BIN or build a3s-use-registry-tools" >&2
  exit 1
}

TOOLS="$(resolve_tools)"
mkdir -p "${OUT_ROOT}/packages/mock-alpha/skills/alpha"
mkdir -p "${OUT_ROOT}/packages/mock-beta/skills/beta"
mkdir -p "${OUT_ROOT}/packages/mock-echo/tools"

cat >"${OUT_ROOT}/packages/mock-alpha/a3s-use-extension.acl" <<'EOF'
extension "a3s/mock-alpha" {
  schema_version = 3
  version        = "0.1.0"
  route          = "mock-alpha"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url      = "https://github.com/A3S-Lab/Use-Registry"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  skill "alpha" {
    path          = "skills/alpha/SKILL.md"
    requires_tool = []
    requires_mcp  = []
    optional      = false
  }
}
EOF
cat >"${OUT_ROOT}/packages/mock-alpha/README.md" <<'EOF'
# mock-alpha

Ephemeral skill fixture for local Registry gates.
EOF
cat >"${OUT_ROOT}/packages/mock-alpha/skills/alpha/SKILL.md" <<'EOF'
---
name: mock-alpha
description: Mock skill package alpha for Registry transport gates
---
# Alpha
EOF

cat >"${OUT_ROOT}/packages/mock-beta/a3s-use-extension.acl" <<'EOF'
extension "a3s/mock-beta" {
  schema_version = 3
  version        = "0.1.0"
  route          = "mock-beta"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read"]

  repository {
    url      = "https://github.com/A3S-Lab/Use-Registry"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  skill "beta" {
    path          = "skills/beta/SKILL.md"
    requires_tool = []
    requires_mcp  = []
    optional      = false
  }
}
EOF
cat >"${OUT_ROOT}/packages/mock-beta/README.md" <<'EOF'
# mock-beta

Second skill fixture used to prove multi-package catalog resolution.
EOF
cat >"${OUT_ROOT}/packages/mock-beta/skills/beta/SKILL.md" <<'EOF'
---
name: mock-beta
description: Mock skill package beta for Registry transport gates
---
# Beta
EOF

cat >"${OUT_ROOT}/packages/mock-echo/a3s-use-extension.acl" <<'EOF'
extension "a3s/mock-echo" {
  schema_version = 3
  version        = "0.1.0"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read", "execute"]

  repository {
    url      = "https://github.com/A3S-Lab/Use-Registry"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  tool "echo" {
    workload    = "task"
    interface   = "cli"
    executable  = "tools/echo"
    command     = "mock-echo"
    json_output = true
    interactive = false
    timeout_ms  = 120000
    activation  = "lazy"
    optional    = false
  }
}
EOF
cat >"${OUT_ROOT}/packages/mock-echo/README.md" <<'EOF'
# mock-echo

Executable tool fixture with a permission ceiling.
EOF
cat >"${OUT_ROOT}/packages/mock-echo/tools/echo" <<'EOF'
#!/bin/sh
exec cat
EOF
chmod 755 "${OUT_ROOT}/packages/mock-echo/tools/echo"

# --- mock-compose: Tool + MCP + OKF + Skill + UI that must cooperate ----------
mkdir -p "${OUT_ROOT}/packages/mock-compose/"{tools,mcp,okf/domain/concepts,skills/compose,ui/compose}

cat >"${OUT_ROOT}/packages/mock-compose/a3s-use-extension.acl" <<'EOF'
extension "a3s/mock-compose" {
  schema_version = 3
  version        = "0.1.0"
  route          = "mock-compose"
  requires_use   = ">=0.3.0, <0.4.0"
  actions        = ["read", "execute"]

  repository {
    url      = "https://github.com/A3S-Lab/Use-Registry"
    revision = "0123456789abcdef0123456789abcdef01234567"
  }

  tool "echo" {
    workload    = "task"
    interface   = "cli"
    executable  = "tools/echo"
    command     = "mock-compose-echo"
    json_output = true
    interactive = false
    timeout_ms  = 30000
    activation  = "lazy"
    optional    = false
  }

  mcp "context" {
    transport  = "stdio"
    executable = "mcp/context"
    args       = ["--stdio"]
    activation = "lazy"
    optional   = false
  }

  okf "domain" {
    format_version         = "0.2"
    root                   = "okf/domain"
    content_digest         = "sha256:355b6f00153630b082e60a0f7e0b67fbbb74b2a29067bca481f7eefecbb86c7a"
    concept_count          = 1
    file_count             = 2
    expanded_bytes         = 427
    max_files              = 64
    max_concepts           = 32
    max_expanded_bytes     = 1048576
    max_document_bytes     = 262144
    max_links_per_document = 128
    optional               = false
  }

  skill "compose" {
    path          = "skills/compose/SKILL.md"
    requires_tool = ["echo"]
    requires_mcp  = ["context"]
    requires_okf  = ["domain"]
    optional      = false
  }

  ui "compose" {
    entry     = "ui/compose/index.html"
    styles    = ["ui/compose/index.css"]
    scripts   = ["ui/compose/index.js"]
    skill     = "compose"
    bind_tool = ["echo"]
    bind_mcp  = ["context"]
    optional  = false
  }
}
EOF

cat >"${OUT_ROOT}/packages/mock-compose/README.md" <<'EOF'
# mock-compose

Ephemeral package that wires Tool, MCP, OKF, Skill, and UI together.
EOF
cat >"${OUT_ROOT}/packages/mock-compose/tools/echo" <<'EOF'
#!/bin/sh
exec cat
EOF
chmod 755 "${OUT_ROOT}/packages/mock-compose/tools/echo"
cat >"${OUT_ROOT}/packages/mock-compose/mcp/context" <<'EOF'
#!/bin/sh
# Fixture MCP stdio binary — never executed by assemble/verify/plan gates.
exit 0
EOF
chmod 755 "${OUT_ROOT}/packages/mock-compose/mcp/context"
# OKF bundle bytes must match the pinned content_digest above (package fixture).
cat >"${OUT_ROOT}/packages/mock-compose/okf/domain/index.md" <<'EOF'
---
okf_version: "0.2"
---

# Package knowledge

- [Lifecycle](concepts/lifecycle.md) - One generation owns every plugin surface.
EOF
cat >"${OUT_ROOT}/packages/mock-compose/okf/domain/concepts/lifecycle.md" <<'EOF'
---
type: Project-Specific Decision
title: Atomic package lifecycle
description: Tool, MCP, OKF, Flow, Skill, and UI contributions activate as one package generation.
status: stable
---

# Decision

Publish package capabilities only after every required contribution is ready.
EOF
cat >"${OUT_ROOT}/packages/mock-compose/skills/compose/SKILL.md" <<'EOF'
---
name: mock-compose
description: Skill that requires Tool, MCP, and OKF surfaces in the same package
---
# Compose
EOF
cat >"${OUT_ROOT}/packages/mock-compose/ui/compose/index.html" <<'EOF'
<!doctype html><title>mock-compose</title><body>compose</body>
EOF
cat >"${OUT_ROOT}/packages/mock-compose/ui/compose/index.css" <<'EOF'
body { font-family: sans-serif; }
EOF
cat >"${OUT_ROOT}/packages/mock-compose/ui/compose/index.js" <<'EOF'
console.log("mock-compose");
EOF

cat >"${OUT_ROOT}/ceiling.json" <<'EOF'
{
  "schema": "a3s.use.plugin-permissions.v1",
  "surfaces": [
    {
      "surface": {"kind": "tool", "id": "echo"},
      "nativeExecution": true,
      "childProcess": false,
      "filesystem": [],
      "networkEgress": [],
      "privateService": false,
      "secrets": [],
      "resources": {
        "cpuMillis": 1000,
        "memoryBytes": 268435456,
        "pids": 64,
        "ephemeralStorageBytes": 1073741824,
        "taskTimeoutMs": 120000,
        "maxStdoutBytes": 1048576,
        "maxStderrBytes": 1048576
      },
      "uiHttp": []
    }
  ]
}
EOF

cat >"${OUT_ROOT}/compose-ceiling.json" <<'EOF'
{
  "schema": "a3s.use.plugin-permissions.v1",
  "surfaces": [
    {
      "surface": {"kind": "mcp", "id": "context"},
      "nativeExecution": true,
      "childProcess": false,
      "filesystem": [],
      "networkEgress": [],
      "privateService": false,
      "secrets": [],
      "resources": {
        "cpuMillis": 500,
        "memoryBytes": 268435456,
        "pids": 32,
        "ephemeralStorageBytes": 536870912
      },
      "uiHttp": []
    },
    {
      "surface": {"kind": "tool", "id": "echo"},
      "nativeExecution": true,
      "childProcess": false,
      "filesystem": [],
      "networkEgress": [],
      "privateService": false,
      "secrets": [],
      "resources": {
        "cpuMillis": 1000,
        "memoryBytes": 268435456,
        "pids": 64,
        "ephemeralStorageBytes": 1073741824,
        "taskTimeoutMs": 30000,
        "maxStdoutBytes": 1048576,
        "maxStderrBytes": 1048576
      },
      "uiHttp": []
    }
  ]
}
EOF

cat >"${OUT_ROOT}/admissions.acl" <<'EOF'
admission "a3s/mock-alpha" {
  package_directory = "packages/mock-alpha"
  channel           = "stable"
  target            = "any"
  display_name      = "Mock Alpha"
  description       = "Ephemeral skill A for multi-package Registry gates."
  license           = "Apache-2.0"
  keywords          = ["mock", "alpha"]
  categories        = ["testing"]
}

admission "a3s/mock-beta" {
  package_directory = "packages/mock-beta"
  channel           = "stable"
  target            = "any"
  display_name      = "Mock Beta"
  description       = "Ephemeral skill B for multi-package Registry gates."
  license           = "Apache-2.0"
  keywords          = ["mock", "beta"]
  categories        = ["testing"]
}

admission "a3s/mock-echo" {
  package_directory  = "packages/mock-echo"
  channel            = "stable"
  target             = "any"
  display_name       = "Mock Echo"
  description        = "Ephemeral tool package for multi-package Registry gates."
  license            = "Apache-2.0"
  keywords           = ["mock", "echo"]
  categories         = ["testing"]
  permission_ceiling = "ceiling.json"
}

admission "a3s/mock-compose" {
  package_directory  = "packages/mock-compose"
  channel            = "stable"
  target             = "any"
  display_name       = "Mock Compose"
  description        = "Tool+MCP+OKF+Skill+UI package that exercises cross-surface requires."
  license            = "Apache-2.0"
  keywords           = ["mock", "compose", "okf", "mcp", "ui"]
  categories         = ["testing"]
  permission_ceiling = "compose-ceiling.json"
}
EOF

KEYS="${OUT_ROOT}/keys"
REGISTRY="${OUT_ROOT}/registry"
rm -rf "${KEYS}" "${REGISTRY}"
"${TOOLS}" keygen --keys-dir "${KEYS}" >/dev/null
ASSEMBLE_OUT="$("${TOOLS}" assemble \
  --keys-dir "${KEYS}" \
  --admissions "${OUT_ROOT}/admissions.acl" \
  --out-root "${REGISTRY}")"
ROOT_SHA="$(printf '%s\n' "${ASSEMBLE_OUT}" | python3 -c 'import json,sys; print(json.load(sys.stdin)["rootSha256"])')"
"${TOOLS}" verify --registry "${REGISTRY}" --expected-root-sha256 "${ROOT_SHA}" >/dev/null

export OUT_ROOT ROOT_SHA
python3 -c '
import json, os
root = os.environ["OUT_ROOT"]
print(json.dumps({
  "rootSha256": os.environ["ROOT_SHA"],
  "registry": os.path.join(root, "registry"),
  "packages": [
    "a3s/mock-alpha",
    "a3s/mock-beta",
    "a3s/mock-echo",
    "a3s/mock-compose",
  ],
}))
'
