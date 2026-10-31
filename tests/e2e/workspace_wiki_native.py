#!/usr/bin/env python3
"""Real native/ACP Wiki generation over exclusively created synthetic task files."""
import argparse
import hashlib
import json
import io
import re
from pathlib import Path
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from xml.sax.saxutils import escape


FIXTURE = {
    'README.md': '# ChronoFlow\n\nChronoFlow runs a bounded retry queue. The entry point is main.py.\n'
                 'Run locally with `python3 main.py`. Read docs/operations.md for retention policy.\n',
    'main.py': 'from retry_queue import retry_delays\n\n'
               'def main():\n    return retry_delays(3)\n\n'
               'if __name__ == "__main__":\n    print(main())\n',
    'retry_queue.py': 'MAX_RETRIES = 3\nBASE_DELAY_SECONDS = 2\n\n'
                      'def retry_delays(attempts):\n'
                      '    if not 0 <= attempts <= MAX_RETRIES:\n'
                      '        raise ValueError("retry limit exceeded")\n'
                      '    return [BASE_DELAY_SECONDS * (2 ** n) for n in range(attempts)]\n',
    'docs/operations.md': '# Operations\n\nFinished queue records are retained for 7 days.\n'
                          'The retry delay sequence for three attempts is 2, 4, 8 seconds.\n'
                          'This example contains no network or persistence implementation.\n',
    '.env': 'PRIVATE_SENTINEL=wiki_native_xqz_unshared_20261026\n',
}


def fixture_files(kind):
    if kind == 'code':
        return {name: text.encode() for name, text in FIXTURE.items()}
    document = io.BytesIO()
    paragraphs = [
        'LumenOps service reporting policy',
        'The source register is data/requests.csv. Each row represents one request.',
        'A request is closed only when its status equals closed. Open requests remain in the backlog.',
        'The management summary is reports/management-summary.md. It reports the closed count, backlog and closure rate.',
        'Closure rate is closed requests divided by all requests. The target is 80 percent.',
        'The policy defines reporting rules, not a live service implementation.'
    ]
    with zipfile.ZipFile(document, 'w', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr(zipfile.ZipInfo('[Content_Types].xml', (2026, 1, 1, 0, 0, 0)), '<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
        archive.writestr(zipfile.ZipInfo('_rels/.rels', (2026, 1, 1, 0, 0, 0)), '<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
        archive.writestr(zipfile.ZipInfo('word/document.xml', (2026, 1, 1, 0, 0, 0)), '<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>' + ''.join('<w:p><w:r><w:t>' + escape(text) + '</w:t></w:r></w:p>' for text in paragraphs) + '</w:body></w:document>')
    files = {
        'README.md': b'# LumenOps reporting workspace\n\nRead policy/service-policy.docx for the reporting rules. The source register is data/requests.csv; its derived deliverable is reports/management-summary.md.\n',
        'policy/service-policy.docx': document.getvalue(),
        'data/requests.csv': ('id,department,status\n' + ''.join(f'R{i:02},Support,{"closed" if i <= 9 else "open"}\n' for i in range(1, 13))).encode(),
        'reports/management-summary.md': b'# Management summary\n\nDerived from data/requests.csv using policy/service-policy.docx.\n12 requests: 9 closed and 3 open. Closure rate is 75 percent, below the 80 percent target.\n',
        '.env': FIXTURE['.env'].encode(),
    }
    if kind == 'mixed':
        files['analysis.py'] = b'import csv\nfrom pathlib import Path\n\ndef closure_rate():\n    with Path("data/requests.csv").open() as source:\n        rows = list(csv.DictReader(source))\n    closed = sum(row["status"] == "closed" for row in rows)\n    return closed, len(rows) - closed, closed / len(rows)\n\nif __name__ == "__main__":\n    print(closure_rate())\n'
    return files


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def loopback(url):
    parsed = urllib.parse.urlsplit(url)
    assert parsed.scheme == 'http' and parsed.hostname == '127.0.0.1', 'non-loopback runtime refused'
    return url.rstrip('/')


def publish_first_fixture_generation(plan, maintenance, output_root):
    """Review only this harness's synthetic candidate, then require one complete publication."""
    active = plan['active']
    if active is None:
        return plan
    pending = [change for change in active['changes'] if change['decision'] == 'pending']
    for change in active['changes']:
        detail = maintenance({'operation': 'change', 'job_id': active['id'], 'path': change['path']})['result']
        assert detail['before'] == '', 'first generation unexpectedly replaced an existing page'
        assert '## Sources' in detail['after'], 'candidate has no source citations'
        assert set(change['sources']) <= set(detail['evidence']), 'candidate evidence is incomplete'
        assert 'wiki_native_xqz_unshared_20261026' not in detail['after']
    assert not list(output_root.joinpath('concepts').rglob('*.md')), 'partial generation published before review'
    for index, change in enumerate(pending):
        command = {'operation': 'review', 'job_id': active['id'], 'path': change['path'],
                   'generation': change['generation'], 'accept': True}
        if index == 0:
            try:
                maintenance({**command, 'generation': '0' * 64})
            except urllib.error.HTTPError as error:
                assert error.code in (400, 409), 'unexpected stale review refusal'
            else:
                raise AssertionError('stale review generation was accepted')
        plan = maintenance(command)['result']
        if index + 1 < len(pending):
            assert not list(output_root.joinpath('concepts').rglob('*.md')), 'partial generation leaked'
    assert plan['active'] is None, 'review did not settle the entire generation'
    assert {str(path.relative_to(output_root)) for path in output_root.joinpath('concepts').rglob('*.md')} == {
        change['path'] for change in active['changes']
    }, 'published generation is incomplete'
    return plan


def verify_published_citations(pages, output_root, source_sha256, maintenance):
    citations = []
    for page in pages:
        relative = str(page.relative_to(output_root))
        references = list(dict.fromkeys(ref for ref in re.findall(r'\]\(([^)]+)\)', page.read_text()) if '_sources/' in ref))
        assert references and len(references) <= 64, 'missing or unbounded source citations'
        for reference in references:
            source = maintenance({'operation': 'citation', 'page': relative, 'reference': reference})['result']
            assert source['current'], 'stale source citation'
            assert source['sourcePath'] != '.env' and source['sourcePath'] in source_sha256, 'foreign source citation'
            assert source['pageGeneration'] == 'sha256:' + digest(page), 'changed page citation'
            assert source['sourceGeneration'] == 'sha256:' + source_sha256[source['sourcePath']], 'changed source citation'
            citations.append({'page': relative, 'reference': reference, 'source': source['sourcePath'], 'current': True})
    return citations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--agent-id', required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--timeout', type=int, default=600)
    parser.add_argument('--fixture', choices=['code', 'documents', 'mixed'], default='code')
    parser.add_argument('--cancel-running', action='store_true', help='Cancel through the Wiki owner after model output begins')
    args = parser.parse_args()
    files = fixture_files(args.fixture)
    runtime = json.loads(args.runtime.read_text())
    authority = loopback(runtime['apiBaseUrl'])
    headers = {'Authorization': 'Bearer ' + runtime['token'], 'Content-Type': 'application/json'}
    receipt = {'case': 'workspace-wiki-native-cancel-v1' if args.cancel_running else 'workspace-wiki-native-real-model-v1',
               'checks': {}, 'compile_runs_started': 0,
               'provider_call_count': None, 'fixture': args.fixture}
    started = time.monotonic()
    compile_session = None
    task_id = None
    job = None
    binding = None
    finished = False

    def request(method, path, body=None, base=None):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request((base or plane) + path, data=data, headers=headers, method=method)
        with urllib.request.urlopen(req, timeout=30) as response:
            return json.load(response)

    def checkpoint(name, **fields):
        print(json.dumps({'checkpoint': name, 'elapsed_seconds': round(time.monotonic() - started, 2), **fields}), flush=True)

    plane = loopback(request('GET', '/api/agent-plane', base=authority)['baseUrl'])
    agent = request('GET', '/api/agents/' + urllib.parse.quote(args.agent_id, safe=''))
    assert agent['type'] == 'aster', 'aster agent required'
    receipt['agent_id'] = agent['id']
    receipt['model'] = agent.get('model')
    try:
        task = request('POST', '/api/agent-sessions', {
            'agentId': agent['id'], 'name': f'Wiki native synthetic {args.fixture} verification',
            'workspace': {'type': 'system'},
        })
        task_id = task['id']
        root = Path(task['workspace']['path']).resolve(strict=True)
        assert root.is_dir() and not any(root.iterdir()), 'new task workspace must be empty'
        for name, body in files.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            with path.open('xb') as output:
                output.write(body)
        before = {name: digest(root / name) for name in files}
        receipt['source_sha256'] = before
        binding = request('GET', f'/api/agent-sessions/{task_id}/wiki-binding')
        assert Path(binding['sourceRoot']) == root
        assert not Path(binding['outputRoot']).exists(), 'browsing must not create a Wiki'
        route = f'/api/agent-sessions/{task_id}/wiki-maintenance'

        def maintenance(command):
            return request('POST', route, {'bindingRevision': binding['bindingRevision'], 'command': command})

        admitted = request('GET', route)
        projections = admitted['inputs']['projections']
        assert {item['path'] for item in projections} == set(files) - {'.env'}
        assert all(item['status'] == 'included' for item in projections)
        job = maintenance({'operation': 'begin'})['result']
        assert job and len(job['sources']) == len(files) - 1
        stage = Path(job['staging'])
        assert 'wiki_native_xqz_unshared_20261026' not in '\n'.join(p.read_text() for p in (stage / '_sources').glob('*.md'))
        receipt.update(task_id=task_id, source_root=str(root), binding=binding, job_id=job['id'], inputs=admitted['inputs'])
        checkpoint('admitted', sources=len(job['sources']))
        workspace = request('POST', '/api/agent-workspaces', {
            'path': str(stage), 'name': 'Wiki compile fixture', 'purpose': 'knowledge-compile',
        })
        compile_session = request('POST', '/api/agent-sessions', {
            'agentId': agent['id'], 'name': 'Wiki native compile verification', 'description': 'knowledge-compile',
            'workspace': {'type': 'user', 'workspace': workspace},
        })['id']
        receipt['compile_session_id'] = compile_session
        try:
            request('POST', f'/api/agent-sessions/{compile_session}/prewarm', {})
        except urllib.error.HTTPError as error:
            assert error.code == 409, 'unexpected compile prewarm refusal'
        else:
            raise AssertionError('compile session admitted ordinary prewarming')
        receipt['checks']['ordinary_compile_prewarm_refused'] = True
        prompt = ('Read .wiki-task.md and complete its work order from the supplied snapshots. '
                  'Produce a concise workspace overview and evidence-supported topics, with citations and '
                  'the strict .wiki-changes.json manifest. Preserve the sources. Stop after staging the complete proposal.')
        opened = request('POST', f'/api/agent-sessions/{compile_session}/turns', {
            'sessionId': compile_session, 'agentId': agent['id'],
            'userMessageParts': [{'type': 'text', 'text': prompt}],
            'hostContext': {'guidelines': '', 'review': 'inline', 'outputLanguage': 'zh-CN',
                            'knowledgeCompile': {'root': binding['outputRoot'], 'jobId': job['id'],
                                                 'workspaceRevision': binding['bindingRevision']}},
        })
        assert opened.get('mode') == 'started', 'native Run did not start'
        receipt['compile_runs_started'] = 1
        receipt['run'] = {key: opened[key] for key in ('mode', 'activeExecutions') if key in opened}
        checkpoint('run-started', session_id=compile_session)
        last = None
        cancelled_at = None
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            turn = request('GET', f'/api/agent-sessions/{compile_session}/turn')
            phase = turn.get('streamStatus', {}).get('status', turn['status'])
            if phase != last:
                checkpoint('run-state', phase=phase)
                last = phase
            if turn['status'] == 'not-found':
                break
            assert phase not in ('awaiting-approval', 'awaiting-form'), 'restricted compilation unexpectedly requires interaction'
            if args.cancel_running and cancelled_at is None and any(
                chunk.get('chunk', {}).get('type') in ('text-delta', 'reasoning-delta', 'tool-input-start')
                for chunk in turn.get('bufferedChunks', [])
            ):
                cancelled_at = time.monotonic()
                stopped = maintenance({'operation': 'cancel', 'job_id': job['id']})
                assert stopped['result']['status'] == 'cancelled'
                checkpoint('owner-cancelled')
            if cancelled_at is not None:
                assert time.monotonic() - cancelled_at < 15, 'cancelled compile Run did not stop promptly'
            time.sleep(1)
        else:
            raise TimeoutError('native compile exceeded the test deadline')
        messages = request('GET', f'/api/agent-sessions/{compile_session}/messages')['items']
        assistant = next((row for row in messages if row['role'] == 'assistant'), None)
        receipt['terminal'] = {'status': assistant['status'], 'stats': assistant.get('stats')} if assistant else None
        if args.cancel_running:
            assert cancelled_at is not None, 'model finished before the cancellation test could act'
            assert assistant and assistant['status'] == 'paused', 'cancelled compile did not retain its terminal status'
            assert not stage.exists(), 'cancelled staging survived'
            assert {name: digest(root / name) for name in files} == before, 'source file mutation'
            assert not any(Path(binding['outputRoot']).joinpath('concepts').rglob('*.md')), 'cancel published candidate pages'
            assert request('GET', f'/api/agent-sessions/{task_id}/messages')['items'] == [], 'compilation changed the original task conversation'
            finished = True
            receipt['checks'].update(owner_cancel_stops_run=True, staging_removed=True,
                                     source_bytes_preserved=True, no_candidate_published=True,
                                     original_conversation_preserved=True)
            receipt['cancel_elapsed_seconds'] = round(time.monotonic() - cancelled_at, 3)
            checkpoint('cancel-verified', cancel_elapsed_seconds=receipt['cancel_elapsed_seconds'])
            return
        if assistant and assistant['status'] == 'error':
            errors = [part.get('data', {}).get('message', '') for part in assistant.get('data', {}).get('parts', [])
                      if part.get('type') == 'data-error']
            receipt['terminal']['errors'] = errors
            raise AssertionError('native Run ended in error before proposal settlement')
        assert (stage / '.wiki-changes.json').is_file(), 'real model produced no proposal manifest'
        plan = maintenance({'operation': 'finish', 'job_id': job['id']})['result']
        assert plan['active'] is None or plan['active']['ready'], 'real model proposal was not prepared'
        finished = True
        receipt['plan'] = plan
        pages = sorted(Path(binding['outputRoot']).joinpath('concepts').rglob('*.md'))
        assert all('## Sources' in page.read_text() for page in pages)
        assert not any('wiki_native_xqz_unshared_20261026' in page.read_text() for page in pages)
        assert {name: digest(root / name) for name in files} == before, 'source file mutation'
        if plan['active'] is not None:
            receipt['checks'].update(source_bytes_preserved=True, owner_prepared=True,
                                     uncertain_changes_retained_for_review=True)
            checkpoint('awaiting-review', published_pages=len(pages),
                       pending_changes=sum(change['decision'] == 'pending' for change in plan['active']['changes']))
            plan = publish_first_fixture_generation(plan, maintenance, Path(binding['outputRoot']))
            receipt['plan'] = plan
            receipt['checks'].update(stale_review_refused=True, complete_generation_published=True)
            pages = sorted(Path(binding['outputRoot']).joinpath('concepts').rglob('*.md'))
        assert pages, 'real model proposal produced no readable pages'
        assert maintenance({'operation': 'begin'})['result'] is None, 'unchanged input scheduled another compile'
        receipt['citations'] = verify_published_citations(pages, Path(binding['outputRoot']), before, maintenance)
        receipt['checks'].update(source_bytes_preserved=True, secret_excluded=True, owner_settled=True,
                                 pages_have_owner_citations=True, unchanged_input_no_new_job=True,
                                 every_citation_current=True)
        receipt['pages'] = [{'path': str(p.relative_to(binding['outputRoot'])), 'sha256': digest(p)} for p in pages]
        checkpoint('published', pages=len(pages))
    except Exception as error:
        receipt['error'] = {'type': type(error).__name__, 'http_status': getattr(error, 'code', None)}
        if isinstance(error, (AssertionError, TimeoutError)):
            receipt['error']['message'] = str(error)
        raise
    finally:
        if not finished:
            if compile_session:
                try:
                    request('POST', f'/api/agent-sessions/{compile_session}/turn/abort', {})
                except Exception:
                    receipt['abort_unconfirmed'] = True
            if task_id and job and binding:
                try:
                    maintenance({'operation': 'cancel', 'job_id': job['id']})
                except Exception:
                    receipt['cancel_unconfirmed'] = True
        receipt['elapsed_seconds'] = round(time.monotonic() - started, 2)
        args.evidence.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()
