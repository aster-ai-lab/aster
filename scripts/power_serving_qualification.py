#!/usr/bin/env python3
"""Bounded OpenAI serving qualification; benchmark data, never product configuration.

Records timing and authoritative usage without persisting prompts, credentials or
generated content. Run both arms against the same synthetic corpus and immutable
provenance before using the comparison for a deployment decision.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import dataclasses
import hashlib
import http.client
import ipaddress
import json
import math
import os
from pathlib import Path
import socket
import threading
import time
from urllib.parse import urlsplit


MAX_FRAME = 1024 * 1024
MAX_RESPONSE = 16 * 1024 * 1024
MAX_U64 = 2**64 - 1
PROVENANCE_FIELDS = (
    "arm", "source_head", "source_diff_sha256", "binary_sha256", "model_sha256",
    "runtime_sha256", "config_redacted_sha256", "corpus_sha256", "context_per_sequence",
    "sampling", "cache_mode", "slots", "gpu_indices", "runtime_flags",
)
PROVENANCE_OPTIONAL_FIELDS = (
    "observed_at_utc", "tokenizer_sha256", "template_sha256", "profile",
    "build_features", "warmup_requests", "runtime_version", "gpu_topology_sha256",
    "admission", "cpu_affinity", "numa_policy", "speculation",
)
MATCH_FIELDS = (
    "model_sha256", "corpus_sha256", "context_per_sequence", "sampling", "cache_mode",
    "slots", "gpu_indices", "runtime_sha256", "runtime_flags",
)
OPTIONAL_MATCH_FIELDS = (
    "tokenizer_sha256", "template_sha256", "profile", "warmup_requests", "admission",
    "cpu_affinity", "numa_policy", "speculation", "gpu_topology_sha256",
)
EXPERIMENT_FACTORS = ("slots", "cache_mode", "runtime_sha256", "runtime_flags", "admission", "cpu_affinity", "numa_policy", "speculation")
RUNTIME_FLAG_NAMES = frozenset((
    "--model", "-m", "--host", "--port", "--ctx-size", "-c", "--parallel", "-np",
    "--batch-size", "-b", "--ubatch-size", "-ub", "--n-gpu-layers", "-ngl",
    "--flash-attn", "-fa", "--threads", "-t", "--threads-batch", "-tb", "--reasoning",
    "--alias", "--jinja", "--cache-prompt", "--cache-ram", "--cache-reuse", "--metrics",
    "--slots", "--cont-batching", "--spec-type", "--spec-draft-n-max", "--spec-draft-n-min",
    "--spec-ngram-mod-n-min", "--spec-ngram-mod-n-max", "--spec-ngram-mod-n-match",
    "--cache-type-k", "--cache-type-v",
))


@dataclasses.dataclass(frozen=True)
class Limits:
    seconds: float = 60
    request_timeout: float = 120
    concurrency: int = 1
    max_requests: int = 10000

    def validate(self):
        if not (0 < self.seconds <= 3600 and 0 < self.request_timeout <= 600):
            raise ValueError("duration and request deadline must be finite and bounded")
        if not (1 <= self.concurrency <= 64 and 1 <= self.max_requests <= 100000):
            raise ValueError("concurrency/request budget exceeds qualification bounds")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def percentile(values, fraction):
    """Linear interpolation; empty samples remain unknown."""
    if not values:
        return None
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    lo, hi = math.floor(position), math.ceil(position)
    return ordered[lo] + (ordered[hi] - ordered[lo]) * (position - lo)


def valid_usage(value):
    if not isinstance(value, dict):
        return None
    fields = ("prompt_tokens", "completion_tokens", "total_tokens")
    if not all(type(value.get(k)) is int and 0 <= value[k] <= MAX_U64 for k in fields):
        return None
    if value["prompt_tokens"] + value["completion_tokens"] != value["total_tokens"]:
        return None
    result = {k: value[k] for k in fields}
    details = value.get("prompt_tokens_details")
    cached = details.get("cached_tokens") if isinstance(details, dict) else None
    if type(cached) is int and 0 <= cached <= result["prompt_tokens"]:
        result["cached_tokens"] = cached
    return result


def extract_usage(payload):
    observation = payload.get("prism_upstream", {})
    if isinstance(observation, dict) and observation.get("source") == "upstream-reported":
        usage = valid_usage(observation.get("usage"))
        if usage is not None:
            return usage, "upstream-usage"
        timings = observation.get("timings", {})
        if isinstance(timings, dict):
            prompt, cached, predicted = (timings.get(k) for k in ("prompt_n", "cache_n", "predicted_n"))
            if all(type(n) is int and n >= 0 for n in (prompt, cached, predicted)):
                usage = valid_usage({"prompt_tokens": prompt + cached,
                                     "completion_tokens": predicted,
                                     "total_tokens": prompt + cached + predicted})
                if usage is not None:
                    usage["cached_tokens"] = cached
                    return usage, "upstream-timings"
    usage = valid_usage(payload.get("usage"))
    # Caller-facing usage is retained, but not silently certified as engine truth.
    return usage, "endpoint-usage" if usage is not None else "unavailable"


class Output:
    def __init__(self, started):
        self.started = started
        self.first = None
        self.last = None
        self.first_tool = None
        self.delta_count = 0
        self.usage = None
        self.usage_source = "unavailable"
        self.finished = False
        self.terminal = False
        self.digest = hashlib.sha256()
        self.tools = {}
        self.upstream_decode_rate = None
        self.wire_usage = None
        self.text = []
        self.finish_reason = None

    def observe(self, payload, now):
        if not isinstance(payload, dict) or "error" in payload:
            raise ValueError("invalid or failed upstream response")
        wire = valid_usage(payload.get("usage"))
        if wire is not None:
            self.wire_usage = wire
        usage, source = extract_usage(payload)
        if usage is not None and (self.usage is None or source.startswith("upstream-")):
            self.usage, self.usage_source = usage, source
        observation = payload.get("prism_upstream", {})
        timings = observation.get("timings", {}) if isinstance(observation, dict) and observation.get("source") == "upstream-reported" else {}
        if not timings and isinstance(payload.get("timings"), dict):
            timings = payload["timings"]
        rate = timings.get("predicted_per_second") if isinstance(timings, dict) else None
        if type(rate) in (int, float) and math.isfinite(rate) and rate > 0:
            self.upstream_decode_rate = rate
        meaningful = False
        choices = payload.get("choices", [])
        if not isinstance(choices, list):
            raise ValueError("invalid choices")
        was_finished = self.finished
        for choice in choices:
            if not isinstance(choice, dict):
                raise ValueError("invalid choice")
            if choice.get("index", 0) != 0:
                raise ValueError("multi-choice result outside single-choice benchmark")
            if choice.get("finish_reason") is not None:
                if choice["finish_reason"] not in ("stop", "length", "tool_calls", "function_call", "content_filter"):
                    raise ValueError("unknown finish reason")
                if self.finished:
                    raise ValueError("duplicate finish")
                self.finished = True
                self.finish_reason = choice["finish_reason"]
            delta = choice.get("delta", choice.get("message", {}))
            if not isinstance(delta, dict):
                raise ValueError("invalid delta")
            for field in ("content", "reasoning_content"):
                text = delta.get(field)
                if isinstance(text, str) and text:
                    meaningful = True
                    self.digest.update(text.encode())
                    if field == "content":
                        self.text.append(text)
            calls = delta.get("tool_calls") or []
            if not isinstance(calls, list):
                raise ValueError("invalid tool calls")
            for ordinal, call in enumerate(calls):
                if not isinstance(call, dict):
                    raise ValueError("invalid tool call")
                index = call.get("index", ordinal)
                if type(index) is not int or not 0 <= index <= 63:
                    raise ValueError("invalid tool index")
                tool = self.tools.setdefault(index, {"id": "", "name": "", "arguments": ""})
                identifier = call.get("id", "")
                if identifier:
                    if tool["id"] and tool["id"] != identifier:
                        raise ValueError("tool identity changed")
                    tool["id"] = identifier
                function = call.get("function", {})
                if not isinstance(function, dict):
                    raise ValueError("invalid tool function")
                for field, target in (("name", "name"), ("arguments", "arguments")):
                    fragment = function.get(field, "")
                    if not isinstance(fragment, str):
                        raise ValueError("invalid tool fragment")
                    tool[target] += fragment
                    if len(tool[target]) > MAX_FRAME:
                        raise ValueError("tool exceeds bounded output")
                    if fragment:
                        self.digest.update(fragment.encode())
                        meaningful = True
                        self.first_tool = self.first_tool or now
        if meaningful:
            if was_finished:
                raise ValueError("output after finish")
            self.delta_count += 1
            self.first = self.first or now
            self.last = now

    def record(self, outcome, status, ended, error=None):
        tool_valid = None
        if self.tools:
            tool_valid = all(t["id"] and t["name"] == "bench_sum"
                             and _expected_arguments(t["arguments"]) for t in self.tools.values())
        tokens = self.usage.get("completion_tokens") if self.usage else None
        duration = ended - self.started
        request_rate = tokens / duration if tokens is not None and duration > 0 and self.usage_source.startswith("upstream-") else None
        return {
            "outcome": outcome, "http_status": status, "error": error,
            "elapsed_ms": 1000 * (ended - self.started),
            "ttft_ms": 1000 * (self.first - self.started) if self.first else None,
            "first_tool_ms": 1000 * (self.first_tool - self.started) if self.first_tool else None,
            "usage": self.usage, "usage_source": self.usage_source,
            "wire_usage": self.wire_usage,
            "wire_usage_mismatch": self.wire_usage is not None and self.usage is not None and any(self.wire_usage[k] != self.usage[k] for k in ("prompt_tokens", "completion_tokens", "total_tokens")),
            "client_request_output_tokens_per_second": request_rate,
            "upstream_decode_tokens_per_second": self.upstream_decode_rate,
            "output_delta_interval_ms": 1000 * (self.last - self.first) if self.first and self.last else None,
            "meaningful_deltas": self.delta_count, "finish_observed": self.finished,
            "terminal_observed": self.terminal, "output_sha256": self.digest.hexdigest(),
            "tool_calls": len(self.tools), "tool_valid": tool_valid,
        }


def _expected_arguments(arguments):
    try:
        value = json.loads(arguments)
        return (isinstance(value, dict) and value == {"a": 19, "b": 23}
                and type(value["a"]) is int and type(value["b"]) is int)
    except (ValueError, TypeError):
        return False


class _DeadlineHTTPSConnection(http.client.HTTPSConnection):
    def connect(self):
        # Publish the SSL socket before its blocking handshake so the absolute
        # deadline watchdog can interrupt it as well as headers/body reads.
        http.client.HTTPConnection.connect(self)
        self.sock = self._context.wrap_socket(self.sock, server_hostname=self.host, do_handshake_on_connect=False)
        self.sock.do_handshake()


def request_once(base_url, body, key, timeout, cancel_after_ms=None, cancel_after_deltas=None, expected=None):
    if type(timeout) not in (int, float) or not math.isfinite(timeout) or not 0 < timeout <= 600:
        raise ValueError("invalid request timeout")
    if cancel_after_ms is not None and (type(cancel_after_ms) not in (int, float) or not math.isfinite(cancel_after_ms) or not 0 < cancel_after_ms <= timeout * 1000):
        raise ValueError("invalid cancellation deadline")
    if cancel_after_deltas is not None and (type(cancel_after_deltas) is not int or not 1 <= cancel_after_deltas <= 1000000):
        raise ValueError("invalid cancellation delta bound")
    parsed = urlsplit(base_url)
    if parsed.scheme not in ("http", "https") or not parsed.hostname or parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise ValueError("endpoint must be HTTP(S) without credentials/query/fragment")
    try:
        ipaddress.ip_address(parsed.hostname)
    except ValueError as exc:
        raise ValueError("qualification endpoints require literal IPs for bounded DNS-free connect") from exc
    started = time.monotonic()
    deadline = started + timeout
    cancel_deadline = started + cancel_after_ms / 1000 if cancel_after_ms is not None else None
    effective_deadline = min(deadline, cancel_deadline) if cancel_deadline else deadline
    state, status, outcome, error = Output(started), None, "error", None
    connection_type = _DeadlineHTTPSConnection if parsed.scheme == "https" else http.client.HTTPConnection
    connection = connection_type(parsed.hostname, parsed.port, timeout=max(.001, effective_deadline - started))
    response = None
    expired = threading.Event()
    transport_box = [None]

    def expire():
        expired.set()
        transport = connection.sock or transport_box[0]
        if transport is not None:
            try:
                transport.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass

        # HTTPResponse owns its parser/file object on the reader thread.
        # Concurrent HTTPConnection.close() can clear response.fp during
        # chunked parsing. Socket shutdown wakes the reader; finally closes it.

    watchdog = threading.Timer(max(.001, effective_deadline - time.monotonic()), expire)
    watchdog.daemon = True
    watchdog.start()
    try:
        connection.connect()
        transport = connection.sock
        transport_box[0] = transport

        def remaining():
            left = effective_deadline - time.monotonic()
            if left <= 0:
                raise socket.timeout()
            transport.settimeout(left)

        headers = {"Content-Type": "application/json", "Accept": "text/event-stream" if body.get("stream") else "application/json"}
        if key:
            headers["Authorization"] = "Bearer " + key
        path = parsed.path.rstrip("/") + "/chat/completions"
        remaining()
        connection.request("POST", path, body=json.dumps(body).encode(), headers=headers)
        remaining()
        response = connection.getresponse()
        status = response.status
        if status != 200:
            outcome = "refused" if status == 429 else "http_error"
        elif not body.get("stream"):
            remaining()
            raw = response.read(MAX_RESPONSE + 1)
            if len(raw) > MAX_RESPONSE:
                raise ValueError("response exceeds limit")
            state.observe(json.loads(raw), time.monotonic())
            state.terminal = True
            outcome = "success" if state.finished else "invalid_stream"
        else:
            event, frame_size, total = [], 0, 0
            while True:
                remaining()
                line = response.readline(MAX_FRAME + 1)
                if not line:
                    raise ValueError("stream ended without terminal")
                total += len(line)
                frame_size += len(line)
                if frame_size > MAX_FRAME or total > MAX_RESPONSE:
                    raise ValueError("stream exceeds bounded frame/response")
                line = line.rstrip(b"\r\n")
                if line:
                    if line.startswith(b"data:"):
                        event.append(line[5:].removeprefix(b" "))
                    continue
                frame_size = 0
                if not event:
                    continue
                raw, event = b"\n".join(event), []
                if raw == b"[DONE]":
                    state.terminal = True
                    outcome = "success" if state.finished else "invalid_stream"
                    break
                state.observe(json.loads(raw), time.monotonic())
                if cancel_after_deltas is not None and state.delta_count >= cancel_after_deltas:
                    outcome = "cancelled"
                    break
    except (TimeoutError, socket.timeout):
        outcome = "cancelled" if cancel_deadline else "timeout"
    except (OSError, http.client.HTTPException, ValueError, TypeError, KeyError) as exc:
        error = type(exc).__name__  # Messages can echo headers or server content.
    finally:
        watchdog.cancel()
        if response:
            try:
                response.close()
            except OSError:
                pass
        connection.close()
    if expired.is_set() or time.monotonic() >= effective_deadline:
        outcome = "cancelled" if cancel_deadline else "timeout"
    record = state.record(outcome, status, time.monotonic(), error)
    record["timing_observation"] = "sse-incremental" if body.get("stream", False) else "buffered-completion"
    if not body.get("stream", False):
        # A buffered JSON response exposes completion time, not first-token time.
        record["ttft_ms"] = None
        record["first_tool_ms"] = None
        record["output_delta_interval_ms"] = None
    choice = body.get("tool_choice")
    function = choice.get("function") if isinstance(choice, dict) else None
    forced_benchmark_tool = isinstance(function, dict) and function.get("name") == "bench_sum"
    record["quality_source"] = "nonempty-only"
    if outcome == "success":
        valid = state.delta_count > 0 and state.finish_reason != "content_filter"
        if forced_benchmark_tool:
            valid = valid and record["tool_valid"] is True and record["tool_calls"] == 1
            record["quality_source"] = "expected-tool"
        if expected is not None:
            if not isinstance(expected, dict) or set(expected) - {"text", "normalize_whitespace", "tool"}:
                raise ValueError("invalid expected output predicate")
            if "tool" in expected:
                if expected != {"tool": "bench_sum"}:
                    raise ValueError("expected tool predicate must name bench_sum exclusively")
                valid = valid and record["tool_valid"] is True and record["tool_calls"] == 1
                record["quality_source"] = "expected-tool"
            else:
                actual, target = "".join(state.text), expected.get("text")
                if not isinstance(target, str):
                    raise ValueError("expected text required")
                if expected.get("normalize_whitespace", False):
                    actual, target = " ".join(actual.split()), " ".join(target.split())
                valid = valid and actual.strip() == target.strip()
                record["quality_source"] = "expected-text"
        if not valid:
            record["outcome"] = "invalid_output"
    return record


def summarize(records, wall_seconds):
    successful = [r for r in records if r["outcome"] == "success"]
    authoritative = [r for r in successful if r["usage"] is not None and r["usage_source"].startswith("upstream-")]
    counts = {name: sum(r["outcome"] == name for r in records)
              for name in ("success", "refused", "http_error", "error", "timeout", "cancelled", "invalid_stream", "invalid_output")}
    tokens = sum(r["usage"]["completion_tokens"] for r in authoritative)
    result = {"requests": len(records), "outcomes": counts, "wall_seconds_including_drain": wall_seconds,
              "successful_requests_per_second": len(successful) / wall_seconds if wall_seconds > 0 else None,
              "authoritative_success_count": len(authoritative), "authoritative_output_tokens": tokens,
              "successful_output_tokens_per_second": tokens / wall_seconds if successful and len(authoritative) == len(successful) and wall_seconds > 0 else None}
    for field in ("ttft_ms", "elapsed_ms", "first_tool_ms"):
        values = [r[field] for r in successful if r[field] is not None]
        result[field] = {"samples": len(values), **{f"p{q}": percentile(values, q / 100) for q in (50, 95, 99)}}
    result["percentile_note"] = "descriptive samples; use repeated load campaigns for tail claims"
    result["wire_usage_mismatches"] = sum(bool(r.get("wire_usage_mismatch")) for r in records)
    result["quality_verified_successes"] = sum(r.get("quality_source") in ("expected-text", "expected-tool") for r in successful)
    return result


def run_campaign(urls, corpus, key, limits, abort_event=None):
    limits.validate()
    if not urls or not corpus:
        raise ValueError("at least one endpoint and workload required")
    started, counter, records = time.monotonic(), 0, []
    lock = threading.Lock()

    def worker():
        nonlocal counter
        while True:
            with lock:
                if (abort_event is not None and abort_event.is_set()) or time.monotonic() - started >= limits.seconds or counter >= limits.max_requests:
                    return
                index, counter = counter, counter + 1
            item = corpus[index % len(corpus)]
            record = request_once(urls[index % len(urls)], item["request"], key, limits.request_timeout,
                                  item.get("cancel_after_ms"), item.get("cancel_after_deltas"), item.get("expect"))
            record.update(request_index=index, case=item["name"], endpoint_index=index % len(urls))
            with lock:
                records.append(record)
            # Rejections remain separate requests, not hidden retries. Avoid a
            # busy loop which measures client rejection spin instead of capacity.
            if record["outcome"] == "refused":
                delay = min(.2, max(0, limits.seconds - (time.monotonic() - started)))
                if abort_event is None:
                    time.sleep(delay)
                else:
                    abort_event.wait(delay)

    with concurrent.futures.ThreadPoolExecutor(max_workers=limits.concurrency) as pool:
        futures = [pool.submit(worker) for _ in range(limits.concurrency)]
        for future in futures:
            future.result(timeout=limits.seconds + limits.request_timeout + 15)
    wall = time.monotonic() - started
    records.sort(key=lambda r: r["request_index"])
    return {"limits": dataclasses.asdict(limits), "summary": summarize(records, wall), "records": records,
            "case_summaries": {name: summarize([r for r in records if r["case"] == name], wall)
                               for name in sorted({r["case"] for r in records})},
            "request_budget_exhausted": counter >= limits.max_requests,
            "aborted": abort_event is not None and abort_event.is_set(),
            "refusal_pacing_ms": 200}


def load_provenance(path, corpus_bytes):
    if Path(path).stat().st_size > 65536:
        raise ValueError("provenance exceeds byte limit")
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(data, dict) or set(data) - set(PROVENANCE_FIELDS + PROVENANCE_OPTIONAL_FIELDS):
        raise ValueError("unknown provenance fields; secrets and free-form payloads are refused")
    missing = [k for k in PROVENANCE_FIELDS if k not in data or data[k] is None]
    if missing:
        raise ValueError("missing provenance: " + ", ".join(missing))
    if data["arm"] not in ("baseline", "candidate") or data["corpus_sha256"] != sha256(corpus_bytes):
        raise ValueError("invalid arm or mismatched corpus hash")
    if type(data["context_per_sequence"]) is not int or data["context_per_sequence"] <= 0:
        raise ValueError("invalid effective context")
    if type(data["slots"]) is not int or data["slots"] <= 0:
        raise ValueError("invalid slot count")
    digest_fields = ("binary_sha256", "model_sha256", "runtime_sha256", "source_diff_sha256",
                     "config_redacted_sha256", "corpus_sha256")
    digest_fields += tuple(key for key in ("tokenizer_sha256", "template_sha256", "gpu_topology_sha256")
                           if key in data)
    for key in digest_fields:
        value = data[key]
        if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
            raise ValueError("invalid digest: " + key)
    flags = data["runtime_flags"]
    if not isinstance(flags, dict) or set(flags) - RUNTIME_FLAG_NAMES:
        raise ValueError("unknown runtime flags; credential-bearing flags are refused")
    if any(not isinstance(v, (str, int, float, bool, type(None))) for v in flags.values()):
        raise ValueError("invalid runtime flag values")
    return data


def compare(baseline, candidate, factors=()):
    if baseline["provenance"]["arm"] != "baseline" or candidate["provenance"]["arm"] != "candidate":
        raise ValueError("both explicitly identified arms required")
    if any(f not in EXPERIMENT_FACTORS for f in factors):
        raise ValueError("unknown experimental factor")
    for field in MATCH_FIELDS:
        if field not in factors and baseline["provenance"][field] != candidate["provenance"][field]:
            raise ValueError("confounded comparison: " + field)
    for field in OPTIONAL_MATCH_FIELDS:
        if field not in factors and baseline["provenance"].get(field) != candidate["provenance"].get(field):
            raise ValueError("confounded optional provenance: " + field)
    if baseline["limits"] != candidate["limits"]:
        raise ValueError("different campaign limits")
    a, b = baseline["summary"], candidate["summary"]
    old, new = a["successful_output_tokens_per_second"], b["successful_output_tokens_per_second"]
    return {"experimental_factors": list(factors), "baseline": a, "candidate": b,
            "throughput_ratio": new / old if old and new is not None else None,
            "note": "descriptive paired result; repeat and apply predeclared correctness/SLO gates"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("run")
    run.add_argument("--url", action="append", required=True, help="base ending in /v1; may repeat across replicas")
    run.add_argument("--corpus", type=Path, required=True)
    run.add_argument("--provenance", type=Path, required=True)
    run.add_argument("--output", type=Path, required=True)
    run.add_argument("--api-key-env", default="A3S_BENCH_KEY")
    run.add_argument("--seconds", type=float, default=60)
    run.add_argument("--request-timeout", type=float, default=120)
    run.add_argument("--concurrency", type=int, default=1)
    run.add_argument("--max-requests", type=int, default=10000)
    paired = commands.add_parser("compare")
    paired.add_argument("baseline", type=Path)
    paired.add_argument("candidate", type=Path)
    paired.add_argument("--factor", choices=EXPERIMENT_FACTORS, action="append", default=[])
    args = parser.parse_args()
    if args.command == "compare":
        print(json.dumps(compare(json.loads(args.baseline.read_text()), json.loads(args.candidate.read_text()), args.factor), indent=2))
        return
    if args.corpus.stat().st_size > 2 * 1024 * 1024:
        raise ValueError("corpus exceeds byte limit")
    corpus_bytes = args.corpus.read_bytes()
    provenance = load_provenance(args.provenance, corpus_bytes)
    corpus = [json.loads(line) for line in corpus_bytes.decode().splitlines() if line.strip()]
    if not corpus or len(corpus) > 256 or any(not isinstance(x.get("name"), str) or not isinstance(x.get("request"), dict) for x in corpus):
        raise ValueError("bounded named corpus required")
    for item in corpus:
        if set(item) - {"name", "request", "expect", "cancel_after_ms", "cancel_after_deltas"}:
            raise ValueError("unknown corpus fields")
        cancel_ms, cancel_deltas = item.get("cancel_after_ms"), item.get("cancel_after_deltas")
        if cancel_ms is not None and (type(cancel_ms) not in (int, float) or not math.isfinite(cancel_ms) or not 0 < cancel_ms <= args.request_timeout * 1000):
            raise ValueError("invalid cancellation deadline")
        if cancel_deltas is not None and (type(cancel_deltas) is not int or not 1 <= cancel_deltas <= 1000000):
            raise ValueError("invalid cancellation delta bound")
    result = run_campaign(args.url, corpus, os.environ.get(args.api_key_env),
                          Limits(args.seconds, args.request_timeout, args.concurrency, args.max_requests))
    result["provenance"] = provenance
    result["schema_version"] = 1
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps(result["summary"], indent=2))


if __name__ == "__main__":
    main()
