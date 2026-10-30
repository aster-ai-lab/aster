#!/usr/bin/env python3
"""Exercise web-tool refusals through the shipped terminal and real ACP."""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import sys
import threading

import capability_baseline as baseline


class PrivateHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.server.requests.append(self.path)
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'private fixture must remain unread')

    def log_message(self, *_args):
        pass


def main():
    if len(sys.argv) != 3:
        raise SystemExit('usage: network_boundaries.py STAGED_PAIR EVIDENCE_DIRECTORY')
    server = ThreadingHTTPServer(('127.0.0.1', 0), PrivateHandler)
    server.requests = []
    thread = threading.Thread(target=server.serve_forever)
    thread.start()
    baseline.CASES = {
        'fetch-invalid-url': ('web_fetch', {'url': 'not-a-url'}, 'must start with'),
        'fetch-private-address': ('web_fetch', {'url': f'http://127.0.0.1:{server.server_port}/private'}, 'URL resolves to a non-public address and was blocked: 127.0.0.1'),
        'search-unknown-engine': ('web_search', {'query': 'boundary probe', 'engines': ['nonexistent']}, 'No valid engines'),
    }
    baseline.REFUSED_CASES = set(baseline.CASES)
    try:
        result = baseline.main()
        receipt = {'cases': len(baseline.CASES), 'terminal_suite_exit_code': result, 'private_http_requests': server.requests}
        (Path(sys.argv[2]) / 'network-boundaries.json').write_text(json.dumps(receipt, indent=2) + '\n')
        assert not server.requests, 'the rejected private target received an HTTP request'
        return result
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


if __name__ == '__main__':
    raise SystemExit(main())
