"""Send a fork's chat completions to the ACL deepseek model.

Signed-in ToWork only calls provider ``a3s`` on the local gateway. That
gateway relays to a different host. This proxy is the harness seam: the fork
is started with ``A3S_GATEWAY_ORIGIN`` pointed here, chat completions are
rewritten onto the ACL model, and every other path is forwarded to the local
gateway. The API key stays in memory and is never written to the evidence file.
"""

from __future__ import annotations

import json
import threading
import urllib.parse
from dataclasses import dataclass
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import http.client


ACL_PATH = Path(__file__).resolve().parent.parents[4] / ".a3s" / "config.acl"
GATEWAY = "http://127.0.0.1:8080"


@dataclass(frozen=True)
class Route:
    model_id: str
    upstream_model: str
    host: str
    api_key: str


def _quoted(line: str) -> str:
    start = line.find('"')
    end = line.rfind('"')
    if start < 0 or end <= start:
        raise ValueError("acl value is not quoted")
    return line[start + 1 : end].replace('\\"', '"').replace("\\\\", "\\")


def load_route(path: Path = ACL_PATH) -> Route:
    """Read provider ``deepseek`` only. Other providers in the file are ignored."""
    default_model = ""
    api_key = ""
    host = ""
    upstream_model = ""
    in_provider = False
    depth = 0
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if line.startswith("default_model") and not default_model:
            default_model = _quoted(line)
            continue
        if line.startswith('providers "'):
            in_provider = line.split('"', 2)[1] == "deepseek"
            depth = line.count("{") - line.count("}")
            continue
        if not in_provider:
            continue
        depth += line.count("{") - line.count("}")
        if line.startswith("api_key") and not api_key:
            api_key = _quoted(line)
        elif line.startswith("base_url") and not host:
            host = _quoted(line).rstrip("/")
        elif line.startswith('models "') and not upstream_model:
            upstream_model = line.split('"', 2)[1]
        if depth <= 0:
            in_provider = False
    model_id = f"deepseek/{upstream_model}" if upstream_model else ""
    if default_model != model_id or not model_id.startswith("deepseek/"):
        raise ValueError("acl default_model is not provider deepseek")
    if "maas.aliyuncs.com" not in host or host.startswith("http://"):
        raise ValueError("acl deepseek host is not the Aliyun MaaS host")
    if not api_key:
        raise ValueError("acl deepseek provider has no api key")
    return Route(model_id=model_id, upstream_model=upstream_model, host=host, api_key=api_key)


class ModelProxy:
    """Loopback origin whose chat completions use ``route``."""

    def __init__(self, route: Route, evidence: Path, upstream: str | None = None, gateway: str = GATEWAY) -> None:
        self.route = route
        self.evidence = evidence
        self.upstream = (upstream or route.host).rstrip("/")
        self.gateway = gateway.rstrip("/")
        self._httpd: ThreadingHTTPServer | None = None
        self._thread: threading.Thread | None = None
        self.origin = ""

    def start(self) -> str:
        proxy = self
        gateway = urllib.parse.urlsplit(self.gateway)

        class Handler(BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, fmt: str, *args) -> None:
                return

            def _body(self) -> bytes:
                length = int(self.headers.get("Content-Length") or "0")
                return self.rfile.read(length) if length else b""

            def _write(self, status: int, reason: str, content_type: str, payload: bytes) -> None:
                self.send_response(status, reason)
                self.send_header("Content-Type", content_type)
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

            def _stream(self, status: int, reason: str, content_type: str, source) -> None:
                self.send_response(status, reason)
                self.send_header("Content-Type", content_type)
                self.send_header("Transfer-Encoding", "chunked")
                self.end_headers()
                while True:
                    chunk = source.read(8192)
                    if not chunk:
                        break
                    self.wfile.write(f"{len(chunk):X}\r\n".encode() + chunk + b"\r\n")
                    self.wfile.flush()
                self.wfile.write(b"0\r\n\r\n")
                self.wfile.flush()

            def _serve_model(self, body: bytes) -> None:
                status = 0
                try:
                    payload = json.loads(body.decode() or "{}")
                    if not isinstance(payload, dict):
                        raise ValueError("chat body is not an object")
                    payload["model"] = proxy.route.upstream_model
                    encoded = json.dumps(payload).encode()
                    target = urllib.parse.urlsplit(proxy.upstream)
                    path = (target.path or "") + "/chat/completions"
                    port = target.port or (443 if target.scheme == "https" else 80)
                    connection_cls = http.client.HTTPSConnection if target.scheme == "https" else http.client.HTTPConnection
                    connection = connection_cls(target.hostname, port, timeout=600)
                    connection.request(
                        "POST",
                        path,
                        body=encoded,
                        headers={
                            "Authorization": f"Bearer {proxy.route.api_key}",
                            "Content-Type": "application/json",
                            "Accept": self.headers.get("Accept") or "application/json",
                            "Content-Length": str(len(encoded)),
                        },
                    )
                    response = connection.getresponse()
                    status = response.status
                    proxy.record(status)
                    content_type = response.getheader("Content-Type") or "application/json"
                    self._stream(status, response.reason, content_type, response)
                except Exception as exc:
                    if status == 0:
                        proxy.record(502)
                    message = str(exc).replace(proxy.route.api_key, "[redacted]")
                    if not self.wfile.closed:
                        try:
                            self._write(502, "Bad Gateway", "application/json", json.dumps({"error": message}).encode())
                        except Exception:
                            pass

            def _serve_gateway(self, body: bytes) -> None:
                target = urllib.parse.urlsplit(self.path)
                path = target.path + (f"?{target.query}" if target.query else "")
                connection = http.client.HTTPConnection(gateway.hostname, gateway.port or 80, timeout=60)
                headers = {"Host": f"{gateway.hostname}:{gateway.port or 80}"}
                for name in ("Authorization", "Content-Type", "Accept"):
                    value = self.headers.get(name)
                    if value:
                        headers[name] = value
                if body:
                    headers["Content-Length"] = str(len(body))
                connection.request(self.command, path, body=body, headers=headers)
                response = connection.getresponse()
                content_type = response.getheader("Content-Type") or "application/octet-stream"
                self._stream(response.status, response.reason, content_type, response)

            def _route(self) -> None:
                body = self._body()
                path = urllib.parse.urlsplit(self.path).path.rstrip("/")
                if self.command == "POST" and path.endswith("/chat/completions"):
                    self._serve_model(body)
                    return
                self._serve_gateway(body)

            def do_GET(self) -> None:
                self._route()

            def do_POST(self) -> None:
                self._route()

            def do_PUT(self) -> None:
                self._route()

            def do_PATCH(self) -> None:
                self._route()

            def do_DELETE(self) -> None:
                self._route()

            def do_OPTIONS(self) -> None:
                self._route()

        class QuietServer(ThreadingHTTPServer):
            def handle_error(self, request, client_address) -> None:
                return

        server = QuietServer(("127.0.0.1", 0), Handler)
        self._httpd = server
        self.origin = f"http://127.0.0.1:{server.server_address[1]}"
        self._thread = threading.Thread(target=server.serve_forever, name="catalog-model-proxy", daemon=True)
        self._thread.start()
        return self.origin

    def record(self, status: int) -> None:
        row = {"model_id": self.route.model_id, "host": self.route.host, "status": status}
        line = json.dumps(row, ensure_ascii=False)
        if self.route.api_key and self.route.api_key in line:
            raise RuntimeError("refusing to record the api key")
        self.evidence.parent.mkdir(parents=True, exist_ok=True)
        with self.evidence.open("a", encoding="utf-8") as handle:
            handle.write(line + "\n")

    def observed(self) -> list[dict]:
        if not self.evidence.is_file():
            return []
        rows = []
        for line in self.evidence.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            row = json.loads(line)
            if self.route.api_key and self.route.api_key in line:
                raise RuntimeError("evidence contains the api key")
            rows.append(row)
        return rows

    def stop(self) -> None:
        if self._httpd is not None:
            self._httpd.shutdown()
            self._httpd.server_close()
            self._httpd = None
