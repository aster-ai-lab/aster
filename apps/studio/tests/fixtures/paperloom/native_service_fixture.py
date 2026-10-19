#!/usr/bin/env python3
"""Owned native-UI fixture. No OCR/model execution: every result is controlled test data."""
import argparse
import hashlib
import json
import threading
from email import policy
from email.parser import BytesParser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

KEY = "paperloom-provenance-native-fixture"
JOB = "native-provenance-job"
ARTIFACT = "translated-pdf"
NAME = "paper-native-traced.pdf"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    if not (root / ".owned").is_file():
        parser.error("an explicitly owned test root is required")
    source = (root / "workspace/paper.pdf").read_bytes()
    if not source.startswith(b"%PDF-") or len(source) > 20 * 1024 * 1024:
        parser.error("the owned fixture source must be a bounded PDF")
    source_id = hashlib.sha256(source).hexdigest()
    output = source + b"\n% controlled native provenance output fixture\n"
    output_sha = "sha256:" + hashlib.sha256(output).hexdigest()
    record = {
        "document_id": source_id, "source_filename": "paper.pdf", "title": "paper.pdf",
        "page_count": 2, "bytes": len(source), "active_job_id": JOB, "reading_status": "unread",
        "added_at": "2026-10-16T00:00:00Z", "updated_at": "2026-10-16T00:00:00Z", "tags": [],
    }

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def send(self, status, data, content_type="application/json"):
            payload = data if isinstance(data, bytes) else json.dumps(data).encode()
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def data(self, value):
            self.send(200, {"code": 0, "data": value})

        def allowed(self):
            if self.headers.get("X-API-Key") == KEY:
                return True
            self.send(401, {"error": "fixture_key_required"})
            return False

        def do_GET(self):
            path = self.path.split("?", 1)[0]
            if path == "/health":
                self.send(200, {"ready": True, "fixture": True})
                return
            if not self.allowed():
                return
            if path == f"/api/v1/documents/{source_id}":
                self.data(record)
            elif path == "/api/v1/documents":
                self.data({"documents": [record], "total": 1})
            elif path == f"/api/v1/documents/{source_id}/source.pdf":
                self.send(200, source, "application/pdf")
            elif path == f"/api/v1/documents/{source_id}/jobs":
                stage = {"state": "completed", "progress": {"current": 2, "total": 2, "percent": 100}}
                self.data({"items": [{"job_id": JOB, "workflow": "ocr", "status": "succeeded", "attempt": 1,
                    "stages": {"ocr": stage, "translation": stage, "render": stage}, "output_pdf_ready": True}]})
            elif path == f"/api/v1/jobs/{JOB}":
                self.data({"job_id": JOB, "status": "succeeded", "workflow": "ocr",
                    "request_payload": {"workflow": "ocr", "source": {"upload_id": "20261016-opaque-fixture-upload"}}})
            elif path == f"/api/v1/jobs/{JOB}/artifacts-manifest":
                self.data({"job_id": JOB, "items": [{"artifact_key": ARTIFACT, "artifact_kind": "file",
                    "file_name": NAME, "content_type": "application/pdf", "ready": True,
                    "attempt": 1, "checksum": output_sha, "size_bytes": len(output)},
                    {"artifact_key": "source_pdf", "artifact_kind": "file", "file_name": "paper.pdf",
                     "content_type": "application/pdf", "ready": True, "attempt": 1,
                     "checksum": "sha256:" + source_id, "size_bytes": len(source)}]})
            elif path == f"/api/v1/jobs/{JOB}/artifacts/source_pdf":
                self.send(200, source, "application/pdf")
            elif path in (f"/api/v1/jobs/{JOB}/artifacts/{ARTIFACT}", f"/api/v1/jobs/{JOB}/pdf"):
                self.send(200, output, "application/pdf")
            else:
                self.send(404, {"error": "fixture_route_not_found"})

        def do_POST(self):
            if not self.allowed():
                return
            if self.path == "/_stop":
                self.send(200, {"stopping_owned_fixture": True})
                threading.Thread(target=self.server.shutdown, daemon=True).start()
                return
            length = int(self.headers.get("Content-Length", "0"))
            if length < 1 or length > 20 * 1024 * 1024 + 4096:
                self.send(413, {"error": "fixture_upload_limit"})
                return
            body = self.rfile.read(length)
            if self.path.split("?", 1)[0] == "/api/v1/uploads":
                message = BytesParser(policy=policy.default).parsebytes(
                    b"MIME-Version: 1.0\r\nContent-Type: " + self.headers.get("Content-Type", "").encode() + b"\r\n\r\n" + body)
                parts = [part for part in message.iter_parts() if part.get_param("name", header="Content-Disposition") == "file"] if message.is_multipart() else []
                if len(parts) != 1 or parts[0].get_payload(decode=True) != source:
                    self.send(409, {"error": "unexpected_fixture_source"})
                    return
                self.data({"document_id": source_id, "upload_id": "20261016-opaque-fixture-upload", "page_count": 2})
            else:
                self.send(404, {"error": "fixture_route_not_found"})

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    receipt = root / "provenance-fixture.port"
    receipt.write_text(str(server.server_port))
    print(json.dumps({"fixture": True, "port": server.server_port, "source_sha256": source_id,
                      "output_sha256": output_sha, "output_bytes": len(output)}), flush=True)
    try:
        server.serve_forever(poll_interval=0.1)
    finally:
        server.server_close()
        receipt.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
