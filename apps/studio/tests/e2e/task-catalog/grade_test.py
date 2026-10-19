"""Contract tests for the frozen ../tasks graders and the upload boundary."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import common
from grade import A51_ROWS, docx_text, grade


NOTICE = common.TASKS_ROOT / "A51文件校核" / "关于开展季度工作督导及相关事项的通知.docx"


def good_a51() -> str:
    lines = []
    for index, row in enumerate(A51_ROWS, start=1):
        notes = row["notes"][0] if row["notes"] else "见原文"
        lines.append(f"{index}. " + " ".join(row["quotes"]) + " " + notes)
    return "\n".join(lines)


class GradeContract(unittest.TestCase):
    def test_catalog_matches_source_tree(self):
        self.assertEqual(len(common.load_catalog()), 25)
        self.assertTrue(common.TASKS_ROOT.is_dir())
        frozen = json.loads(common.PIN_PATH.read_text(encoding="utf-8"))
        self.assertEqual(frozen, common.source_files())
        self.assertNotIn(".DS_Store", frozen)
        for case in common.load_catalog():
            source = common.case_dir(case)
            for relative in case["allow"]:
                self.assertTrue((source / relative).is_file(), relative)
            for relative in case["deny"]:
                self.assertTrue((source / relative).is_file(), relative)
                self.assertNotIn(relative, case["allow"])
            cell = common.sheet_cell("功能点", f"C{case['list_row']}")
            self.assertIn(case["title"][:4], cell)
            if "prompt" in case:
                self.assertIn(case["prompt"], cell)
            prompt = common.prompt_for(case)
            self.assertTrue(prompt.strip())
            if case["id"] == "A51":
                self.assertEqual(prompt, "帮我校验下上传文件")
            if case["id"] == "CAT-CONFLICT":
                self.assertNotIn("模型识别", prompt)

    def test_prepare_keeps_answers_out(self):
        case = common.case_by_id("A23")
        with tempfile.TemporaryDirectory() as directory:
            job = common.prepare_job(case, Path(directory) / "job")
            workspace = job / "workspace"
            copied = json.loads((job / "copied.json").read_text(encoding="utf-8"))
            self.assertEqual(set(copied), set(case["allow"]))
            self.assertFalse((workspace / "02_核对答案_不要上传给模型.md").exists())
            instruction = (job / "instruction.txt").read_text(encoding="utf-8")
            self.assertNotIn("应有5项", instruction)
            self.assertIn("整理待办表", instruction)
            for relative, digest in copied.items():
                self.assertEqual(digest, common.sha256(common.case_dir(case) / relative))

    def test_a51_quotes_exist_in_the_notice(self):
        source = docx_text(NOTICE)
        for row in A51_ROWS:
            for quote in row["quotes"]:
                self.assertIn(quote, source)

    def test_closed_answers_pass_and_a_missing_fact_fails(self):
        samples = {
            "A03": "订单模块完成率85%。9月12日发现两个高风险问题。项目星河预算120万元。剩余15%下周完成。",
            "A23": "2026年。指南清查，综合科，11月5日，清查台账。咨询问答更新，服务科，11月8日，问答清单。账号检查，信息科，11月10日，检查记录。宣传材料归档，未明确，11月12日，归档目录。经验交流，综合科，未明确，交流纪要。",
            "A24": "申请年龄18周岁未变。适用范围只调整了措辞和顺序。取消身份证明复印件。受理由10个工作日改为5个工作日。增加门户提交。代理授权书为新增。咨询电话条款删除。",
            "A25": "申请表已提供。登记资料对应主体证明，已提供。项目说明已提供，日期需人工确认。预算表已提供。授权书未找到。",
            "A26": "完成事项 P01、A01、B01、C01。P01 由甲编写、乙校对。问题未提供。N01 列入下月计划。",
            "A28": "Z01 已完成。F01 进度未提供，状态进行中。X01 进度60%，未完成。",
            "A29": "J01 偏差-2。J02 按期。J03 偏差+2。J04 滞后3天。J05 日期缺失，无法计算。",
            "A30": "A站10份中有2份缺少更新时间。B站120与135待核。培训是受访者建议。",
            "A35": "明细30条。咨询225，办理240，合计465。",
            "A36": "指南助手由120到180，环比50%，随后到90，再由99到108，环比9.09%。分析助手第4周缺失，该周环比不可计算。",
            "A40": "模块为检索、收藏、反馈。不支持上传、审批和视频。",
            "A49": "方案A符合已知约束。方案B为8人日，超过5。方案C需要外部授权，工作量待确认。",
            "CAT-MAIL": "各位同事：原定周三14:00的项目评审会改到周五10:00，地点不变。请知悉。",
            "CAT-COPY": "管理者：把重复工作交给助手。\n业务人员：在对话里完成周报和表格。\nIT人员：沿用现有账号，不新增接口。",
            "CAT-CONFLICT": "详细报告和100字不能同时满足。请确认优先级？",
        }
        self.assertEqual(grade("A51", good_a51(), NOTICE), [])
        for case_id, text in samples.items():
            self.assertEqual(grade(case_id, text), [], case_id)
            self.assertTrue(grade(case_id, "没有事实"), case_id)

    def test_prohibited_claims_fail(self):
        self.assertTrue(grade("A24", "18周岁未变。适用范围措辞调整。取消身份证明复印件。10个工作日改为5个工作日。门户提交。代理授权书新增。咨询电话条款删除。电话已失效。"))
        self.assertTrue(grade("A26", "完成事项 P01 A01 B01 C01。甲编写，乙校对。问题未提供。N01已完成，并写入下月。"))
        self.assertTrue(grade("A30", "10份里2份缺少更新时间。把120与135合计255。培训是建议。待核。"))
        self.assertTrue(grade("A51", good_a51().replace("此处前文未提及上季度督导问题，仅作表述", "草稿备注已删除"), NOTICE))

    def test_unfrozen_cases_do_not_pass_on_empty_output(self):
        for case_id in ("A01", "A02", "A04", "A06", "A07", "A08", "A09", "A10", "A12"):
            self.assertEqual(grade(case_id, ""), [f"{case_id} 的评分器尚未冻结"])


if __name__ == "__main__":
    unittest.main()
