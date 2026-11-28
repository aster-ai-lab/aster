#!/usr/bin/env python3
"""Compare DeepSeek Messages deferral with full Messages using unchanged ACP graders."""

import argparse
import asyncio
import json
import os
from pathlib import Path
import sys

evidence = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(evidence / "2026-11-27-model-selected-tool-catalog"))
sys.path.insert(0, str(evidence / "2026-11-28-default-catalog-capability-audit"))
sys.path.insert(0, str(evidence / "2026-11-27-six-turn-dsh-comparison"))
import full_catalog
import measure_form
import pilot
import revisions


def full_card(tool):
    return (
        tool["name"] + pilot.annotated_signature(tool["input_schema"])
        + " — " + tool["description"]
    )


def parameter_card(tool):
    purpose = tool["description"].split("\n", 1)[0][:150]
    return tool["name"] + pilot.annotated_signature(tool["input_schema"]) + " — " + purpose


def keep_full(tool):
    # Preserve native parsing hints for composite inputs and descriptions that
    # do not fit a first-line card. The same rule applies to mounted tools.
    description = tool["description"]
    if description != description.split("\n", 1)[0][:150]:
        return True
    schema = tool["input_schema"]
    if not isinstance(schema, dict) or not isinstance(schema.get("properties", {}), dict):
        return True
    if any(key in schema for key in ("oneOf", "anyOf", "allOf", "$ref")):
        return True
    return any(
        field.get("properties") or (
            isinstance(field.get("items"), dict) and field["items"].get("properties")
        )
        for field in schema.get("properties", {}).values()
        if isinstance(field, dict)
    )


def relay_factory(summary):
    def build(credential, usage_path, *, max_tokens=1024, max_requests=None, **_chat_options):
        return pilot.PilotRelay(
            credential, usage_path, max_tokens=max_tokens, max_requests=max_requests,
            keep_full=keep_full if summary == "composite-eager" else None,
        )
    return build


async def forms(args, build_relay):
    args.out.mkdir()
    args.fixture = "contract"
    relay = build_relay(os.environ.pop("DEEPSEEK_API_KEY"), args.out / "provider-usage.jsonl", max_requests=12)
    rows = []
    try:
        for index, order in enumerate((("release", "candidate"), ("candidate", "release")), 1):
            for arm in order:
                row = await measure_form.run_trial(f"{index}-{arm}", args.acp, args, relay)
                rows.append({"arm": arm, **row})
                (args.out / "trials.json").write_text(
                    json.dumps(rows, ensure_ascii=False, indent=2) + "\n"
                )
                print(row["label"], row["passed"], row["tokens"], flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenario", required=True, choices=(
        "short", "long", "form", *full_catalog.SCENARIOS, "program_approved", "verified_edit_exact_count",
    ))
    parser.add_argument("--summary", choices=(
        "first-line", "parameter-notes", "annotated", "composite-eager",
    ), default="annotated")
    parser.add_argument("--order", choices=("forward", "reverse"), default="forward")
    for name in ("acp", "dsh", "embedding", "out"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=name != "dsh")
    args = parser.parse_args()
    if args.summary == "annotated":
        pilot.card = full_card
    elif args.summary in ("parameter-notes", "composite-eager"):
        pilot.card = parameter_card
    build_relay = relay_factory(args.summary)
    if args.scenario == "form":
        asyncio.run(forms(args, build_relay))
    elif args.scenario in ("short", "long"):
        if args.dsh is None:
            parser.error("--dsh is required for the three-way comparison")
        pilot.base.Relay = build_relay
        if args.scenario == "short":
            args.candidate = args.release = args.acp
            args.order = ["release,candidate,dsh", "dsh,candidate,release"]
            asyncio.run(pilot.base.evaluate(args))
        else:
            revisions.candidate_with_catalog = lambda _directory, binary: binary
            args.reference_acp = args.acp
            args.reference_deferred = False
            args.fixture = evidence / "2026-11-27-six-turn-dsh-comparison" / "fixture.json"
            args.order = (
                ["candidate", "release", "dsh"] if args.order == "forward"
                else ["dsh", "release", "candidate"]
            )
            asyncio.run(revisions.compare(args))
    else:
        full_catalog.PilotRelay = build_relay
        args.native_catalog = args.default_catalog = False
        args.annotate_signatures = False
        args.native_required = False
        args.ungated = True
        args.disk_content_attestation = args.scenario == "verified_edit_exact_count"
        args.reference_acp = args.acp
        asyncio.run(full_catalog.evaluate(args))


if __name__ == "__main__":
    main()
