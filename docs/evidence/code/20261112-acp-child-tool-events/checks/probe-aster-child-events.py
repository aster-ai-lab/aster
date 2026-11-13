import json
from pathlib import Path
import secrets
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import test_bindings_native as bindings

native_prompt = bindings.AcpClient.prompt


def probe(client, session, text, **options):
    result = native_prompt(client, session, text, **options)
    offset = len(client.updates)
    marker = 'KRUN_CHILD_PROOF:' + secrets.token_hex(16)
    delegated = native_prompt(client, session,
        'Perform a delegation-only protocol probe. Use your native task tool to ask the built-in '
        'verification worker to run exactly this command in its native bash tool: printf ' + marker + '. '
        'The worker must actually execute bash and report the exit status and observed output. '
        'Do not run that command in the parent. A text-only worker answer is insufficient.', timeout=480)
    events = [message['update'] for message in client.updates[offset:]]
    tasks = [(index, event['toolCallId']) for index, event in enumerate(events)
             if event.get('sessionUpdate') == 'tool_call' and event.get('title') == 'task']
    assert tasks, 'native task delegation missing'
    verified = []
    for start, parent in tasks:
        end = next((index for index, event in enumerate(events) if index > start
                    and event.get('toolCallId') == parent and event.get('status') == 'completed'), None)
        assert end is not None, 'parent task did not complete'
        for index in range(start + 1, end):
            call = events[index]
            if call.get('sessionUpdate') != 'tool_call' or call.get('title') != 'bash':
                continue
            if marker not in call.get('rawInput', {}).get('command', ''):
                continue
            identifier = call['toolCallId']
            terminal = next((position for position in range(index + 1, end)
                             if events[position].get('toolCallId') == identifier
                             and events[position].get('status') == 'completed'), None)
            assert terminal is not None, 'child bash did not complete before its parent'
            assert any(event.get('toolCallId') == identifier and marker in json.dumps(event.get('content', []))
                       for event in events[index + 1:terminal + 1]), 'actual child output missing'
            verified.append(dict(parent=parent, child=identifier, startIndex=index, terminalIndex=terminal,
                                 parentTerminalIndex=end))
    assert verified, 'no actual delegated bash execution observed'
    (client.owner.evidence / 'delegation-audit.json').write_text(json.dumps(dict(status='PASS', marker=marker,
        toolCalls=delegated['tool_calls'], seconds=delegated['elapsed_s'], verified=verified), indent=2) + '\n')
    return result


bindings.AcpClient.prompt = probe
bindings.main()
