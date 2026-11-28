#!/usr/bin/env python3
"""Compare real extraction from identical ACP turns and copied synthetic memory."""

import argparse
import asyncio
from contextlib import asynccontextmanager, contextmanager
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import sys
from threading import Thread

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-deepseek-harness-token-comparison'))
import compare as base

OLD = 'The Cedar release workflow runs the container rebuild on Friday at 18:00 UTC.'
SEEDS = (
    OLD,
    'The Cedar release workflow signs provenance attestations with builder identity RIVER-6.',
    'The Cedar release workflow stores final review checklists in the regional audit diary.',
    'The Cedar release workflow sends the stage-specific handbook link to the duty team.',
)
CASES = {
    'replacement': ('请长期记住 Cedar 项目的发布流程已经正式更新。容器重建改为星期三 09:30 UTC。'
                    '这一条替代此前星期五 18:00 UTC 的容器重建安排。签名、清单和通知流程继续照常使用。',
                    '09:30', 'supersedes'),
    'conflict': ('Remember a durable Cedar release discrepancy. The operations handbook says the container '
                 'rebuild runs at 21:45 UTC, which conflicts with the current stored Cedar rebuild schedule. '
                 'The change is unapproved. Retain the current rule and record the unresolved contradiction.',
                 '21:45', 'conflicts_with'),
}
ANSWER = 'I have noted the Cedar release workflow information.'


@contextmanager
def endpoint(relay=None):
    captures = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def respond(self, code, content_type, body):
            self.send_response(code)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_POST(self):
            raw = self.rfile.read(int(self.headers['Content-Length']))
            request = json.loads(raw)
            messages = request['messages']
            extraction = any(m.get('role') == 'system' and m['content'].startswith(
                'You extract durable, reusable memory for a coding agent.') for m in messages)
            text = ANSWER
            if extraction and relay is not None:
                user = messages[-1]['content']
                related = user.partition('\n\nRelated existing memories:\n')[2]
                related = related.partition('\n\nCompressed turn transcript:\n')[0]
                capture = {'user_prompt_bytes': len(user.encode()),
                           'related_rows': [json.loads(row) for row in related.splitlines()]}
                captures.append(capture)
                host, port = relay.server.server_address
                with_connection = HTTPConnection(host, port, timeout=75)
                try:
                    with_connection.request('POST', self.path, raw, {'Content-Type': 'application/json'})
                    response = with_connection.getresponse()
                    body = response.read()
                    if response.status == 200 and not request.get('stream'):
                        capture['proposal'] = json.loads(body)['choices'][0]['message'].get('content')
                    self.respond(response.status, response.getheader('Content-Type'), body)
                finally:
                    with_connection.close()
                return
            if extraction:
                text = json.dumps({'items': [dict(memory_type='procedural', content=content,
                                                 importance=0.92, confidence=0.97, source='workflow',
                                                 scope='workspace', reason='Retain this durable release procedure.')
                                             for content in SEEDS]})
            if request.get('stream'):
                chunk = {'choices': [{'delta': {'content': text}, 'finish_reason': None}]}
                stop = {'choices': [{'delta': {}, 'finish_reason': 'stop'}]}
                body = f'data: {json.dumps(chunk)}\n\ndata: {json.dumps(stop)}\n\ndata: [DONE]\n\n'
                self.respond(200, 'text/event-stream', body.encode())
            else:
                body = {'id': 'cmpl-fixture', 'object': 'chat.completion',
                        'choices': [{'index': 0, 'message': {'role': 'assistant', 'content': text},
                                     'finish_reason': 'stop'}],
                        'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}
                self.respond(200, 'application/json', json.dumps(body).encode())

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server.url = f'http://127.0.0.1:{server.server_port}'
    Thread(target=server.serve_forever, daemon=True).start()
    try:
        yield server, captures
    finally:
        server.shutdown()
        server.server_close()


@asynccontextmanager
async def connect(binary, directory, server, embedding):
    workspace, home = directory / 'workspace', directory / 'home'
    home.mkdir(exist_ok=True)
    command, env = base.acp_process('candidate', binary, directory, workspace, home, server, embedding)
    with (directory / 'stderr.log').open('ab') as stderr:
        process = await asyncio.create_subprocess_exec(*command, cwd=workspace, env=env, stderr=stderr,
                                                       stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
                                                       limit=2**22)
        client = base.Acp(process, workspace)
        try:
            result = await call(client, 'initialize', {'protocolVersion': 1, 'clientCapabilities': {}})
            assert result['protocolVersion'] == 1
            result = await call(client, 'session/new', {'cwd': str(workspace), 'mcpServers': []})
            yield client, result['sessionId']
        finally:
            await base.close_acp(process)


async def call(client, method, params):
    reply, _, _, _ = await client.call(method, params, timeout=90)
    if 'result' not in reply:
        raise RuntimeError(f'{method} refused: {reply.get("error")}')
    return reply['result']


async def prompt(client, session, text):
    result = await call(client, 'session/prompt', {'sessionId': session, 'prompt': [{'type': 'text', 'text': text}]})
    assert result['stopReason'] == 'end_turn', result


async def records(client):
    result = await call(client, '_aster/memory/manage', {'workspace_root': str(client.workspace),
                       'request': {'operation': 'list', 'query': '', 'cursor': None, 'memory_type': None}})
    fields = ('id', 'content', 'metadata')
    return [{field: row[field] for field in fields} for row in result['outcome']['Ok']['items']]


async def await_records(client, count):
    async with asyncio.timeout(45):
        while len(rows := await records(client)) < count:
            await asyncio.sleep(0.25)
    return rows


async def evaluate(args):
    args.out.mkdir()
    seed = args.out / 'seed'
    (seed / 'workspace').mkdir(parents=True)
    memory_path = Path('workspace/.aster/memory')
    if args.seed_directory:
        clone_memory(args.seed_directory / memory_path, seed / memory_path)
    with endpoint() as (server, _):
        async with connect(args.reference, seed, server, args.embedding) as (client, session):
            if not args.seed_directory:
                await prompt(client, session, 'Remember the Cedar release workflows in this workspace.')
            frozen = await await_records(client, len(SEEDS))
    ids = {row['content']: row['id'] for row in frozen}
    assert set(ids) == set(SEEDS), frozen
    relay = base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl')
    trials = []
    try:
        for (case, (text, marker, relation)), order in zip(CASES.items(), (('reference', 'candidate'),
                                                                            ('candidate', 'reference'))):
            for arm in order:
                label = f'{case}-{arm}'
                directory = args.out / label
                (directory / 'workspace').mkdir(parents=True)
                clone_memory(seed / memory_path, directory / memory_path)
                with endpoint(relay) as (server, requests):
                    binary = getattr(args, arm)
                    relay.take(label, 1)
                    async with connect(binary, directory, server, args.embedding) as (client, session):
                        await prompt(client, session, text)
                        await collect_retention(client, relay, label)
                    async with connect(binary, directory, server, args.embedding) as (client, _):
                        persisted = await records(client)
                    assert await relay.settle_usage(label)
                current = next(row for row in persisted if row['id'] == ids[OLD])
                expected_status = 'superseded' if relation == 'supersedes' else None
                related = [row for row in persisted if marker in row['content']
                           and row['metadata'].get(relation) == ids[OLD]]
                unaffected = all(seed_record in persisted for seed_record in frozen if seed_record['id'] != ids[OLD])
                archive_bound = relation != 'supersedes' or (len(related) == 1
                                    and current['metadata'].get('superseded_by') == related[0]['id'])
                usage = base.summarise_usage([record for record in relay.rows if record['trial'] == label])
                outcome = {'case': case, 'arm': arm, 'request_capture': requests, 'persistent_records': persisted,
                           'passed': len(related) == 1 and current['metadata'].get('aster.memory.status') == expected_status
                           and current['content'] == OLD and unaffected and archive_bound and usage['usage_complete'], **usage}
                trials.append(outcome)
                (args.out / 'trials.json').write_text(json.dumps(trials, ensure_ascii=False, indent=2) + '\n')
                print(f'{label} {outcome["passed"]} {usage["tokens"]}', flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


def clone_memory(source, destination):
    shutil.copytree(source, destination, ignore=shutil.ignore_patterns('.aster-vec.lock*'))


async def collect_retention(client, relay, label):
    assert await relay.settle_usage(label)
    try:
        await await_records(client, len(SEEDS) + 1)
    except TimeoutError:
        # The independent read after reopen grades an absent or rejected proposal.
        pass


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('reference', 'candidate', 'embedding', 'out'):
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--seed-directory', type=lambda value: Path(value).resolve())
    asyncio.run(evaluate(parser.parse_args()))
