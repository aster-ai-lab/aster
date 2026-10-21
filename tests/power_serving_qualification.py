#!/usr/bin/env python3
"""Hermetic client/provenance proofs for the Power serving campaign."""
from __future__ import annotations

import copy
import importlib.util
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import sys
import tempfile
import threading
import time
import unittest
from unittest import mock
from types import SimpleNamespace

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/power_serving_qualification.py"
SPEC = importlib.util.spec_from_file_location("power_qualification", SCRIPT)
q = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = q
SPEC.loader.exec_module(q)

MATRIX_PATH = SCRIPT.parents[1] / "docs/evidence/power/2026-10-21-serving-optimization/matrix-campaign.py"
MATRIX_SPEC = importlib.util.spec_from_file_location("power_matrix_campaign", MATRIX_PATH)
m = importlib.util.module_from_spec(MATRIX_SPEC)
sys.modules[MATRIX_SPEC.name] = m
sys.modules["power_serving_qualification"] = q
MATRIX_SPEC.loader.exec_module(m)


class Fixture(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    active = 0
    maximum = 0
    lock = threading.Lock()
    barrier = None

    def log_message(self, *_args):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        with self.lock:
            type(self).active += 1
            type(self).maximum = max(type(self).maximum, type(self).active)
        try:
            if body.get("case") == "overlap":
                self.barrier.wait(timeout=.5)
            if body.get("case") == "slow_headers":
                for byte in b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n":
                    self.connection.sendall(bytes([byte]))
                    time.sleep(.006)
                return
            if body.get("case") == "overload":
                self.send_response(429)
                self.send_header("Content-Length", "0")
                self.end_headers()
                return
            if body.get("case") == "chunked_stall":
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Transfer-Encoding", "chunked")
                self.end_headers()
                raw = b'data: {"choices":[{"delta":{"role":"assistant"}}]}\n\n'
                self.wfile.write(f"{len(raw):x}\r\n".encode() + raw + b"\r\n")
                self.wfile.flush()
                time.sleep(.15)
                self.close_connection = True
                return
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream" if body.get("stream") else "application/json")
            self.send_header("Connection", "close")
            self.end_headers()
            self.close_connection = True
            if body.get("case") == "slow_body":
                raw = json.dumps({"choices": [{"message": {"content": "slow"}, "finish_reason": "stop"}]}).encode()
                for byte in raw:
                    self.wfile.write(bytes([byte]))
                    self.wfile.flush()
                    time.sleep(.006)
                return
            if body.get("case") == "buffered":
                time.sleep(.015)
                self.wfile.write(json.dumps({"choices": [{"message": {"content": "你好"}, "finish_reason": "stop"}]}).encode())
                self.wfile.flush()
                return

            def event(value):
                data = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)
                raw = ("data: " + data + "\r\n\r\n").encode()
                for byte in raw:  # Real fragmentation includes UTF-8 boundaries.
                    self.wfile.write(bytes([byte]))
                self.wfile.flush()

            event({"choices": [{"delta": {"role": "assistant"}, "finish_reason": None}]})
            if body.get("case") == "malformed_shape":
                event({"choices": [None]})
                return
            if body.get("case") == "slow_line":
                for byte in b'data: {"choices":[{"delta":{"content":"slow"}}]}\n\n':
                    self.wfile.write(bytes([byte]))
                    self.wfile.flush()
                    time.sleep(.006)
                return
            if body.get("case") == "stall":
                time.sleep(.3)
                return
            time.sleep(.015)
            if body.get("case") == "tools":
                event({"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call-1", "function": {"name": "bench_", "arguments": "{\"a\":"}}]}}]})
                event({"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"name": "sum", "arguments": "19,\"b\":23}"}}]}, "finish_reason": "tool_calls"}]})
            else:
                event({"choices": [{"delta": {"content": "你"}, "finish_reason": None}]})
                if body.get("case") == "truncated":
                    return
                event({"choices": [{"delta": {"content": "好"}, "finish_reason": "stop"}]})
            event({"choices": [], "prism_upstream": {"source": "upstream-reported", "usage": {"prompt_tokens": 12, "completion_tokens": 7, "total_tokens": 19}}})
            event("[DONE]")
        except (BrokenPipeError, ConnectionResetError, OSError):
            pass
        finally:
            with self.lock:
                type(self).active -= 1


class QuietServer(ThreadingHTTPServer):
    def handle_error(self, *_args):
        # Cancellation/absolute-deadline cases intentionally abort sockets.
        pass


class QualificationTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = QuietServer(("127.0.0.1", 0), Fixture)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.url = f"http://127.0.0.1:{cls.server.server_port}/v1"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()

    def request(self, case="text", **kwargs):
        return q.request_once(self.url, {"stream": True, "case": case}, None, 1, **kwargs)

    def test_fragmented_utf8_trailing_usage_and_meaningful_ttft(self):
        result = self.request()
        self.assertEqual(result["outcome"], "success")
        self.assertEqual(result["usage"]["completion_tokens"], 7)
        self.assertEqual(result["meaningful_deltas"], 2)
        self.assertEqual(result["usage_source"], "upstream-usage")
        self.assertGreaterEqual(result["ttft_ms"], 10)
        self.assertTrue(result["terminal_observed"])

    def test_buffered_completion_is_not_first_token_measurement(self):
        result = q.request_once(self.url, {"stream": False, "case": "buffered"}, None, 1,
                                expected={"text": "你好"})
        self.assertEqual(result["outcome"], "success")
        self.assertGreaterEqual(result["elapsed_ms"], 10)
        self.assertIsNone(result["ttft_ms"])
        self.assertIsNone(result["first_tool_ms"])
        self.assertEqual(result["timing_observation"], "buffered-completion")

    def test_incremental_tools_reconstruct_without_counting_chunks(self):
        result = self.request("tools", expected={"tool": "bench_sum"})
        self.assertEqual(result["outcome"], "success")
        self.assertEqual(result["quality_source"], "expected-tool")
        self.assertTrue(result["tool_valid"])
        self.assertEqual(result["tool_calls"], 1)
        self.assertEqual(result["usage"]["completion_tokens"], 7)
        self.assertEqual(self.request(expected={"tool": "bench_sum"})["outcome"], "invalid_output")

    def test_truncated_stream_is_not_success(self):
        result = self.request("truncated")
        self.assertEqual(result["outcome"], "error")
        self.assertFalse(result["terminal_observed"])

    def test_cancel_before_output_and_after_delta(self):
        before = self.request("stall", cancel_after_ms=30)
        after = self.request(cancel_after_deltas=1)
        self.assertEqual(before["outcome"], "cancelled")
        self.assertLess(before["elapsed_ms"], 200)
        self.assertIsNone(before["ttft_ms"])
        self.assertEqual(after["outcome"], "cancelled")
        self.assertFalse(after["terminal_observed"])

    def test_overload_and_total_request_deadline(self):
        self.assertEqual(self.request("overload")["outcome"], "refused")
        result = q.request_once(self.url, {"stream": True, "case": "stall"}, None, .03)
        self.assertEqual(result["outcome"], "timeout")

    def test_finite_concurrency_and_request_budget(self):
        deadline = time.monotonic() + 1
        while Fixture.active and time.monotonic() < deadline:
            time.sleep(.005)
        self.assertEqual(Fixture.active, 0)
        Fixture.maximum, Fixture.barrier = 0, threading.Barrier(2)
        corpus = [{"name": "text", "request": {"stream": True, "case": "overlap"}}]
        result = q.run_campaign([self.url], corpus, None, q.Limits(.5, 1, 2, 6))
        self.assertEqual(result["summary"]["requests"], 6)
        self.assertEqual(result["summary"]["outcomes"]["success"], 6)
        self.assertEqual(result["summary"]["authoritative_output_tokens"], 42)
        self.assertLessEqual(max(r["request_index"] for r in result["records"]), 5)
        self.assertEqual(Fixture.maximum, 2, "requires real overlap and respects concurrency cap")

    def test_usage_validation_and_undefined_rates(self):
        self.assertIsNone(q.valid_usage({"prompt_tokens": True, "completion_tokens": 1, "total_tokens": 2}))
        self.assertIsNone(q.valid_usage({"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 8}))
        state = q.Output(time.monotonic())
        state.observe({"choices": [{"delta": {"content": "one"}}], "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}}, time.monotonic())
        record = state.record("success", 200, time.monotonic())
        self.assertIsNone(record["upstream_decode_tokens_per_second"])
        self.assertIsNone(record["client_request_output_tokens_per_second"])
        self.assertIsNone(q.summarize([record], .1)["successful_output_tokens_per_second"])
        self.assertIsNone(q.percentile([], .95))
        self.assertEqual(q.percentile([1, 2, 3, 4], .5), 2.5)

    def test_limits_and_credential_urls_are_refused(self):
        for limits in [q.Limits(0), q.Limits(float("inf")), q.Limits(concurrency=65), q.Limits(max_requests=0)]:
            with self.subTest(limits=limits), self.assertRaises(ValueError):
                limits.validate()
        with self.assertRaises(ValueError):
            q.request_once("http://secret@127.0.0.1/v1", {}, None, 1)

    def test_slow_headers_line_and_body_obey_absolute_deadline(self):
        for case in ("slow_headers", "slow_line", "slow_body"):
            with self.subTest(case=case):
                result = q.request_once(self.url, {"case": case, "stream": case != "slow_body"}, None, .03)
                self.assertEqual(result["outcome"], "timeout")
                self.assertLess(result["elapsed_ms"], 180)

    def test_chunked_sse_cancellation_records_outcome_without_cross_thread_parser_close(self):
        owner, violations = threading.get_ident(), []
        close = q.http.client.HTTPResponse.close

        def observed_close(response):
            if threading.get_ident() != owner:
                violations.append("HTTPResponse parser was mutated by another thread")
            return close(response)

        with mock.patch.object(q.http.client.HTTPResponse, "close", observed_close):
            for _ in range(5):
                result = q.request_once(self.url, {"stream": True, "case": "chunked_stall"}, None,
                                        1, cancel_after_ms=30)
                self.assertEqual(result["outcome"], "cancelled")
                self.assertEqual(result["meaningful_deltas"], 0)
                self.assertLess(result["elapsed_ms"], 180)
        self.assertEqual(violations, [], "socket watchdog must not race the response parser owner")

    def test_malformed_shape_and_quality_failure_are_recorded(self):
        self.assertEqual(self.request("malformed_shape")["outcome"], "error")
        result = q.request_once(self.url, {"stream": True}, None, 1, expected={"text": "wrong"})
        self.assertEqual(result["outcome"], "invalid_output")
        state = q.Output(time.monotonic())
        with self.assertRaises(ValueError):
            state.observe({"choices": [{"delta": {}, "finish_reason": "nonsense"}]}, time.monotonic())
        state.observe({"choices": [{"delta": {}, "finish_reason": "stop"}]}, time.monotonic())
        self.assertEqual(state.delta_count, 0)

    def test_wire_mismatch_and_uncertified_timing_are_visible(self):
        state = q.Output(time.monotonic())
        state.observe({"choices": [], "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
                       "prism_upstream": {"source": "upstream-reported", "usage": {"prompt_tokens": 2, "completion_tokens": 7, "total_tokens": 9}}}, time.monotonic())
        self.assertTrue(state.record("success", 200, time.monotonic())["wire_usage_mismatch"])
        state.observe({"prism_upstream": {"source": "estimated", "timings": {"predicted_per_second": 9999}}}, time.monotonic())
        self.assertIsNone(state.upstream_decode_rate)

    def test_provenance_refuses_secrets_and_unbounded_input(self):
        corpus = b'{"name":"fixture","request":{}}\n'
        data = {k: "a" * 64 for k in q.PROVENANCE_FIELDS}
        data.update(arm="baseline", context_per_sequence=32768, slots=1,
                    corpus_sha256=q.sha256(corpus), runtime_flags={"--parallel": 1})
        with tempfile.TemporaryDirectory(prefix="power-provenance-") as directory:
            path = Path(directory) / "provenance.json"
            path.write_text(json.dumps(data))
            self.assertEqual(q.load_provenance(path, corpus)["arm"], "baseline")
            for field in ("tokenizer_sha256", "template_sha256", "gpu_topology_sha256"):
                data[field] = "b" * 64
                path.write_text(json.dumps(data))
                self.assertEqual(q.load_provenance(path, corpus)[field], "b" * 64)
                for invalid in ("b" * 65, "g" * 64, None):
                    data[field] = invalid
                    path.write_text(json.dumps(data))
                    with self.subTest(field=field, invalid=invalid), self.assertRaises(ValueError):
                        q.load_provenance(path, corpus)
                data[field] = "b" * 64
            data["runtime_flags"] = {"--api-key": "test-secret"}
            path.write_text(json.dumps(data))
            with self.assertRaises(ValueError):
                q.load_provenance(path, corpus)
            path.write_text("x" * 65537)
            with self.assertRaises(ValueError):
                q.load_provenance(path, corpus)

    def test_comparison_refuses_confounded_context_and_missing_arm(self):
        provenance = {k: "fixed" for k in q.MATCH_FIELDS}
        provenance["arm"] = "baseline"
        base = {"provenance": provenance, "limits": {"seconds": 60},
                "summary": {"successful_output_tokens_per_second": 10}}
        candidate = copy.deepcopy(base)
        candidate["provenance"]["arm"] = "candidate"
        candidate["summary"]["successful_output_tokens_per_second"] = 12
        self.assertEqual(q.compare(base, candidate)["throughput_ratio"], 1.2)
        candidate["provenance"]["slots"] = "different"
        with self.assertRaises(ValueError):
            q.compare(base, candidate)
        self.assertEqual(q.compare(base, candidate, ["slots"])["throughput_ratio"], 1.2)
        candidate["provenance"]["context_per_sequence"] = "smaller"
        with self.assertRaises(ValueError):
            q.compare(base, candidate, ["slots"])
        candidate["provenance"]["context_per_sequence"] = "fixed"
        candidate["provenance"]["warmup_requests"] = 20
        with self.assertRaises(ValueError):
            q.compare(base, candidate, ["slots"])
        with self.assertRaises(ValueError):
            q.compare(base, candidate, ["context_per_sequence"])


class MatrixCampaignTest(unittest.TestCase):
    @staticmethod
    def gpu():
        return {"used_mib": 1000.0, "total_mib": 24564.0, "gpu_percent": 1.0,
                "temperature_c": 40.0, "power_w": 50.0}

    @staticmethod
    def native():
        return {name: 0.0 for name in m.COUNTER_NAMES}

    @staticmethod
    def result():
        record = {"outcome": "success", "quality_source": "expected-text",
                  "usage_source": "upstream-usage", "usage": {"completion_tokens": 7}}
        return {"records": [record], "summary": {"wire_usage_mismatches": 0,
                "wall_seconds_including_drain": .1, "outcomes": {"success": 1},
                "ttft_ms": {}, "elapsed_ms": {}}, "request_budget_exhausted": False}

    def test_engine_state_requires_actual_model_nonempty_slots_and_per_sequence_context(self):
        props = {"model_path": m.MODEL_PATH, "model_alias": m.MODEL_ALIAS,
                 "total_slots": 1, "default_generation_settings": {"n_ctx": 32768}}
        slots = [{"id": 0, "n_ctx": 32768, "is_processing": False}]

        def check(p, s, count=1):
            with mock.patch.object(m, "get", side_effect=[json.dumps(p).encode(), json.dumps(s).encode()]):
                return m.engine_state(count)

        self.assertEqual(check(props, slots)["slots"][0]["n_ctx"], 32768)
        invalid = [(props, []), (dict(props, model_path="/wrong.gguf"), slots),
                   (dict(props, model_alias="wrong"), slots),
                   (props, [dict(slots[0], n_ctx=8192)]),
                   (props, [dict(slots[0], id=-1)]),
                   (props, [dict(slots[0], is_processing="false")]),
                   (dict(props, default_generation_settings={"n_ctx": float("nan")}), slots),
                   ({"error": "legal 200 error body"}, slots)]
        for p, s in invalid:
            with self.subTest(props=p, slots=s), self.assertRaises(m.GuardError):
                check(p, s)
        double = [slots[0], dict(slots[0], id=1)]
        self.assertEqual(check(dict(props, total_slots=2, default_generation_settings={"n_ctx": 65536}), double, 2)["total_slots"], 2)
        with self.assertRaises(m.GuardError):
            check(dict(props, total_slots=2), [slots[0], slots[0]], 2)

    def test_native_observation_requires_all_finite_integral_counts_and_idle_gauges(self):
        good = self.native()
        for values in [good, {k: v for k, v in good.items() if k != "llamacpp:tokens_predicted_total"},
                       dict(good, **{"llamacpp:requests_processing": 1}),
                       dict(good, **{"llamacpp:tokens_predicted_total": float("nan")}),
                       dict(good, **{"llamacpp:tokens_predicted_total": .5})]:
            raw = "\n".join(f"{key} {value}" for key, value in values.items()).encode()
            with mock.patch.object(m, "get", return_value=raw):
                if values is good:
                    self.assertEqual(m.counters(), good)
                else:
                    with self.assertRaises(m.GuardError):
                        m.counters()

    def test_gpu_columns_nan_and_physical_bounds_are_refused(self):
        self.assertEqual(m.validate_gpu(self.gpu()), self.gpu())
        for values in [dict(self.gpu(), used_mib=float("nan")), dict(self.gpu(), total_mib=0),
                       dict(self.gpu(), gpu_percent=101), {"used_mib": 1000}]:
            with self.subTest(values=values), self.assertRaises(m.GuardError):
                m.validate_gpu(values)
        for text in ["nan,24564,nan,nan,nan\n", "1000,24564,1,40\n", "1000,24564,1,40,50\n1000,24564,1,40,50\n"]:
            with mock.patch.object(m.subprocess, "run", return_value=SimpleNamespace(stdout=text)):
                with self.assertRaises(m.GuardError):
                    m.gpu_sample()

    def test_proc_listener_owner_and_reused_pid_prevent_wrong_termination(self):
        raw = b"header\n 0: 0100007F:4A9C 00000000:0000 0A 00000000:00000000 00:00000000 00000000 1000 0 12345 1\n"
        self.assertEqual(m.parse_listeners(raw, 19100), {"12345"})
        with mock.patch.object(m, "listener_inodes", return_value={"12345"}), mock.patch.object(m, "socket_inodes", return_value={"999"}):
            with self.assertRaises(m.GuardError):
                m.require_listener(42, 19100)
        expected = {"pid": 42, "start_ticks": 100}
        with mock.patch.object(m, "engine_identity", return_value=dict(expected, start_ticks=200)), mock.patch.object(m.os, "kill") as kill:
            with self.assertRaises(m.GuardError):
                m.terminate_private_engine(expected, 1)
            kill.assert_not_called()
        command = ["server", "--port", "19100", "--port=19101"]
        with self.assertRaises(m.GuardError):
            m.command_flag(command, ("--port",))
        fields = ["S"] + ["0"] * 18 + ["123"]
        self.assertEqual(m.start_ticks(("42 (server (worker)) " + " ".join(fields)).encode()), 123)

    def test_closed_rig_engine_geometry_cuda_binding_and_kill_cannot_cross_gpu(self):
        rig = m.RIGS[5]
        command = ['llama-server', '--host', '127.0.0.1', '--port', '19120', '-m', m.MODEL_PATH,
                   '--alias', m.MODEL_ALIAS, '-c', '131072', '--parallel', '4']
        raw = ('\0'.join(command) + '\0').encode()
        identity = {'pid': 42, 'start_ticks': 100, 'exe_sha256': 'a' * 64}
        with mock.patch.object(m, 'process_identity', return_value=dict(identity)) as process, mock.patch.object(m, 'read_limited', side_effect=[raw, b'PRIVATE_SECRET=DO_NOT_PERSIST\0CUDA_VISIBLE_DEVICES=5\0']):
            actual = m.engine_identity(42, 4, rig)
            process.assert_called_once_with(42, 19120)
            self.assertEqual(actual['cuda_visible_devices'], '5')
            self.assertNotIn('PRIVATE_SECRET', json.dumps(actual))
        for chosen, environment, slots in [(m.RIGS[6], b'CUDA_VISIBLE_DEVICES=5\0', 4),
                                            (rig, b'CUDA_VISIBLE_DEVICES=6\0', 4),
                                            (rig, b'CUDA_VISIBLE_DEVICES=5\0', 2)]:
            with mock.patch.object(m, 'process_identity', return_value=dict(identity)), mock.patch.object(m, 'read_limited', side_effect=[raw, environment]), mock.patch.object(m.os, 'kill') as kill:
                with self.subTest(rig=chosen.index, slots=slots), self.assertRaises(m.GuardError):
                    m.terminate_private_engine(actual, slots, chosen)
                kill.assert_not_called()

    def test_rig_manifest_requires_exact_physical_gpu_and_context(self):
        fixed = {'power': {'exe_sha256': 'a' * 64}, 'engine': {'exe_sha256': 'b' * 64}}
        manifest = {'arm': 'candidate', 'gpu_indices': [5], 'context_per_sequence': 32768,
                    'binary_sha256': 'a' * 64, 'runtime_sha256': 'b' * 64}
        m.validate_manifest('candidate', manifest, fixed, m.RIGS[5])
        for changes in ({'gpu_indices': [6]}, {'gpu_indices': [5, 6]}, {'context_per_sequence': 8192}, {'arm': 'baseline'}):
            with self.subTest(changes=changes), self.assertRaises(m.GuardError):
                m.validate_manifest('candidate', dict(manifest, **changes), fixed, m.RIGS[5])
        ports = []
        for index in (7, 6, 5):
            rig = m.RIGS[index]
            ports += [rig.engine_port, *rig.power_ports.values(), *rig.gateway_ports.values()]
            self.assertEqual(rig.key_path, m.PRIVATE / 'benchmark.key')
            with mock.patch.object(m.subprocess, 'run', return_value=SimpleNamespace(stdout='1000,24564,1,40,50\n')) as query:
                m.gpu_sample(rig)
                self.assertIn('--id=' + str(index), query.call_args.args[0])
        self.assertEqual(len(ports), len(set(ports)), 'closed rigs cannot share private ports')

    def test_single_arm_np4_campaign_uses_its_own_gateway_gpu_and_evidence(self):
        rig, fixed = m.RIGS[5], {'engine': {'pid': 42}}
        after = dict(self.native(), **{'llamacpp:tokens_predicted_total': 7})
        corpus = [{'name': 'fixture', 'request': {'stream': True}, 'expect': {'text': 'OK'}}]
        with tempfile.TemporaryDirectory(prefix='power-rig5-') as directory:
            with mock.patch.object(m, 'identity', return_value=fixed) as identity, mock.patch.object(m, 'engine_state', return_value={}) as state, mock.patch.object(m, 'counters', side_effect=[self.native(), self.native(), after]), mock.patch.object(m, 'gpu_sample', side_effect=lambda *_args: self.gpu()) as gpu, mock.patch.object(m, 'monitor_gpu'), mock.patch.object(m.q, 'request_once', return_value={'outcome': 'success'}) as warmup, mock.patch.object(m.q, 'run_campaign', return_value=self.result()) as campaign, mock.patch('builtins.print'):
                result = m.run_one('candidate', 8, 1, .1, Path(directory), corpus, 'PRIVATE_KEY',
                                   {'slots': 4}, fixed, rig, ['slots', 'runtime_flags', 'admission'])
            self.assertEqual(result['qualification_status'], 'passed')
            self.assertEqual(result['rig'], 5)
            self.assertEqual(result['experimental_factors'], ['slots', 'runtime_flags', 'admission'])
            self.assertEqual(warmup.call_args.args[0], 'http://127.0.0.1:19885/v1')
            self.assertEqual(campaign.call_args.args[0], ['http://127.0.0.1:19885/v1'])
            self.assertTrue(all(call.args == ('candidate', 4, rig) for call in identity.call_args_list))
            self.assertTrue(all(call.args == (4, rig) for call in state.call_args_list))
            self.assertTrue(all(call.args == (rig,) for call in gpu.call_args_list))
            raw = (Path(directory) / 'candidate-c8-r1.json').read_text()
            self.assertNotIn('PRIVATE_KEY', raw)
            self.assertEqual(json.loads(raw)['rig'], 5)

    def test_monitor_failure_aborts_dispatch_and_vram_kill_requires_identity(self):
        stop, abort = threading.Event(), threading.Event()
        errors = []
        with mock.patch.object(m, "gpu_sample", side_effect=OSError()), mock.patch.object(m.os, "kill") as kill:
            m.monitor_gpu(stop, abort, [], errors, {"pid": 42}, 1)
            self.assertTrue(abort.is_set())
            kill.assert_not_called()
        stop, abort, errors = threading.Event(), threading.Event(), []
        expected = {"pid": 42, "start_ticks": 100}
        with mock.patch.object(m, "gpu_sample", return_value=dict(self.gpu(), used_mib=24000)), mock.patch.object(m, "engine_identity", return_value=expected), mock.patch.object(m.os, "kill") as kill:
            m.monitor_gpu(stop, abort, [], errors, expected, 1)
            self.assertTrue(abort.is_set())
            kill.assert_called_once_with(42, m.signal.SIGTERM)

    def test_native_success_usage_equality_and_baseline_unknown_subset(self):
        before, after, result = self.native(), self.native(), self.result()
        after["llamacpp:tokens_predicted_total"] = 7
        self.assertEqual(m.validate_native_result("candidate", result, before, after), 7)
        with self.assertRaises(m.GuardError):
            m.validate_native_result("candidate", result, before, dict(after, **{"llamacpp:tokens_predicted_total": 8}))
        invalid = copy.deepcopy(result)
        invalid["summary"]["wire_usage_mismatches"] = 1
        with self.assertRaises(m.GuardError):
            m.validate_native_result("candidate", invalid, before, after)
        unknown = copy.deepcopy(result)
        unknown["records"][0].update(usage=None, usage_source="unavailable")
        self.assertEqual(m.validate_native_result("baseline", unknown, before, after), 7)
        self.assertIsNone(unknown["records"][0]["usage"])
        with self.assertRaises(m.GuardError):
            m.validate_native_result("candidate", unknown, before, after)
        with self.assertRaises(m.GuardError):
            m.validate_native_result("baseline", result, before, before)

    def test_post_observation_failure_retains_completed_records_without_key(self):
        fixed = {"engine": {"pid": 42}}
        corpus = [{"name": "fixture", "request": {"stream": True}, "expect": {"text": "OK"}}]
        with tempfile.TemporaryDirectory(prefix="power-matrix-") as directory:
            with mock.patch.object(m, "identity", return_value=fixed), mock.patch.object(m, "engine_state", side_effect=[{}, {}, OSError()]), mock.patch.object(m, "counters", return_value=self.native()), mock.patch.object(m, "gpu_sample", side_effect=lambda *_args: self.gpu()), mock.patch.object(m, "monitor_gpu"), mock.patch.object(m.q, "request_once", return_value={"outcome": "success"}), mock.patch.object(m.q, "run_campaign", return_value=self.result()):
                with self.assertRaises(OSError):
                    m.run_one("candidate", 1, 1, .1, Path(directory), corpus, "DO_NOT_PERSIST_KEY", {"slots": 1}, fixed)
            raw = (Path(directory) / "candidate-c1-r1.json").read_text()
            saved = json.loads(raw)
            self.assertEqual(saved["qualification_status"], "failed")
            self.assertEqual(saved["failure_stage"], "postchecks")
            self.assertEqual(len(saved["records"]), 1)
            self.assertEqual(saved["tool_identity"], m.TOOL_IDENTITY)
            self.assertEqual(set(saved["tool_identity"]), {"runner_sha256", "matrix_sha256"})
            self.assertTrue(all(len(value) == 64 for value in saved["tool_identity"].values()))
            self.assertNotIn("DO_NOT_PERSIST_KEY", raw)

    def test_invalid_preflight_cannot_send_warmup_or_measurement_requests(self):
        with tempfile.TemporaryDirectory(prefix="power-matrix-refusal-") as directory:
            with mock.patch.object(m, "identity", return_value={}), mock.patch.object(m, "engine_state", side_effect=m.GuardError("empty slots")), mock.patch.object(m.q, "request_once") as request, mock.patch.object(m.q, "run_campaign") as campaign:
                with self.assertRaises(m.GuardError):
                    m.run_one("baseline", 1, 1, .1, Path(directory), [], "", {"slots": 1}, {})
                request.assert_not_called()
                campaign.assert_not_called()
            saved = json.loads((Path(directory) / "baseline-c1-r1.json").read_text())
            self.assertEqual(saved["failure_stage"], "preflight")

    def test_observation_slow_headers_and_body_have_absolute_deadlines(self):
        class ObservationFixture(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_GET(self):
                try:
                    if self.path == "/slow_headers":
                        for byte in b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n":
                            self.connection.sendall(bytes([byte]))
                            time.sleep(.006)
                        return
                    self.send_response(200)
                    self.send_header("Connection", "close")
                    self.end_headers()
                    self.close_connection = True
                    for byte in b'{"actual":"bounded"}':
                        self.wfile.write(bytes([byte]))
                        self.wfile.flush()
                        if self.path == "/slow_body":
                            time.sleep(.006)
                except OSError:
                    pass

        server = QuietServer(("127.0.0.1", 0), ObservationFixture)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            self.assertEqual(json.loads(m.get("/ok", port=server.server_port)), {"actual": "bounded"})
            for path in ("/slow_headers", "/slow_body"):
                started = time.monotonic()
                with self.assertRaises(Exception):
                    m.get(path, port=server.server_port, timeout=.03)
                self.assertLess(time.monotonic() - started, .18)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_campaign_abort_stops_new_dispatch_and_drains_inflight_deadline(self):
        entered, abort = threading.Event(), threading.Event()

        class InflightFixture(BaseHTTPRequestHandler):
            def log_message(self, *_args):
                pass

            def do_POST(self):
                self.rfile.read(int(self.headers["Content-Length"]))
                self.send_response(200)
                self.send_header("Connection", "close")
                self.end_headers()
                self.wfile.flush()
                entered.set()
                time.sleep(.2)
                self.close_connection = True

        server = QuietServer(("127.0.0.1", 0), InflightFixture)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()

        def abort_entered():
            if entered.wait(1):
                abort.set()

        controller = threading.Thread(target=abort_entered, daemon=True)
        controller.start()
        try:
            result = q.run_campaign([f"http://127.0.0.1:{server.server_port}/v1"],
                                    [{"name": "inflight", "request": {"stream": True}}], None,
                                    q.Limits(.5, .08, 1, 100), abort_event=abort)
            self.assertEqual(result["summary"]["requests"], 1)
            self.assertTrue(result["aborted"])
            self.assertEqual(result["summary"]["outcomes"]["timeout"], 1)
            self.assertLess(result["summary"]["wall_seconds_including_drain"], .25)
        finally:
            server.shutdown()
            server.server_close()
            thread.join()
            controller.join()


if __name__ == "__main__":
    unittest.main()
