#!/usr/bin/env python3
"""Loopback model oracle; Aster's real ACP executes ask_user_form."""
import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FORM = (ROOT / 'packages/intelligent-ui/fixtures/form.json').read_text()
EXPECTED = {'name': 'Ada', 'amount': 4, 'ready': True, 'choice': 'alpha', 'notes': 'Need a detailed plan'}


def decide(body):
    raw = json.dumps(body, ensure_ascii=False)
    if 'emit_pre_analysis' in raw or 'You are a pre-analysis assistant' in raw:
        result = {'intent': 'GeneralPurpose', 'requires_planning': False,
                  'goal': {'description': 'Collect synthetic form answers', 'success_criteria': []},
                  'execution_plan': {'complexity': 'Simple', 'steps': [], 'required_tools': []},
                  'optimized_input': ''}
        return {'role': 'assistant', 'content': json.dumps(result)}, 'stop', {'kind': 'pre_analysis'}
    if 'You extract durable, reusable memory' in raw:
        return {'role': 'assistant', 'content': '{"items":[]}'}, 'stop', {'kind': 'memory'}
    messages = body.get('messages', [])
    user = next((m.get('content', '') for m in reversed(messages) if m.get('role') == 'user'), '')
    user = user if isinstance(user, str) else json.dumps(user)
    markers = {'JSON-RENDER-CUA-SUBMIT': 'submit', 'JSON-RENDER-CUA-CANCEL': 'cancel',
               'JSON-RENDER-CUA-INVALID': 'invalid'}
    cases = [case for marker, case in markers.items() if marker in user]
    if len(cases) != 1:
        return {'role': 'assistant', 'content': 'JSON-RENDER-CUA-MISSING-MARKER'}, 'stop', {'kind': 'missing_marker'}
    case = cases[0]
    call_id = 'call-json-render-cua-' + case
    results = [m for m in messages if m.get('role') == 'tool' and m.get('tool_call_id') == call_id]
    event = {'case': case, 'tool_call_id': call_id}
    if results:
        content = results[-1].get('content', '')
        if case == 'invalid':
            accepted = 'invalid JSON Render document:' in str(content)
            suffix = 'REJECTED' if accepted else 'BAD-REJECTION'
            event.update(kind='tool_result', rejected=accepted)
        elif case == 'cancel':
            accepted = 'form was cancelled by the user' in str(content)
            suffix = 'CANCELLED' if accepted else 'BAD-CANCELLATION'
            event.update(kind='tool_result', cancelled=accepted)
        else:
            try:
                result = json.loads(content)
            except (ValueError, TypeError):
                result = {}
            result = result if isinstance(result, dict) else {}
            values = result.get('values')
            accepted = (result.get('schema') == 'aster.code.user-form.v1' and values == EXPECTED
                        and type(values.get('amount')) in (int, float)
                        and type(values.get('ready')) is bool and result.get('permission_grant') is False)
            suffix = 'PASS: Ada, 4, true, alpha, Need a detailed plan' if accepted else 'BAD-VALUES'
            event.update(kind='tool_result', schema=result.get('schema'), values=values,
                         permission_grant=result.get('permission_grant'), accepted=accepted)
        return {'role': 'assistant', 'content': 'JSON-RENDER-CUA-' + suffix}, 'stop', event
    offered = any(t.get('function', {}).get('name') == 'ask_user_form' for t in body.get('tools', []))
    event.update(kind='tool_call', offered=offered)
    if not offered:
        return {'role': 'assistant', 'content': 'JSON-RENDER-CUA-MISSING-TOOL'}, 'stop', event
    form = 'root = Input("name", "Name")' if case == 'invalid' else FORM
    call = {'id': call_id, 'type': 'function', 'function': {'name': 'ask_user_form',
            'arguments': json.dumps({'form': form, 'prompt': 'JSON Render end-to-end test'})}}
    return {'role': 'assistant', 'content': None, 'tool_calls': [call]}, 'tool_calls', event


class Handler(BaseHTTPRequestHandler):
    model_name = 'json-render-cua'
    decide = staticmethod(decide)

    def log_message(self, *args):
        pass

    def send(self, payload, content_type='application/json'):
        data = payload.encode()
        self.send_response(200)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.send(json.dumps({'object': 'list', 'data': [{'id': self.model_name, 'object': 'model'}]}))

    def do_POST(self):
        try:
            length = int(self.headers.get('Content-Length', '0'))
            if not 0 < length <= 2_000_000:
                self.send_error(413)
                return
            body = json.loads(self.rfile.read(length))
            if not isinstance(body, dict):
                raise ValueError('body must be an object')
        except (ValueError, TypeError):
            self.send_error(400)
            return
        if self.path == '/v1/systemone':
            self.send(json.dumps({'model': 'apofasi-lexical-fixture',
                                 'answers': {'surface': {'type': 'choice', 'choice': 'prose'}}}))
            return
        try:
            message, finish, event = self.decide(body)
        except ValueError as error:
            self.send_error(422, str(error))
            return
        # Retain the synthetic receipt only, never request headers or complete model context.
        with self.server.events.open('a') as output:
            output.write(json.dumps(event, ensure_ascii=False) + '\n')
        if body.get('stream'):
            delta = {k: v for k, v in message.items() if k != 'role'}
            for index, call in enumerate(delta.get('tool_calls', [])):
                call['index'] = index
            chunks = [{'choices': [{'index': 0, 'delta': delta}]},
                      {'choices': [{'index': 0, 'delta': {}, 'finish_reason': finish}]}]
            self.send(''.join('data: ' + json.dumps(chunk) + '\n\n' for chunk in chunks)
                      + 'data: [DONE]\n\n', 'text/event-stream')
        else:
            self.send(json.dumps({'id': self.model_name, 'object': 'chat.completion',
                                 'choices': [{'index': 0, 'message': message, 'finish_reason': finish}],
                                 'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=0)
    parser.add_argument('--events', type=Path, required=True)
    args = parser.parse_args()
    args.events.parent.mkdir(parents=True, exist_ok=True)
    server = ThreadingHTTPServer(('127.0.0.1', args.port), Handler)
    server.events = args.events
    print(json.dumps({'baseUrl': f'http://127.0.0.1:{server.server_port}/v1'}), flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
