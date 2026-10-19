"""Direct checks for the background CUA guard. This file does not open ToWork."""

from __future__ import annotations

import os
import unittest

from cua_guard import ACTION_TOOLS, GuardError, GuardState


class Recorder:
    def __init__(self, delivery: str = "background") -> None:
        self.calls: list[tuple[str, dict]] = []
        self.delivery = delivery

    def invoke(self, name: str, arguments: dict) -> dict:
        self.calls.append((name, arguments))
        return {"structuredContent": {"delivery": {"mode": self.delivery}}}


class GuardContract(unittest.TestCase):
    def tearDown(self) -> None:
        os.environ.pop("A3S_GUI_ALLOW_MOUSE", None)

    def test_start_refuses_mouse_mode(self) -> None:
        os.environ["A3S_GUI_ALLOW_MOUSE"] = "1"
        with self.assertRaises(GuardError) as caught:
            GuardState()
        self.assertEqual(str(caught.exception), "A3S_GUI_ALLOW_MOUSE=1")
        print("refuse_start A3S_GUI_ALLOW_MOUSE=1", flush=True)

    def test_actions_refuse_non_background_delivery(self) -> None:
        os.environ.pop("A3S_GUI_ALLOW_MOUSE", None)
        state = GuardState()
        recorder = Recorder()
        refused = ("click", "type_text", "hotkey")
        self.assertTrue(set(refused).issubset(ACTION_TOOLS))
        for name in refused:
            for mode in (None, "foreground", "hid"):
                arguments = {} if mode is None else {"delivery_mode": mode}
                with self.assertRaises(GuardError):
                    state.call(lambda: recorder.invoke(name, arguments), name, arguments)
                print(f"refuse_{name} delivery_mode={mode or 'missing'} driver_called=0", flush=True)
        self.assertEqual(recorder.calls, [])
        self.assertEqual(state.non_background, 0)

    def test_background_action_reaches_the_driver(self) -> None:
        state = GuardState()
        recorder = Recorder()
        for name in ("click", "type_text", "hotkey"):
            arguments = {"delivery_mode": "background"}
            state.call(lambda name=name, arguments=arguments: recorder.invoke(name, arguments), name, arguments)
        self.assertEqual([name for name, _ in recorder.calls], ["click", "type_text", "hotkey"])
        self.assertEqual(state.non_background, 0)
        print("background_actions=3 non_background=0", flush=True)

    def test_reported_non_background_delivery_is_refused(self) -> None:
        state = GuardState()
        recorder = Recorder(delivery="foreground")
        with self.assertRaises(GuardError) as caught:
            state.call(
                lambda: recorder.invoke("click", {"delivery_mode": "background"}),
                "click",
                {"delivery_mode": "background"},
            )
        self.assertIn("foreground", str(caught.exception))
        self.assertEqual(state.non_background, 1)
        print("refuse_reported_click delivery_mode=foreground", flush=True)


if __name__ == "__main__":
    unittest.main()
