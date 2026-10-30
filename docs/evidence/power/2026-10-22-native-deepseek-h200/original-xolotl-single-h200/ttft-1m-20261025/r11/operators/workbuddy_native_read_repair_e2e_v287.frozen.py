"""Actual installed WorkBuddy Read-tool roundtrip; private raw client output stays unretained."""
import json
import os
import subprocess
import threading
import time
import uuid
from pathlib import Path

BASE = Path(__file__).resolve().parent
PLAN = json.loads((BASE / 'workbuddy-native-read-e2e-v60-plan.json').read_text())
OUT = BASE / 'workbuddy-read-repair-e2e-v287-result.json'
PLAN['private_output'] = str(BASE / 'workbuddy-read-private-v287')
Path(PLAN['private_output']).mkdir(exist_ok=False)
STATE = dict(schema='aster-workbuddy-native-read-repair-e2e-v287', unix=time.time(), phase='starting',
             client='installed WorkBuddy lite-wb, headless, Read-only tool profile',
             base_url=PLAN['base_url'], model=PLAN['model'], gui_test_passed=False,
             no_session_persistence=True, native_tool_roundtrip_passed=False)
child = timer = None


def save():
    temp = OUT.with_suffix('.tmp')
    temp.write_text(json.dumps(STATE, indent=2) + '\n')
    temp.replace(OUT)


try:
    save()
    fixture = Path(PLAN['public_fixture']).resolve()
    if fixture.parent != Path(PLAN['cwd']).resolve() or not fixture.is_file():
        raise RuntimeError('exact public WorkBuddy Read fixture differs')
    prompt = ('Use the available Read tool to read only this exact public file: '
              + fixture.as_posix()
              + '. Add the two integer fields left and right from the file. '
                'Reply with only the integer sum. Do not read any other file. '
                'Do not propose a plan and do not ask a question.')
    env = dict(os.environ, CODEBUDDY_FORCE_LITE_WB_BUNDLE='1')
    args = [PLAN['node'], PLAN['entry'], '--model', 'custom-local:' + PLAN['model'],
            '--print', '--output-format', 'stream-json', '--include-partial-messages', '--verbose',
            '--no-session-persistence', '--permission-mode', 'dontAsk', '--tools', 'Read',
            '--allowedTools', 'Read', '--max-turns', '4', '--session-id', 'aster-native-read-' + uuid.uuid4().hex,
            prompt]
    start = time.monotonic()
    first = None
    answer = ''
    tool_calls = []
    final = None
    received = events = 0
    private = Path(PLAN['private_output'])
    with (private / 'stdout.jsonl').open('xb') as raw, (private / 'stderr.log').open('xb') as error:
        child = subprocess.Popen(args, cwd=PLAN['cwd'], env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=error, creationflags=subprocess.CREATE_NO_WINDOW)
        timer = threading.Timer(240, child.terminate)
        timer.start()
        STATE.update(phase='running', client_pid=child.pid)
        save()
        for line in iter(child.stdout.readline, b''):
            received += len(line)
            if len(line) > 4 * 1024**2 or received > 16 * 1024**2:
                raise RuntimeError('bounded WorkBuddy public-probe event output exceeded')
            raw.write(line)
            raw.flush()
            try:
                item = json.loads(line)
            except (ValueError, UnicodeError):
                continue
            events += 1
            if item.get('type') == 'stream_event':
                part = item.get('event', {}).get('delta', {}).get('text') or ''
                if part:
                    first = first or time.monotonic()
            elif item.get('type') == 'assistant':
                content = item.get('message', {}).get('content', [])
                for block in content:
                    if block.get('type') == 'tool_use':
                        values = block.get('input') or {}
                        matches = [value for value in values.values() if isinstance(value, str)
                                   and value.replace('\\', '/') in [fixture.as_posix(), fixture.name]]
                        if block.get('name') != 'Read' or not matches:
                            raise RuntimeError('WorkBuddy requested a tool/path outside the declared public Read fixture')
                        tool_calls.append(dict(name=block['name'], public_fixture_path_preserved=True,
                                               argument_keys=sorted(values)))
                    elif block.get('type') == 'text':
                        answer = block.get('text') or answer
            elif item.get('type') == 'result':
                final = {key: item.get(key) for key in ['subtype', 'is_error', 'duration_ms', 'duration_api_ms',
                                                       'num_turns', 'usage']}
                answer = item.get('result') or answer
                if item.get('is_error'):
                    raise RuntimeError('actual WorkBuddy native-tool result was an error')
            STATE.update(events=events, tool_calls=len(tool_calls), elapsed_seconds=time.monotonic() - start)
            save()
        returncode = child.wait(timeout=15)
    if returncode or final is None or not tool_calls or answer.strip() != '442':
        raise RuntimeError('actual WorkBuddy Read/followup did not produce the exact public sum')
    STATE.update(phase='passed', native_tool_roundtrip_passed=True, client_exit_code=returncode,
                 tool_calls=tool_calls, reply='442', result=final,
                 seconds=time.monotonic() - start,
                 time_to_first_output_seconds=first - start if first is not None else None,
                 finished_unix=time.time())
    save()
except BaseException as error:
    if child is not None and child.poll() is None:
        child.terminate()
        child.wait(timeout=10)
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
finally:
    if timer:
        timer.cancel()
