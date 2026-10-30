"""Protocol-boundary pins for the test relay, separate from real-LLM acceptance."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class RelayTests(unittest.TestCase):
    def exercise(self, respond, error=None, packets=None, responses=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            agent = root / "fixture-agent"
            agent.write_text(
                "#!" + sys.executable + "\n"
                "import json,sys\n"
                "responses=" + repr(responses or {}) + "\n"
                "for line in sys.stdin.buffer:\n"
                " packet=json.loads(line)\n"
                " sys.stdout.buffer.write(line);sys.stdout.buffer.flush()\n"
                + (" result=" + repr(error) + "\n"
                   " response={'jsonrpc':'2.0','id':packet['id']}\n"
                   " response.update(responses.get(packet['id'], "
                   "{'error' if result else 'result':result or "
                   "{'stopReason':'end_turn','private':'RELAY-SECRET-CANARY'}}))\n"
                   " print(json.dumps(response),flush=True)\n" if respond else "")
            )
            agent.chmod(0o700)
            path = root / "trace.jsonl"
            env = os.environ.copy()
            env.update(A3S_CODE_TUI_TEST_ACP_REAL=str(agent), A3S_CODE_TUI_TEST_ACP_TRACE=str(path))
            process = subprocess.Popen(
                [sys.executable, str(Path(__file__).with_name("acp_trace_proxy.py"))],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env,
            )
            packet = {"jsonrpc":"2.0", "id":9,"method":"session/prompt", "params":{
                "sessionId":"s1","_meta":{"promptId":"p1"},
                "prompt":[{"type":"text","text":"RELAY-SECRET-CANARY 中文"}]}}
            try:
                for packet in packets or [packet]:
                    wire = (json.dumps(packet, ensure_ascii=False, separators=(", ", ": ")) + "\n").encode()
                    process.stdin.write(wire)
                    process.stdin.flush()
                    self.assertEqual(process.stdout.readline(), wire, "relay must preserve actual bytes")
                    if respond:
                        response = json.loads(process.stdout.readline())
                        self.assertEqual(response["id"], packet["id"])
                process.stdin.close()
                self.assertEqual(process.wait(timeout=10), 0)
                trace = path.read_text()
                self.assertNotIn("RELAY-SECRET-CANARY", trace)
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)
                return [json.loads(line) for line in trace.splitlines()]
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
                process.stdout.close()
                process.stderr.close()

    def test_byte_preservation_identity_and_content_redaction(self):
        events = self.exercise(True)
        self.assertEqual([e["kind"] for e in events], ["prompt-request", "prompt-response", "proxy-closed"])
        self.assertEqual(events[0]["prompt_digest"], events[1]["prompt_digest"])
        self.assertEqual(events[0]["in_flight_before"], 0)
        self.assertEqual(events[1]["stop_reason"], "end_turn")
        self.assertEqual(events[2]["pending_prompts"], 0)

    def test_eof_does_not_invent_a_terminal(self):
        events = self.exercise(False)
        self.assertEqual([e["kind"] for e in events], ["prompt-request", "proxy-closed"])
        self.assertEqual(events[-1]["pending_prompts"], 1)

    def test_notification_trace_retains_identity_without_message_or_queue_text(self):
        events = self.exercise(False, packets=[
            {"jsonrpc":"2.0", "method":"session/update", "params":{
                "sessionId":"s1", "_meta":{"promptId":"p2"},
                "update":{"sessionUpdate":"agent_message_chunk", "content":{
                    "type":"text", "text":"RELAY-SECRET-CANARY"}}}},
            {"jsonrpc":"2.0", "method":"_a3s/queue/changed", "params":{
                "sessionId":"s1", "runningPromptId":"p2",
                "runningText":"RELAY-SECRET-CANARY"}},
        ])
        self.assertEqual(events[0], {"seq":1, "kind":"session-update",
                                    "session_id":"s1", "prompt_id":"p2",
                                    "update":"agent_message_chunk"})
        self.assertEqual(events[1], {"seq":2, "kind":"queue-changed",
                                    "session_id":"s1", "running_prompt_id":"p2"})
        self.assertEqual(events[-1]["pending_prompts"], 0)

    def test_only_the_narrow_gate_error_is_classified(self):
        events = self.exercise(True, {"code":-32603,"data":"completion gate: workspace mutation abc (rate.py) has no bound Passed verification"})
        self.assertTrue(events[1]["completion_gate"])
        events = self.exercise(True, {"code":-32603,"data":"unrelated message mentions completion gate: RELAY-SECRET-CANARY"})
        self.assertFalse(events[1]["completion_gate"])

    def test_model_receipts_require_the_matching_rpc_and_session(self):
        methods = [
            ("unrelated/extension", {}),
            ("session/new", {}),
            ("session/set_model", {"sessionId":"s1", "modelId":"rejected/model"}),
            ("session/set_model", {"sessionId":"s1", "modelId":"selected/model"}),
            ("session/set_config_option", {"sessionId":"s1", "configId":"other"}),
            ("session/set_config_option", {"sessionId":"s1", "configId":"model"}),
            ("session/load", {"sessionId":"s2"}),
            ("unrelated/extension", {}),
        ]
        packets = [{"jsonrpc":"2.0", "id":i, "method":method, "params":params}
                   for i, (method, params) in enumerate(methods)]
        events = self.exercise(True, packets=packets, responses={
            0: {"result":{"models":{"currentModelId":"unrelated/model"}}},
            1: {"result":{"sessionId":"s1", "models":{"currentModelId":"initial/model"}}},
            2: {"error":{"code":-32602}},
            3: {"result":{}},
            4: {"result":{"configOptions":[{"id":"model", "currentValue":"unrelated/model"}]}},
            5: {"result":{"configOptions":[{"id":"model", "currentValue":"configured/model"}]}},
            6: {"result":{"models":{"currentModelId":"loaded/model"}}},
            7: {"result":[]},
        })
        self.assertEqual([(e["rpc_id"], e["session_id"], e["model_id"])
                          for e in events if e["kind"] == "model-selected"], [
            (1, "s1", "initial/model"), (3, "s1", "selected/model"),
            (5, "s1", "configured/model"), (6, "s2", "loaded/model"),
        ])


if __name__ == "__main__":
    unittest.main()
