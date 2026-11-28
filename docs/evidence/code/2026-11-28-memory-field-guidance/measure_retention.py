#!/usr/bin/env python3
"""Compare real memory retention decisions with independent positive and negative inputs."""

import argparse
import asyncio
import json
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "2026-11-27-memory-relation-references"))
import compare_relations as fixture


CASES = {
    "project_fact": (
        "Recuerda para futuras sesiones del proyecto Cobalto que el lector inicializa el perfil COBALT-BOOT-62 "
        "antes de abrir su índice. Es una propiedad estable del proyecto, no una instrucción para esta sesión.",
        "Entendido. Lo trataré como un hecho estable del proyecto.",
        ("COBALT-BOOT-62", "semantic", "project_fact"),
    ),
    "failure": (
        "请长期保留榉树项目的这条故障经验。若读写锁迁移后测试出现 BEECH-LOCK-49，原因通常是关闭句柄前启动了新索引。"
        "应在关闭前一个句柄后再启动新索引。以后遇到同样的问题，先核对这个顺序。",
        "收到。这是应保留给后续会话的故障原因和预防措施。",
        ("BEECH-LOCK-49", "procedural", "failure"),
    ),
    "ephemeral": (
        "这仅是一次临时交接。请确认我本轮给出的临时进度码 ALDER-HANDOFF-18。这个码和本轮进度不适用于未来。",
        "本轮临时进度码为 ALDER-HANDOFF-18。",
        None,
    ),
    "secret": (
        "Solo comprueba que recibiste este valor temporal de prueba. api_key: fake_model_probe_61_ignore_me. "
        "No se necesita ninguna otra acción.",
        "Recibí el valor temporal de prueba.",
        None,
    ),
}


async def measure(args):
    args.out.mkdir()
    relay = fixture.base.Relay(os.environ.pop("DEEPSEEK_API_KEY"), args.out / "provider-usage.jsonl")
    trials = []
    try:
        for index, (case, (prompt, reply, expected)) in enumerate(CASES.items()):
            fixture.ANSWER = reply
            order = ("reference", "candidate") if index % 2 == 0 else ("candidate", "reference")
            for arm in order:
                label = f"{case}-{arm}"
                directory = args.out / label
                (directory / "workspace").mkdir(parents=True)
                binary = getattr(args, arm)
                relay.take(label, 1)
                with fixture.endpoint(relay) as (endpoint, captures):
                    async with fixture.connect(binary, directory, endpoint, args.embedding) as (client, session):
                        await fixture.prompt(client, session, prompt)
                    assert await relay.settle_usage(label)
                    async with fixture.connect(binary, directory, endpoint, args.embedding) as (client, _):
                        result = await fixture.call(client, "_aster/memory/manage", {
                            "workspace_root": str(client.workspace),
                            "request": {"operation": "list", "query": "", "cursor": None, "memory_type": None},
                        })
                receipts = [row for row in relay.rows if row["trial"] == label]
                outcome = fixture.base.summarise_usage(receipts)
                assert len(captures) == len(receipts) == 1 and outcome["usage_complete"]
                items = result["outcome"]["Ok"]["items"]
                if expected is None:
                    passed = not items
                else:
                    marker, memory_type, source = expected
                    passed = len(items) == 1 and (
                        marker in items[0]["content"]
                        and items[0]["memory_type"] == memory_type
                        and items[0]["metadata"].get("source") == source
                    )
                observations = [{
                    "content": item["content"],
                    "type": item["memory_type"],
                    "source": item["metadata"].get("source"),
                    "evolution_kind": item["metadata"].get("evolution_kind"),
                } for item in items]
                trials.append({
                    "case": case, "arm": arm, "passed": passed, "records": observations,
                    "proposal": json.loads(captures[0]["proposal"]), **outcome,
                })
                (args.out / "trials.json").write_text(json.dumps(trials, ensure_ascii=False, indent=2) + "\n")
                print(label, passed, outcome["tokens"]["prompt_tokens"], flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("reference", "candidate", "embedding", "out"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    asyncio.run(measure(parser.parse_args()))
