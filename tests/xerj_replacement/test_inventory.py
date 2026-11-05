"""Frozen-source integrity checks; these do not qualify runtime capabilities."""

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from inventory import CONTRACTS, REFERENCE, ROOT, build_inventory, generate, read_json, serialized


class InventoryTests(unittest.TestCase):
    def setUp(self):
        _, self.reference = read_json(REFERENCE, 1024 * 1024)
        _, self.decisions = read_json(CONTRACTS / "mcp-owners.json", 128 * 1024)

    def test_generated_inventory_is_exact_and_does_not_claim_runtime_success(self):
        inventory = build_inventory()
        self.assertEqual(serialized(inventory), (CONTRACTS / "mcp-inventory.json").read_bytes())
        self.assertEqual(len(inventory["tools"]), 22)
        self.assertEqual([t["name"] for t in inventory["tools"]], [t["name"] for t in self.reference["tools"]])
        for index, (row, source) in enumerate(zip(inventory["tools"], self.reference["tools"])):
            self.assertEqual(row["source_pointer"], f"/tools/{index}")
            self.assertEqual(row["required"], source["inputSchema"].get("required", []))
            self.assertEqual(row["properties"], list(source["inputSchema"]["properties"]))
            self.assertEqual(row["qualification_status"], "NOT_RUN")
        self.assertFalse(inventory["coverage"]["complete_replacement_denominator"])
        self.assertFalse(inventory["coverage"]["live_reference_verified"])

    def test_every_schema_field_contributes_to_identity_and_source_order_is_preserved(self):
        original = generate(self.reference, self.decisions)
        changed = copy.deepcopy(self.reference)
        changed["tools"][0]["inputSchema"]["properties"]["symbol"]["description"] += " altered"
        inventory = generate(changed, self.decisions)
        self.assertNotEqual(original["tools"][0]["tool_schema_sha256"], inventory["tools"][0]["tool_schema_sha256"])
        changed["tools"].reverse()
        inventory = generate(changed, self.decisions)
        self.assertEqual([r["name"] for r in inventory["tools"]], [t["name"] for t in changed["tools"]])

    def test_missing_unknown_duplicate_and_invalid_reference_tools_refuse(self):
        for corruption in ("missing", "extra", "unknown", "duplicate", "required"):
            reference = copy.deepcopy(self.reference)
            if corruption == "missing":
                reference["tools"].pop()
            elif corruption == "extra":
                reference["tools"].append(reference["tools"][0])
            elif corruption == "unknown":
                reference["tools"][0]["name"] = "unmapped_tool"
            elif corruption == "duplicate":
                reference["tools"][1] = reference["tools"][0]
            else:
                reference["tools"][0]["inputSchema"]["required"] = ["absent_property"]
            with self.subTest(corruption=corruption), self.assertRaises(ValueError):
                generate(reference, self.decisions)

    def test_owner_decisions_cannot_guess_admission_or_forge_pass(self):
        for corruption in ("missing", "extra", "duplicate", "owner", "presence", "status", "path", "evidence"):
            decisions = copy.deepcopy(self.decisions)
            if corruption == "missing":
                decisions["tools"].pop()
            elif corruption == "extra":
                decisions["tools"].append(dict(decisions["tools"][0], name="unlisted_tool"))
            elif corruption == "duplicate":
                decisions["tools"][1] = decisions["tools"][0]
            elif corruption == "owner":
                decisions["tools"][0]["owner"] = "guess-from-name"
            elif corruption == "presence":
                decisions["tools"][-1]["presence"] = "declared"
            elif corruption == "status":
                decisions["tools"][0]["status"] = "PASS"
            elif corruption == "path":
                decisions["tools"][0]["paths"] = ["../outside"]
            else:
                decisions["tools"][0]["required_evidence"] = []
            with self.subTest(corruption=corruption), self.assertRaises(ValueError):
                generate(self.reference, decisions)

    def test_source_bytes_are_pinned_and_input_reads_are_bounded(self):
        with tempfile.TemporaryDirectory(prefix="inventory 中文 ") as directory:
            path = Path(directory) / "schema with spaces.json"
            path.write_bytes(REFERENCE.read_bytes() + b" ")
            with self.assertRaisesRegex(ValueError, "digest mismatch"):
                build_inventory(reference_path=path)
            path.write_bytes(b"x" * (1024 * 1024 + 1))
            with self.assertRaisesRegex(ValueError, "byte limit"):
                read_json(path, 1024 * 1024)

    def test_check_command_does_not_rewrite_the_generated_file(self):
        path = CONTRACTS / "mcp-inventory.json"
        before = path.stat().st_mtime_ns
        result = subprocess.run([sys.executable, str(Path(__file__).with_name("inventory.py")), "--check"],
                                cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("runtime qualification NOT_RUN", result.stdout)
        self.assertEqual(before, path.stat().st_mtime_ns)


if __name__ == "__main__":
    unittest.main()
