"""Test-only driver for the actual ACP workspace-search extension."""

import hashlib
import json
import os
import queue
import subprocess
import tempfile
import threading
import time
from pathlib import Path, PurePosixPath


SCHEMA = "aster.acp.workspace-search.v1"
TIMEOUT = 30
MAX_REPLY_BYTES = 1024 * 1024
MAX_OBSERVATION_BYTES = 64 * 1024 * 1024


class EngineError(Exception):
    pass


class AcpEngine:
    def __init__(self, command, corpus, semantic=False, trace_directory=None):
        self.command = command
        self.corpus = Path(corpus).resolve(strict=True)
        self.semantic = semantic
        self.trace_directory = Path(trace_directory).resolve() if trace_directory else None
        self.process = None
        self.reader = None
        self.home = None
        self.replies = queue.Queue(maxsize=32)
        self.stopping = threading.Event()
        self.sequence = 0
        self.observations = []
        self.observation_bytes = 0
        self.timings = {}
        self.session = None
        self.agent_info = None

    def __enter__(self):
        try:
            self.home = tempfile.TemporaryDirectory(prefix="aster-retrieval-home-")
            home = Path(self.home.name)
            config = home / "config.acl"
            text = (
                'import_accounts = []\ndefault_model = "fixture/unused"\n'
                'providers "fixture" { api_key = "synthetic-not-a-credential" '
                'base_url = "http://127.0.0.1:9/v1" models "unused" {} }\n'
                f'memory_dir = {json.dumps(str(home / "memory"))}\n'
                'tool_presentation { mode = "direct" }\n'
            )
            if self.semantic:
                text += "workspace_retrieval { enabled = true\n local_cpu {}\n}\n"
            config.write_text(text, encoding="utf-8")
            env = os.environ.copy()
            for name in ("ASTER_CONFIG", "ASTER_DEFAULT_MODEL", "ASTER_SESSION_DIR"):
                env.pop(name, None)
            env.update({
                "HOME": str(home), "USERPROFILE": str(home),
                "ASTER_HOME": str(home / ".aster"), "GROK_HOME": str(home / "grok"),
                "XDG_CONFIG_HOME": str(home / ".config"),
                "APPDATA": str(home / "roaming"), "LOCALAPPDATA": str(home / "local"),
                "ASTER_LOG_FILE": "0",
            })
            if self.trace_directory is not None:
                self.trace_directory.mkdir(parents=True, exist_ok=True)
                env["ASTER_LOG_DIR"] = str(self.trace_directory)
                env["ASTER_LOG_FILE"] = "aster_acp::retrieval_phase=debug,aster_core::retrieval_phase=debug"
            started = time.monotonic()
            self.process = subprocess.Popen(
                [*self.command, "--cwd", str(self.corpus), "--config", str(config)],
                cwd=self.corpus, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
            )
            self.reader = threading.Thread(target=self._read_stdout, daemon=True)
            self.reader.start()
            self.timings["spawn_ms"] = (time.monotonic() - started) * 1000
            started = time.monotonic()
            initialized = self.rpc("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
            self.agent_info = initialized.get("agentInfo")
            self.timings["initialize_ms"] = (time.monotonic() - started) * 1000
            started = time.monotonic()
            opened = self.rpc("session/new", {"cwd": str(self.corpus), "mcpServers": []})
            self.session = opened.get("sessionId")
            if not isinstance(self.session, str) or not self.session:
                raise EngineError("ACP session/new omitted sessionId")
            self.timings["new_session_ms"] = (time.monotonic() - started) * 1000
            return self
        except BaseException:
            self.close()
            raise

    def __exit__(self, *_):
        self.close()

    def close(self):
        self.stopping.set()
        if self.process is not None:
            if self.process.poll() is None:
                self.process.kill()
            self.process.wait(timeout=10)
            if self.reader is not None:
                self.reader.join(timeout=2)
            for pipe in (self.process.stdin, self.process.stdout):
                if pipe is not None:
                    pipe.close()
        if self.home is not None:
            self.home.cleanup()

    def _read_stdout(self):
        while not self.stopping.is_set():
            try:
                line = self.process.stdout.readline(MAX_REPLY_BYTES + 1)
                if not line:
                    raise EngineError("ACP stdout closed before a reply")
                if len(line) > MAX_REPLY_BYTES:
                    raise EngineError("ACP reply exceeds the 1 MiB test-driver limit")
                value = json.loads(line.decode("utf-8"))
                if not isinstance(value, dict):
                    raise EngineError("ACP reply is not a JSON object")
            except (EngineError, UnicodeError, ValueError) as error:
                value = EngineError(f"invalid ACP output: {error}")
            while not self.stopping.is_set():
                try:
                    self.replies.put(value, timeout=0.05)
                    break
                except queue.Full:
                    continue
            if isinstance(value, EngineError):
                return

    def rpc(self, method, params, deadline=None):
        self.sequence += 1
        request_id = self.sequence
        deadline = deadline if deadline is not None else time.monotonic() + TIMEOUT
        wire = json.dumps({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        try:
            self.process.stdin.write(wire.encode("utf-8") + b"\n")
            self.process.stdin.flush()
        except OSError as error:
            raise EngineError(f"cannot send ACP {method}: {error}") from error
        for _ in range(4096):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise EngineError(f"ACP {method} timed out")
            try:
                value = self.replies.get(timeout=remaining)
            except queue.Empty as error:
                raise EngineError(f"ACP {method} timed out") from error
            if isinstance(value, EngineError):
                raise value
            if value.get("method"):
                if "id" in value:
                    raise EngineError(f"unexpected ACP host request: {value['method']}")
                continue
            if value.get("id") != request_id:
                raise EngineError("ACP response ID does not match the current request")
            if "error" in value:
                raise EngineError(f"ACP {method} failed: {json.dumps(value['error'])}")
            result = value.get("result")
            if not isinstance(result, dict):
                raise EngineError(f"ACP {method} omitted its result object")
            return result
        raise EngineError("ACP notification limit exceeded")

    def search(self, query):
        deadline = time.monotonic() + TIMEOUT
        while True:
            operation = f"retrieval-bench-{self.sequence + 1}"
            started = time.monotonic()
            result = self.rpc("_aster/workspace/search/v1", {
                "schema": SCHEMA, "sessionId": self.session, "operationId": operation,
                "cwd": str(self.corpus), "query": query,
            }, deadline)
            cwd = result.get("cwd")
            if (result.get("schema") != SCHEMA or result.get("sessionId") != self.session
                    or result.get("operationId") != operation
                    or not isinstance(cwd, str) or Path(cwd).resolve() != self.corpus):
                raise EngineError("workspace-search reply has the wrong source/session identity")
            metadata = result.get("metadata")
            if not isinstance(metadata, dict) or not isinstance(metadata.get("results"), list):
                raise EngineError("workspace-search reply omitted ranked result metadata")
            observation = {
                "operation_id": operation, "elapsed_ms": (time.monotonic() - started) * 1000,
                "query": query, "metadata": metadata,
            }
            self.observation_bytes += len(json.dumps(observation, ensure_ascii=False).encode("utf-8"))
            if self.observation_bytes > MAX_OBSERVATION_BYTES:
                raise EngineError("ACP observations exceed the 64 MiB test-driver limit")
            self.observations.append(observation)
            status = metadata.get("semantic_status")
            if status is not None and not isinstance(status, dict):
                raise EngineError("workspace-search reply has invalid semantic status")
            phase = (status or {}).get("phase")
            if self.semantic and phase != "ready":
                if phase != "building":
                    raise EngineError(f"requested semantic index is unavailable: {phase}")
            lexical_engine = metadata.get("lexical_engine")
            if lexical_engine not in ("portable_bm25_v1", "aster_vec_fts_v1"):
                raise EngineError("workspace-search reply has an invalid lexical engine")
            if (lexical_engine == "aster_vec_fts_v1" and metadata.get("source_revision", 0) > 0
                    and (not self.semantic or phase == "ready")):
                break
            if time.monotonic() >= deadline:
                raise EngineError("workspace-search index did not become ready")
            time.sleep(min(0.02, max(0, deadline - time.monotonic())))
        hits = metadata["results"]
        for hit in hits:
            if not isinstance(hit, dict):
                raise EngineError("search hit is not an object")
            path = hit.get("path")
            start = hit.get("start_line")
            end = hit.get("end_line")
            if not isinstance(path, str) or not path:
                raise EngineError("search hit omitted its source path")
            relative = PurePosixPath(path)
            if relative.is_absolute() or ".." in relative.parts:
                raise EngineError("search hit escaped the corpus")
            if not (self.corpus / Path(path)).resolve().is_relative_to(self.corpus):
                raise EngineError("search hit escaped the corpus")
            if (type(start) is not int or type(end) is not int or start < 1 or end < start
                    or hit.get("digest_verified") is not True):
                raise EngineError("search hit has invalid/unverified source coordinates")
        return hits

    def rank(self, query, _corpus=None):
        # The legacy corpus grades file:startline. Keep the actual returned
        # snippet anchor; consulting gold lines here would inflate its score.
        return [f"{hit['path']}:{hit['start_line']}" for hit in self.search(query)]

    def identity(self):
        binary = Path(self.command[0])
        hasher = hashlib.sha256()
        with binary.open("rb") as source:
            for chunk in iter(lambda: source.read(MAX_REPLY_BYTES), b""):
                hasher.update(chunk)
        return {
            "protocol": "acp-stdio", "binary_sha256": hasher.hexdigest(),
            "agent_info": self.agent_info, "mode": "hybrid", "semantic_opt_in": self.semantic,
            "qualification_lexical_engine": "aster_vec_fts_v1",
            "ranking_granularity": "returned-snippet-start-line", "timings_ms": self.timings,
            "observations": self.observations,
        }
