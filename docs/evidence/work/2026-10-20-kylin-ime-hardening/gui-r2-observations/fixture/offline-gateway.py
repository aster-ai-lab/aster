"""Owned loopback-only model supplier. No external requests or real credentials."""
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import sys
import threading
import time

scope = Path(sys.argv[1]).resolve()
assert scope.is_dir()
TOKEN = 'fixture-only-kylin-ime-not-an-account'
MODEL = 'gpt-test'
REPLY = 'OFFLINE_IME_FIXTURE_OK'
lock = threading.Lock()
sequence = 0


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_args):
        pass

    def record(self, status, body=None):
        global sequence
        with lock:
            sequence += 1
            record = {'sequence': sequence, 'method': self.command, 'path': self.path,
                      'status': status, 'time_ns': time.monotonic_ns()}
            if body is not None:
                record.update({'model': body.get('model'), 'stream': body.get('stream', False)})
                messages = body.get('messages', [])
                user = next((item.get('content') for item in reversed(messages)
                             if isinstance(item, dict) and item.get('role') == 'user'), None)
                encoded = json.dumps(user, ensure_ascii=False, separators=(',', ':')).encode()
                record['last_user_content_sha256'] = hashlib.sha256(encoded).hexdigest()
                record['last_user_content_bytes'] = len(encoded)
                record['plain_control_marker'] = 'ime-plain-control' in encoded.decode()
            with (scope/'gateway-wire.jsonl').open('a') as output:
                output.write(json.dumps(record)+'\n')

    def reply(self, status, value):
        payload = json.dumps(value).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def authenticated(self):
        return self.headers.get('Authorization') == 'Bearer '+TOKEN

    def do_GET(self):
        if self.path == '/v1/models' and self.authenticated():
            self.record(200)
            self.reply(200, {'object': 'list', 'data': [{'id': MODEL, 'object': 'model',
                                                       'created': 0, 'owned_by': 'offline-ime-fixture'}]})
        else:
            status = 401 if self.path == '/v1/models' else 404
            self.record(status)
            self.reply(status, {'error': 'offline fixture has no such route'})

    def do_POST(self):
        length = int(self.headers.get('Content-Length', '0'))
        if length <= 0 or length > 1024 * 1024:
            self.record(413)
            self.reply(413, {'error': 'bounded fixture request refused'})
            self.close_connection = True
            return
        raw = self.rfile.read(length)
        try:
            body = json.loads(raw)
        except (UnicodeDecodeError, ValueError):
            self.record(400)
            self.reply(400, {'error': 'invalid JSON'})
            return
        if self.path != '/v1/chat/completions' or not self.authenticated() or body.get('model') != MODEL:
            self.record(400)
            self.reply(400, {'error': 'only the synthetic fixture model is admitted'})
            return
        self.record(200, body)
        if not body.get('stream'):
            self.reply(200, {'id': 'offline-ime-fixture', 'object': 'chat.completion', 'created': 0,
                             'model': MODEL, 'choices': [{'index': 0,
                                 'message': {'role': 'assistant', 'content': REPLY}, 'finish_reason': 'stop'}],
                             'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}})
            return
        chunks = []
        for delta, reason in [({'role': 'assistant', 'content': REPLY}, None), ({}, 'stop')]:
            value = {'id': 'offline-ime-fixture', 'object': 'chat.completion.chunk', 'created': 0,
                     'model': MODEL, 'choices': [{'index': 0, 'delta': delta, 'finish_reason': reason}]}
            chunks.append('data: '+json.dumps(value)+'\n\n')
        payload = (''.join(chunks)+'data: [DONE]\n\n').encode()
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Content-Length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)
        self.wfile.flush()


server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
server.daemon_threads = True
(scope/'gateway.json').write_text(json.dumps({'origin': 'http://127.0.0.1:'+str(server.server_port),
                                             'model': MODEL, 'reply': REPLY})+'\n')
server.serve_forever(poll_interval=0.1)
