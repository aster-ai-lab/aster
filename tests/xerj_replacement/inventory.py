"""Generate qualification inventory from the pinned published MCP schema."""

import argparse
import hashlib
import json
import sys
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[2]
CONTRACTS = ROOT / "specs/033-xerj-replacement-contracts/contracts"
REFERENCE = ROOT / "docs/evidence/code/2026-11-01-xerj-replacement/references/landing/docs/agents/schemas/mcp-tools.json"
REFERENCE_SHA = "e8f2de3c67eeb3d795b9cc7e4f5cac52cdc356afa3eb61f839ebd4e387691ba0"
REFERENCE_COMMIT = "682d7ce072c644239cfa13ca1a9ca9a0e45d794b"
OWNERS = {"core", "host", "adapter"}
PRESENCE = {"declared", "hub_signing_key"}
HUB_TOOLS = {"xerj_hub_find", "xerj_hub_def", "xerj_hub_ask"}


def read_json(path, limit):
    with path.open("rb") as source:
        raw = source.read(limit + 1)
    if len(raw) > limit:
        raise ValueError("inventory input exceeds its byte limit")
    return raw, json.loads(raw.decode("utf-8"))


def generate(reference, decisions):
    tools = reference["tools"]
    if not isinstance(tools, list) or len(tools) != 22:
        raise ValueError("pinned MCP maximum must contain exactly 22 tools")
    if set(decisions) != {"schema_version", "tools"} or decisions["schema_version"] != 1:
        raise ValueError("unknown owner-decision schema")
    by_name = {}
    for decision in decisions["tools"]:
        if set(decision) != {"name", "owner", "paths", "presence", "required_evidence"}:
            raise ValueError("unknown owner-decision field")
        name = decision["name"]
        if name in by_name or decision["owner"] not in OWNERS or decision["presence"] not in PRESENCE:
            raise ValueError("duplicate tool or unknown owner/presence")
        if not decision["paths"] or not decision["required_evidence"]:
            raise ValueError("owner decision needs source paths and explicit evidence")
        for path in decision["paths"]:
            relative = PurePosixPath(path)
            if (relative.is_absolute() or relative.as_posix() != path or ".." in relative.parts
                    or "\\" in path or ":" in path or not (ROOT / path).exists()):
                raise ValueError("owner source path is absent or not safely relative")
        if any(not isinstance(item, str) or not item.strip() for item in decision["required_evidence"]):
            raise ValueError("owner evidence must be nonempty text")
        by_name[name] = decision
    seen = set()
    rows = []
    for index, tool in enumerate(tools):
        name = tool["name"]
        if not isinstance(name, str) or not name or name in seen or name not in by_name:
            raise ValueError("duplicate, unknown or unmapped reference tool")
        seen.add(name)
        schema = tool["inputSchema"]
        properties = schema.get("properties", {})
        required = schema.get("required", [])
        if (schema["type"] != "object" or not isinstance(properties, dict)
                or not isinstance(required, list) or len(required) != len(set(required))
                or not all(isinstance(field, str) and field in properties for field in required)):
            raise ValueError("invalid reference input schema")
        decision = by_name[name]
        if (decision["presence"] == "hub_signing_key") != (name in HUB_TOOLS):
            raise ValueError("reference Hub admission was changed")
        canonical = json.dumps(tool, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        rows.append({
            "name": name, "source_pointer": f"/tools/{index}",
            "tool_schema_sha256": hashlib.sha256(canonical).hexdigest(),
            "properties": list(properties), "required": required,
            "candidate_owner": decision["owner"], "candidate_paths": decision["paths"],
            "reference_presence": decision["presence"],
            "required_evidence": decision["required_evidence"], "qualification_status": "NOT_RUN",
        })
    if seen != set(by_name):
        raise ValueError("owner decisions include tools absent from the reference")
    return {
        "schema_version": 1,
        "reference": {"tag": "v1.0.0-rc.100", "commit": REFERENCE_COMMIT,
                      "source": REFERENCE.relative_to(ROOT).as_posix(), "sha256": REFERENCE_SHA},
        "coverage": {"surface": "published_mcp_maximum", "tool_count": len(rows),
                     "complete_replacement_denominator": False, "live_reference_verified": False,
                     "owner_binding": "candidate_only", "qualification_status": "NOT_RUN"},
        "tools": rows,
    }


def build_inventory(reference_path=REFERENCE, decisions_path=CONTRACTS / "mcp-owners.json"):
    raw, reference = read_json(reference_path, 1024 * 1024)
    if hashlib.sha256(raw).hexdigest() != REFERENCE_SHA:
        raise ValueError("pinned reference schema digest mismatch")
    _, decisions = read_json(decisions_path, 128 * 1024)
    return generate(reference, decisions)


def serialized(inventory):
    return (json.dumps(inventory, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="refuse drift without rewriting the generated inventory")
    args = parser.parse_args()
    try:
        raw = serialized(build_inventory())
        destination = CONTRACTS / "mcp-inventory.json"
        if args.check:
            with destination.open("rb") as stored:
                if stored.read(len(raw) + 1) != raw:
                    raise ValueError("generated inventory drift; regenerate from pinned source and decisions")
        else:
            destination.write_bytes(raw)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"inventory failed: {error}", file=sys.stderr)
        return 1
    print("MCP inventory: 22 declared tools; runtime qualification NOT_RUN")
    return 0


if __name__ == "__main__":
    sys.exit(main())
