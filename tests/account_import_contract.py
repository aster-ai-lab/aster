"""Account-import closed-set contract guard.

The local account importers name themselves in three places that must
not drift: the `AccountImport` enum, the refusal error text, and the
companion binary's usage string. A drift makes a valid import name parse on
one surface and be rejected as unknown on another.
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENUM = ROOT / "crates/code/core/src/config/account_import.rs"
USAGE = ROOT / "crates/code/acp/src/bin/aster-acp.rs"


def enum_names():
    matches = re.findall(r'"([a-z0-9-]+)" => Ok\(Self::', ENUM.read_text())
    if not matches:
        raise AssertionError("no AccountImport parse arms found")
    return matches


class AccountImportContractTest(unittest.TestCase):
    def test_parse_and_error_text_name_the_same_closed_set(self):
        names = enum_names()
        # The refusal names every member, in order, so an unknown source
        # error can never advertise a stale set.
        self.assertIn("closed set: " + ", ".join(names), ENUM.read_text())

    def test_the_usage_string_lists_every_import(self):
        usage = USAGE.read_text()
        names = enum_names()
        listed = re.search(r"\[--import-accounts ([^\]]+)\]", usage)
        self.assertIsNotNone(listed, "usage string lost --import-accounts")
        shown = [name.strip() for name in listed.group(1).split(",")]
        self.assertEqual(shown, names)


if __name__ == "__main__":
    unittest.main()
