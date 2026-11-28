#!/usr/bin/env python3
"""Measure input usage for synthetic requests with paired tool and policy representations."""

import argparse
import copy
import http.client
import json
import os
from pathlib import Path
import re

PROJECTIONS = ('signature', 'keys', 'leaf', 'required', 'bare', 'discriminator')
SCHEMA_KEYS = {'type', 'properties', 'required', 'items', 'enum', 'description', 'default', 'pattern'}
PRIMITIVES = {'string', 'integer', 'number', 'boolean', 'array', 'object', 'null'}


def concise(value):
    return json.dumps(value, separators=(',', ':'), ensure_ascii=False)


def spell(node, path, notes):
    if not isinstance(node, dict) or set(node) - SCHEMA_KEYS:
        raise ValueError('unsupported schema')
    kind = node.get('type')
    if 'required' in node and 'properties' not in node:
        raise ValueError('required fields without object properties')
    if 'items' in node and 'properties' in node:
        raise ValueError('mixed array and object schema')
    annotation = []
    if 'default' in node:
        annotation.append('default ' + concise(node['default']))
    if 'pattern' in node:
        annotation.append('pattern ' + concise(node['pattern']))
    if 'description' in node:
        annotation.append(node['description'])
    if annotation:
        notes.append(f'{path}: ' + '. '.join(annotation))
    if 'enum' in node:
        if 'properties' in node or 'items' in node:
            raise ValueError('enum with compound types')
        if kind is not None and not (kind == 'string' and all(isinstance(v, str) for v in node['enum'])):
            raise ValueError('explicit enum type')
        return '|'.join(concise(value) for value in node['enum'])
    if 'properties' in node:
        fields = node['properties']
        if kind not in (None, 'object') or not isinstance(fields, dict):
            raise ValueError('invalid object')
        required = set(node.get('required', []))
        if not required <= fields.keys():
            raise ValueError('missing required field')
        declarations = []
        for name, child in fields.items():
            if not re.fullmatch(r'[A-Za-z_$][\w$]*', name, flags=re.ASCII):
                raise ValueError('requires quoted field name')
            marker = '' if name in required else '?'
            child_path = f'{path}.{name}' if path else name
            declarations.append(f'{name}{marker}:' + spell(child, child_path, notes))
        return '{' + ','.join(declarations) + '}'
    if 'items' in node:
        if kind != 'array':
            raise ValueError('invalid array')
        return 'Array<' + spell(node['items'], path + '[]', notes) + '>'
    if kind is None or kind in PRIMITIVES:
        return kind or 'unknown'
    raise ValueError('unsupported type')


def compact(tools, style, preserve_native=()):
    result = []
    count = 0
    for tool in tools:
        function = tool['function']
        schema = function.get('parameters')
        if (function['name'] in preserve_native or not isinstance(schema, dict)
                or not isinstance(schema.get('properties'), dict) or function.get('strict')):
            result.append(tool)
            continue
        notes = []
        try:
            signature = spell(schema, '', notes)
        except (TypeError, ValueError):
            result.append(tool)
            continue
        if style == 'leaf' and any(fragment in signature for fragment in ('Array<{', ':{')):
            result.append(tool)
            continue
        description = function['description'] + '\nArguments (? = optional): ' + signature
        if notes:
            description += '\n' + '\n'.join(notes)
        parameters = {} if style in ('bare', 'discriminator') else {'type': 'object'}
        if style == 'keys':
            parameters['properties'] = {name: {} for name in schema['properties']}
            if 'required' in schema:
                parameters['required'] = schema['required']
        if style in ('required', 'discriminator'):
            required = [name for name in schema.get('required', ())
                        if style == 'required' or 'enum' in schema['properties'][name]]
            if required:
                parameters['type'] = 'object'
                parameters['properties'] = {
                    name: {key: value for key, value in schema['properties'][name].items()
                           if key in ('type', 'enum')}
                    for name in required
                }
                parameters['required'] = required
        result.append({**tool, 'function': {**function, 'description': description, 'parameters': parameters}})
        count += 1
    return result, count


def meter(args):
    key = os.environ['DEEPSEEK_API_KEY']
    aster = json.loads((args.capture / '1-candidate-first-wire.json').read_text())
    dsh = json.loads((args.capture / '1-dsh-first-wire.json').read_text())
    user = [message for message in aster['messages'] if message['role'] == 'user']
    assert len(user) == 1 and user[0] in dsh['messages']
    if args.standing:
        previous = aster['messages'][0]['content']
        assert previous.count('Session capabilities:') == 1
        assert 'thinking_budget=1024' in previous and 'Tool calls have no count limit.' in previous
        runtime = previous[previous.index('Session capabilities:'):]
        standing = (args.standing.read_text().strip()
                    + '\n\nRespect thinking_budget=1024 as an upper bound, without expanding scope to consume it.'
                    + '\nTool calls have no count limit.\n\n' + runtime)
        labels = ('native', 'standing', 'discriminator', 'combined', 'dsh', 'discriminator', 'combined', 'native')
    else:
        labels = ('native', *PROJECTIONS, 'dsh', 'system_only', 'native_tools_only', 'dsh_tools_only', 'native')

    results = []
    for label in labels:
        request = copy.deepcopy(dsh if label.startswith('dsh') else aster)
        request['stream'] = False
        request.pop('stream_options', None)
        request['max_tokens'] = 1
        if label in ('standing', 'combined'):
            request['messages'][0]['content'] = standing
        count = 0
        if label in PROJECTIONS or label == 'combined':
            style = 'discriminator' if label == 'combined' else label
            request['tools'], count = compact(request['tools'], style)
        if label == 'system_only':
            request.pop('tools')
        if label.endswith('_tools_only'):
            request['messages'] = user
        wire = concise(request).encode()
        connection = http.client.HTTPSConnection('api.deepseek.com', timeout=50)
        try:
            connection.request('POST', '/chat/completions', wire,
                               {'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json'})
            response = connection.getresponse()
            payload = json.loads(response.read())
            assert response.status == 200, (label, response.status)
        finally:
            connection.close()
        row = {'arm': label, 'usage': payload['usage'], 'tool_count': len(request.get('tools', ())),
               'converted': count, 'wire_bytes': len(wire),
               'system_bytes': sum(len(msg['content'].encode()) for msg in request['messages'] if msg['role'] == 'system')}
        results.append(row)
        print(label, row['usage']['prompt_tokens'], count, flush=True)
    args.out.write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('out', type=Path)
    parser.add_argument('--standing', type=Path)
    meter(parser.parse_args())
