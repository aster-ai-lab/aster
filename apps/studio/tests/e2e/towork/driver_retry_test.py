"""The background press retries when the accessibility token cannot be used."""

from __future__ import annotations

import unittest

from driver import Drive, _PRESS_ATTEMPTS, _incomplete_suffix, _press_should_retry

_STALE = "element_token is stale; call get_window_state again to refresh"


class _Harness:
    """Drive methods without a cua-driver process."""

    def __init__(self, failures: int, *, message: str = _STALE) -> None:
        self.pid = 1
        self.window_id = 2
        self.failures = failures
        self.message = message
        self.calls: list[dict] = []
        self.refreshes = 0

    def call(self, method: str, arguments: dict) -> None:
        self.calls.append({"method": method, "arguments": dict(arguments)})
        if len(self.calls) <= self.failures:
            raise RuntimeError(self.message)

    def refresh(self) -> None:
        self.refreshes += 1

    def _readdress(self, element: dict) -> dict:
        fresh = dict(element)
        fresh["element_token"] = "fresh-token"
        return fresh

    press_background = Drive.press_background
    chord = Drive.chord


class PressRetryTest(unittest.TestCase):
    def test_retries_a_stale_or_unproven_token(self) -> None:
        self.assertTrue(_press_should_retry(_STALE))
        self.assertTrue(_press_should_retry("Background input refused (element_outside_target_window)"))
        self.assertTrue(_press_should_retry("AXError -25202"))

    def test_retries_a_superseded_snapshot_more_than_once(self) -> None:
        self.assertGreaterEqual(_PRESS_ATTEMPTS, 5)

    def test_leaves_other_refusals_closed(self) -> None:
        self.assertFalse(_press_should_retry("minimized_or_hidden_window"))
        self.assertFalse(_press_should_retry("background drag is unavailable on macOS"))

    def test_incomplete_insert_returns_only_the_unsent_tail(self) -> None:
        text = "a" * 300
        message = (
            "type_text incomplete: delivered 295 of 300 character(s) via CGEvent "
            "(0ms delay); retry only the remaining suffix"
        )
        self.assertEqual(_incomplete_suffix(message, text), "a" * 5)
        self.assertIsNone(_incomplete_suffix(message, "short"))
        self.assertIsNone(_incomplete_suffix("element_token is stale", text))

    def test_incomplete_type_retries_only_the_suffix(self) -> None:
        class Typer(_Harness):
            type_text = Drive.type_text

        text = "b" * 300
        harness = Typer(
            1,
            message=(
                "type_text incomplete: delivered 295 of 300 character(s) via CGEvent "
                "(0ms delay); retry only the remaining suffix"
            ),
        )
        harness.type_text(text, {"element_token": "old"})
        self.assertEqual(
            [item["arguments"]["text"] for item in harness.calls],
            [text, text[295:]],
        )
        self.assertNotIn("element_token", harness.calls[1]["arguments"])

    def test_type_into_focuses_then_types_without_a_token(self) -> None:
        class Typer(_Harness):
            pixel_click = Drive.pixel_click
            _background_click = Drive._background_click
            chord = Drive.chord
            type_text = Drive.type_text
            press = Drive.press
            type_into = Drive.type_into

        harness = Typer(0)
        harness.type_into(
            {"element_token": "field", "role": "AXComboBox", "label": "搜索设置..."},
            "归档",
        )
        self.assertEqual(
            [item["method"] for item in harness.calls],
            ["click", "hotkey", "type_text"],
        )
        self.assertEqual(harness.calls[0]["arguments"]["element_token"], "field")
        self.assertNotIn("element_token", harness.calls[1]["arguments"])
        self.assertEqual(harness.calls[1]["arguments"]["keys"], ["cmd", "a"])
        self.assertNotIn("element_token", harness.calls[2]["arguments"])
        self.assertEqual(harness.calls[2]["arguments"]["text"], "归档")

    def test_zero_delivery_retries_the_same_field(self) -> None:
        class Typer(_Harness):
            type_text = Drive.type_text

        text = "e2e-renamed-123"
        harness = Typer(
            1,
            message=(
                "type_text incomplete: delivered 0 of 15 character(s) via CGEvent "
                "(0ms delay); retry only the remaining suffix"
            ),
        )
        harness.type_text(text, {"role": "AXTextField", "label": "重命名", "element_token": "old"})
        self.assertEqual([item["arguments"]["text"] for item in harness.calls], [text, text])
        self.assertEqual(harness.calls[1]["arguments"]["element_token"], "fresh-token")

    def test_incomplete_type_does_not_resend_a_mismatched_count(self) -> None:
        class Typer(_Harness):
            type_text = Drive.type_text

        harness = Typer(1, message="type_text incomplete: delivered 2 of 9 character(s)")
        with self.assertRaises(RuntimeError):
            harness.type_text("abcdef")
        self.assertEqual(len(harness.calls), 1)

    def test_chord_readdresses_a_stale_token(self) -> None:
        harness = _Harness(1)
        element = {
            "role": "AXTextArea",
            "label": "Markdown 编辑区",
            "element_token": "old",
            "frame": {"x": 1, "y": 2, "w": 3, "h": 4},
        }
        harness.chord(["cmd", "s"], element)
        self.assertEqual(
            [item["arguments"].get("element_token") for item in harness.calls],
            ["old", "fresh-token"],
        )
        self.assertEqual(harness.calls[1]["arguments"]["keys"], ["cmd", "s"])

    def test_chord_stops_at_the_attempt_bound(self) -> None:
        harness = _Harness(10)
        element = {"role": "AXTextArea", "label": "Markdown 编辑区", "element_token": "old", "frame": {}}
        with self.assertRaises(RuntimeError):
            harness.chord(["cmd", "s"], element)
        self.assertEqual(len(harness.calls), _PRESS_ATTEMPTS)

    def test_chord_does_not_retry_other_refusals(self) -> None:
        harness = _Harness(1, message="minimized_or_hidden_window")
        with self.assertRaises(RuntimeError):
            harness.chord(["cmd", "a"])
        self.assertEqual(len(harness.calls), 1)
        self.assertEqual(harness.refreshes, 0)

    def test_untargeted_key_refreshes_until_the_token_is_current(self) -> None:
        harness = _Harness(2)
        harness.press_background("escape")
        self.assertEqual(len(harness.calls), 3)
        self.assertTrue(all("element_token" not in item["arguments"] for item in harness.calls))
        self.assertGreaterEqual(harness.refreshes, 2)

    def test_untargeted_key_stops_at_the_attempt_bound(self) -> None:
        harness = _Harness(10)
        with self.assertRaises(RuntimeError):
            harness.press_background("escape")
        self.assertEqual(len(harness.calls), _PRESS_ATTEMPTS)

    def test_ui_chord_forwards_the_retry_attempt(self) -> None:
        from suite import Ui

        class Forward(Ui):
            def __init__(self) -> None:
                self.forwarded = None

            def _with_visible_window(self, method, *args, **kwargs):
                self.forwarded = (method, args, kwargs)

        dummy = Forward()
        dummy.chord(["cmd", "s"], {"element_token": "t"}, attempt=2)
        self.assertIsNotNone(dummy.forwarded)
        self.assertIs(dummy.forwarded[0].__func__, Drive.chord)
        self.assertEqual(dummy.forwarded[2]["attempt"], 2)

    def test_ambiguous_key_closes_the_sibling_and_retries(self) -> None:
        import suite

        closed: list[int] = []
        original = suite._close_extra_windows
        suite._close_extra_windows = lambda pid: closed.append(pid)

        class Once(suite.Ui):
            def __init__(self) -> None:
                self.pid = 82545

        calls: list[int] = []

        def method() -> None:
            calls.append(1)
            if len(calls) == 1:
                raise RuntimeError(
                    "Background input refused (same_pid_keyboard_ambiguity): pid 82545 owns 1 other eligible top-level window(s)"
                )

        try:
            Once()._with_visible_window(method)
        finally:
            suite._close_extra_windows = original
        self.assertEqual(closed, [82545])
        self.assertEqual(calls, [1, 1])

    def test_ambiguous_key_stays_closed_when_the_sibling_remains(self) -> None:
        import suite

        original = suite._close_extra_windows
        suite._close_extra_windows = lambda pid: None

        class Once(suite.Ui):
            def __init__(self) -> None:
                self.pid = 82545

        def method() -> None:
            raise RuntimeError("Background input refused (same_pid_keyboard_ambiguity): still blocked")

        try:
            with self.assertRaises(RuntimeError):
                Once()._with_visible_window(method)
        finally:
            suite._close_extra_windows = original


if __name__ == "__main__":
    unittest.main()
