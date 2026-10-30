import unittest
from types import SimpleNamespace
import power_xolotl_bridge as bridge


def choice_of(choice):
    if choice is None or choice in ("auto", "none", "required"):
        return choice or "auto", None
    if isinstance(choice, dict) and choice.get("type") == "function":
        return "named", choice.get("function", {}).get("name")
    return "unknown", None


class ToolSelection(unittest.TestCase):
    def setUp(self):
        self.frontend = SimpleNamespace(tool_choice_of=choice_of)
        self.service = SimpleNamespace()
        bridge.install_dsml_tool_choice({"F": self.frontend, "S": self.service,
                                        "CALLS_OPEN": "<｜DSML｜ calls>",
                                        "INVOKE_OPEN": "<｜DSML｜ invoke"})
        self.force = self.service.forced_call
        self.tools = [{"name": "lookup_test_record", "parameters": {"type": "object"}}]

    def test_named_and_single_required_include_the_canonical_calls_envelope(self):
        expected = '<｜DSML｜ calls>\n<｜DSML｜ invoke name="lookup_test_record">\n'
        named = {"type": "function", "function": {"name": "lookup_test_record"}}
        self.assertEqual(self.force(named, self.tools), expected)
        self.assertEqual(self.force("required", self.tools), expected)
        self.assertIs(self.frontend.forced_call, self.service.forced_call)
        self.assertNotIn("parameter", expected)  # no fabricated model arguments

    def test_required_multiple_defers_the_function_name_to_generation(self):
        offered = self.tools + [{"name": "second"}]
        self.assertEqual(self.force("required", offered), '<｜DSML｜ calls>\n<｜DSML｜ invoke ')

    def test_auto_none_and_absent_do_not_force_a_call(self):
        for choice in (None, "auto", "none"):
            self.assertIsNone(self.force(choice, self.tools))

    def test_unknown_choice_or_unoffered_name_fails_closed(self):
        choices = ["anything", "required", {"type": "function", "function": {"name": "missing"}}]
        for choice in choices:
            with self.subTest(choice=choice), self.assertRaises(ValueError):
                self.force(choice, [] if choice == "required" else self.tools)

    def test_dsml_attribute_delimiters_cannot_inject_another_tool(self):
        for name in ['bad"name', "bad<name", "bad>name", "bad\nname", "bad\rname", "bad\0name"]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.force({"type": "function", "function": {"name": name}}, [{"name": name}])


if __name__ == "__main__":
    unittest.main()
