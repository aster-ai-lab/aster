"""Keep exact evidence checks independent of Markdown's clause-label layout."""

import importlib.util
from pathlib import Path
import tempfile
import unittest


HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("content_grader", HERE / "cases/studio-content/tests/test_outputs.py")
grader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(grader)


class ContentGraderTests(unittest.TestCase):
    def test_clause_body_can_be_quoted_under_its_heading_but_changed_amount_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "inputs").mkdir()
            clauses = ["甲方支付100000元。", "验收标准由双方确定。", "交付日期由双方确定。", "双方保留权利。"]
            (root / "inputs/briefs.md").write_text("\n".join(
                f"第{number}条：{body}" for number, body in zip("一二三四", clauses)))
            original = "建议：" + "源条款逐字引用，意见与原文逐项对应。" * 12 + "\n" + "\n".join(
                f"### 第{number}条\n> {body}" for number, body in zip("一二三四", clauses))
            path = root / "F11.md"
            path.write_text(original)
            grader.ROOT = root
            grader.test_f11_contract_findings_quote_each_critical_original_clause()
            path.write_text(original.replace("100000元", "90000元"))
            with self.assertRaises(AssertionError):
                grader.test_f11_contract_findings_quote_each_critical_original_clause()


if __name__ == "__main__":
    unittest.main()
