"""The catalog model proxy rewrites chat onto the ACL route and does not store the key."""

from __future__ import annotations

import json
import threading
import unittest
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import tempfile

from model_route import ModelProxy, Route, load_route


ACL = Path(__file__).resolve().parent.parents[4] / ".a3s" / "config.acl"


class ModelRouteTest(unittest.TestCase):
    def test_acl_selects_the_aliyun_deepseek_model(self) -> None:
        route = load_route(ACL)
        self.assertEqual(route.model_id, "deepseek/deepseek-v4.1-flash")
        self.assertEqual(route.upstream_model, "deepseek-v4.1-flash")
        self.assertIn("maas.aliyuncs.com", route.host)
        self.assertTrue(route.host.startswith("https://"))
        self.assertNotIn("35.220.164.252", route.host)
        self.assertNotIn(route.api_key, route.model_id)
        self.assertNotIn(route.api_key, route.host)
        self.assertTrue(route.api_key)

    def test_chat_is_rewritten_and_the_key_stays_out_of_evidence(self) -> None:
        seen: dict[str, str] = {}

        class Upstream(BaseHTTPRequestHandler):
            def log_message(self, fmt: str, *args) -> None:
                return

            def do_POST(self) -> None:
                length = int(self.headers.get("Content-Length") or "0")
                body = json.loads(self.rfile.read(length).decode())
                seen["model"] = body["model"]
                seen["authorization"] = self.headers.get("Authorization") or ""
                seen["path"] = self.path
                payload = b"data: ok\n\n"
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

        upstream = ThreadingHTTPServer(("127.0.0.1", 0), Upstream)
        thread = threading.Thread(target=upstream.serve_forever, daemon=True)
        thread.start()
        port = upstream.server_address[1]
        route = Route(
            model_id="deepseek/deepseek-v4.1-flash",
            upstream_model="deepseek-v4.1-flash",
            host="https://llm-example.maas.aliyuncs.com/compatible-mode/v1",
            api_key="sk-test-catalog-secret",
        )
        proxy = None
        try:
            with tempfile.TemporaryDirectory() as directory:
                evidence = Path(directory) / "model-live.jsonl"
                proxy = ModelProxy(route, evidence, upstream=f"http://127.0.0.1:{port}/compatible-mode/v1")
                origin = proxy.start()
                body = json.dumps({"model": "deepseek-v4-flash", "messages": [{"role": "user", "content": "hi"}]}).encode()
                request = urllib.request.Request(
                    origin + "/v1/chat/completions",
                    data=body,
                    headers={"Authorization": "Bearer member-token", "Content-Type": "application/json"},
                )
                with urllib.request.urlopen(request, timeout=10) as response:
                    text = response.read().decode()
                    status = response.status
                self.assertEqual(status, 200)
                self.assertIn("data: ok", text)
                self.assertEqual(seen["model"], "deepseek-v4.1-flash")
                self.assertEqual(seen["path"], "/compatible-mode/v1/chat/completions")
                self.assertEqual(seen["authorization"], "Bearer sk-test-catalog-secret")
                self.assertNotIn("member-token", seen["authorization"])
                rows = proxy.observed()
                self.assertEqual(rows, [{
                    "model_id": "deepseek/deepseek-v4.1-flash",
                    "host": "https://llm-example.maas.aliyuncs.com/compatible-mode/v1",
                    "status": 200,
                }])
                stored = evidence.read_text(encoding="utf-8")
                self.assertNotIn(route.api_key, stored)
                self.assertNotIn("member-token", stored)
        finally:
            if proxy is not None:
                proxy.stop()
            upstream.shutdown()
            upstream.server_close()


if __name__ == "__main__":
    unittest.main()
