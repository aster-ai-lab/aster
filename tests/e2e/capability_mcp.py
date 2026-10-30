"""Synthetic stdio MCP service for the actual terminal capability regression."""

import json
import os
from pathlib import Path
import sys


def record(event):
    with Path(os.environ['A3S_TEST_MCP_LOG']).open('a') as output:
        output.write(json.dumps({'event': event, 'pid': os.getpid()}) + '\n')


record('started')
for line in sys.stdin:
    request = json.loads(line)
    if 'id' not in request:
        continue
    method = request.get('method')
    if method == 'initialize':
        result = {'protocolVersion': '2024-11-05', 'capabilities': {'tools': {}}, 'serverInfo': {'name': 'fixture', 'version': '1'}}
    elif method == 'tools/list':
        result = {'tools': [{'name': 'lookup', 'description': 'Return the synthetic fixture value.', 'inputSchema': {'type': 'object', 'properties': {}, 'additionalProperties': False}, 'annotations': {'readOnlyHint': True, 'destructiveHint': False, 'openWorldHint': False}}]}
    elif method == 'tools/call':
        assert request['params'] == {'name': 'lookup', 'arguments': {}}
        record('called')
        result = {'content': [{'type': 'text', 'text': 'fixture'}], 'isError': False}
    else:
        result = {}
    print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'result': result}), flush=True)
record('stdin_closed')
