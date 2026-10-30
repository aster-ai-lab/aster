"""A model acknowledgement or successful CLI exit cannot qualify a refused command."""
import unittest
from types import SimpleNamespace

from cli_sandbox_smoke import ProbeHandler, execution_passed


class ExecutionOracle(unittest.TestCase):
    def test_model_follows_runtime_owned_tool_ids_and_never_retries_the_command(self):
        handler = object.__new__(ProbeHandler)
        handler.server = SimpleNamespace(attempted=False)
        first = handler.decide({"messages": []})
        self.assertEqual(first[2]["kind"], "tool_call")
        self.assertEqual(handler.decide({"messages": []})[2]["kind"], "missing_receipt")
        receipt = "native_process_lifetime_unqualified"
        messages = [
            {"role": "assistant", "tool_calls": [{"id": "runtime-owned", "function": {"name": "bash"}}]},
            {"role": "tool", "tool_call_id": "runtime-owned", "content": receipt},
        ]
        self.assertEqual(handler.decide({"messages": messages})[2], {"kind": "tool_result", "output": receipt})

    def test_requires_one_successful_guest_receipt_with_the_workspace_nonce(self):
        output = "Tool exit code: 0\nASTER_RELEASE_SANDBOX_OK\nLinux\nfixture-nonce\n"
        events = [{"kind": "tool_call"}, {"kind": "tool_result", "output": output}]
        self.assertTrue(execution_passed(0, events, "fixture-nonce"))
        for code, rows, nonce in (
            (1, events, "fixture-nonce"),
            (0, [], "fixture-nonce"),
            (0, events, "different-workspace"),
            (0, events + [{"kind": "tool_call"}], "fixture-nonce"),
            (0, [{"kind": "tool_call"}, {"kind": "tool_result", "output": output.replace("Linux", "Darwin")}], "fixture-nonce"),
            (0, [{"kind": "tool_call"}, {"kind": "tool_result", "output": "Tool exit code: 1\nnative_process_lifetime_unqualified"}], "fixture-nonce"),
        ):
            with self.subTest(code=code, rows=rows, nonce=nonce):
                self.assertFalse(execution_passed(code, rows, nonce))


if __name__ == "__main__":
    unittest.main()
