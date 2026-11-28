#!/usr/bin/env python3
"""Local, effect-free MCP fixture for a documented argument value."""

import json
import sys


FORMAT = 'Harbor-Protocol-4387'
MARKER = 'SCHEMA-ACK-TEAL-2087'
DEFINITION = {
    'name': 'observe_format',
    'description': 'Report the synthetically accepted format version.',
    'inputSchema': {
        'type': 'object',
        'properties': {'format_version': {'type': 'string', 'description': (
            'The value is determined by this fixture version rather than by the request. '
            'The exact required literal is ' + FORMAT + '. Supply the literal with its case and punctuation.'
        )}},
        'required': ['format_version'],
        'additionalProperties': False,
    },
    'annotations': {'readOnlyHint': True, 'destructiveHint': False, 'openWorldHint': False},
}


def dispatch(request):
    if request['method'] == 'initialize':
        return {'protocolVersion': '2024-11-05', 'capabilities': {'tools': {}},
                'serverInfo': {'name': 'fixture', 'version': '1'}}
    if request['method'] == 'tools/list':
        return {'tools': [DEFINITION]}
    if request['method'] == 'tools/call':
        params = request.get('params', {})
        matched = params.get('name') == DEFINITION['name'] and params.get('arguments') == {'format_version': FORMAT}
        text = MARKER if matched else 'The exact documented format version is required.'
        return {'content': [{'type': 'text', 'text': text}], 'isError': not matched}
    return {}


for line in sys.stdin:
    request = json.loads(line)
    if 'id' in request:
        print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'result': dispatch(request)}), flush=True)
