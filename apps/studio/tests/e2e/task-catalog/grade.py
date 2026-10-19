"""Fact graders for the ../tasks cases whose answers are already closed.

These checks read the deliverable text. They do not score wording similarity.
"""

from __future__ import annotations

from pathlib import Path
import zipfile
import xml.etree.ElementTree as ET


DOCX_NS = {"w": "http://schemas.openxmlformats.org/wordprocessingml/2006/main"}
SHEET_NS = {"m": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
REL_NS = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}"

# The oral weekly report. A polish that still contains this whole sentence failed.
A03_SOURCE = (
    "这周订单模块联调差不多搞定了，目前完成率85%。9月12日我们发现了2个高风险问题。"
    "项目“星河”的预算还是120万元，没有调整。下周准备把剩下15%做完，同时关闭这2个问题。"
)

# Quotes must occur verbatim in the notice. Notes are the gap the answer key requires.
A51_ROWS = [
    {
        "quotes": ["1. 本季度重点工作任务完成情况", "（二）工作纪律执行情况", "3. 工作整改落实情况"],
        "notes": ["条目标号"],
    },
    {
        "quotes": ["二、督导内容", "四、督导方式"],
        "notes": ["缺少三", "没有“三", "没有「三", "跳过三"],
    },
    {
        "quotes": ["1. 查阅资料", "3. 个别谈话"],
        "notes": ["现场核查"],
    },
    {
        "quotes": ["2. 督导过程中", "4. 本通知自发布之日起执行"],
        "notes": ["第3条", "第 3 条"],
    },
    {
        "quotes": ["（二）工作纪律执行情况", "员工体检"],
        "notes": ["工作纪律"],
    },
    {
        "quotes": ["整改报告", "整改计划"],
        "notes": [],
    },
    {
        "quotes": ["此处前文未提及上季度督导问题，仅作表述"],
        "notes": [],
    },
    {
        "quotes": ["佐证材料", "印证材料"],
        "notes": [],
    },
    {
        "quotes": ["本通知自发布之日起执行"],
        "notes": [],
    },
    {
        "quotes": ["XX单位督导小组"],
        "notes": [],
    },
]


def any_of(text: str, options: list[str]) -> bool:
    return any(option in text for option in options)


def require(text: str, facts: list[str | list[str]], failures: list[str], label: str) -> None:
    missing = []
    for fact in facts:
        options = fact if isinstance(fact, list) else [fact]
        if not any_of(text, options):
            missing.append(" / ".join(options))
    if missing:
        failures.append(f"{label}缺少：{'、'.join(missing)}")


def forbid(text: str, phrases: list[str], failures: list[str]) -> None:
    found = [phrase for phrase in phrases if phrase in text]
    if found:
        failures.append("出现禁止表述：" + "、".join(found))


def sentences(text: str) -> list[str]:
    chunks = []
    for block in text.replace("\r", "\n").split("\n"):
        for mark in "。！？；":
            block = block.replace(mark, mark + "\n")
        chunks.extend(part.strip() for part in block.split("\n") if part.strip())
    return chunks


def grade(case_id: str, text: str, notice_docx: Path | None = None) -> list[str]:
    checker = CHECKS.get(case_id)
    if checker is None:
        return [f"{case_id} 的评分器尚未冻结"]
    if case_id == "A51":
        return checker(text, notice_docx)
    return checker(text)


def grade_a03(text: str) -> list[str]:
    failures: list[str] = []
    require(
        text,
        ["85%", "星河", "120万", ["9月12日", "9月12日"], "15%", ["2个", "两个"], "高风险"],
        failures,
        "A03",
    )
    if A03_SOURCE in text:
        failures.append("A03 整段照抄了口语原文")
    return failures


def grade_a23(text: str) -> list[str]:
    failures: list[str] = []
    require(text, ["2026"], failures, "A23")
    rows = [
        ["指南清查", "综合科", ["11月5日", "11月05日", "2026-11-05"], "清查台账"],
        ["咨询问答更新", "服务科", ["11月8日", "11月08日", "2026-11-08"], "问答清单"],
        ["账号检查", "信息科", ["11月10日", "2026-11-10"], "检查记录"],
        ["宣传材料归档", "未明确", ["11月12日", "2026-11-12"], "归档目录"],
        ["经验交流", "综合科", "未明确", "交流纪要"],
    ]
    for index, row in enumerate(rows, start=1):
        require(text, row, failures, f"A23第{index}行")
    return failures


def grade_a24(text: str) -> list[str]:
    failures: list[str] = []
    require(
        text,
        [
            "18周岁",
            ["未变", "没有变化", "保持不变", "未调整"],
            "适用范围",
            ["措辞", "顺序"],
            "身份证明复印件",
            ["取消", "删除"],
            "10",
            "5",
            "工作日",
            "门户",
            "代理授权书",
            "新增",
            "咨询电话",
            ["删除", "取消"],
        ],
        failures,
        "A24",
    )
    forbid(text, ["号码已失效", "号码失效", "电话已失效", "电话失效", "号码作废"], failures)
    return failures


def grade_a25(text: str) -> list[str]:
    failures: list[str] = []
    require(
        text,
        ["申请表", "主体证明", "项目说明", "预算表", "已提供", "授权书", "未找到", "登记资料", "需人工确认"],
        failures,
        "A25",
    )
    forbid(text, ["真实有效", "材料有效", "材料无效"], failures)
    return failures


def grade_a26(text: str) -> list[str]:
    failures: list[str] = []
    require(text, ["P01", "A01", "B01", "C01", "甲", "编写", "乙", "校对", "N01", "下月", "未提供"], failures, "A26")
    if any("N01" in sentence and "已完成" in sentence for sentence in sentences(text)):
        failures.append("A26 把 N01 写成了已完成")
    return failures


def grade_a28(text: str) -> list[str]:
    failures: list[str] = []
    require(text, ["Z01", "F01", "X01", "60%", "未提供", ["未完成", "进行中"]], failures, "A28")
    forbid(text, ["视为同一事项", "合并为一项", "是同一事项"], failures)
    return failures


def grade_a29(text: str) -> list[str]:
    failures: list[str] = []
    require(
        text,
        ["J01", "-2", "J02", ["按期", "偏差0", "偏差为0", "偏差：0"], "J03", "+2", "J04", "3", "J05", ["无法", "不能计算", "不可计算"]],
        failures,
        "A29",
    )
    return failures


def grade_a30(text: str) -> list[str]:
    failures: list[str] = []
    require(text, ["10", "2", "更新时间", "120", "135", "待核", "培训", ["建议", "意见"]], failures, "A30")
    forbid(text, ["合计255", "共计255", "120+135", "所有站点都需要培训", "全部站点需要培训"], failures)
    return failures


def grade_a35(text: str) -> list[str]:
    failures: list[str] = []
    require(text, ["30", "225", "240", "465", "咨询", "办理"], failures, "A35")
    return failures


def grade_a36(text: str) -> list[str]:
    failures: list[str] = []
    require(
        text,
        ["指南助手", "180", "120", "50%", "90", "9.09%", "分析助手", ["缺失", "未提供"], ["不可计算", "不可算"]],
        failures,
        "A36",
    )
    forbid(text, ["因推广", "由于推广", "因故障", "由于故障", "故障导致"], failures)
    return failures


def grade_a40(text: str) -> list[str]:
    failures: list[str] = []
    require(text, ["检索", "收藏", "反馈", ["不支持上传", "不能上传"], "审批", "视频"], failures, "A40")
    forbid(text, ["支持全文检索", "提供全文检索", "可以全文检索"], failures)
    return failures


def grade_a49(text: str) -> list[str]:
    failures: list[str] = []
    require(text, ["8人日", "5", ["待确认", "未知"], ["外部授权", "授权"]], failures, "A49")
    if "A" not in text or "符合" not in text:
        failures.append("A49缺少：方案 A 符合已知约束")
    forbid(text, ["工作量为0", "成本为0", "C更优", "方案C更优"], failures)
    return failures


def grade_mail(text: str) -> list[str]:
    failures: list[str] = []
    require(text, ["周三", "14:00", "周五", "10:00", "地点"], failures, "CAT-MAIL")
    if len(text.strip()) > 200:
        failures.append("CAT-MAIL 正文超过 200 字")
    forbid(text, ["已发送", "发送成功"], failures)
    return failures


def grade_copy(text: str) -> list[str]:
    failures: list[str] = []
    parts = [part.strip() for part in text.split("\n") if part.strip()]
    audiences = ["管理者", "业务人员", "IT人员"]
    if len(parts) != 3:
        failures.append(f"CAT-COPY 需要 3 条，实际 {len(parts)} 条")
        return failures
    for audience, part in zip(audiences, parts):
        if audience not in part:
            failures.append(f"CAT-COPY 缺少受众 {audience}")
        if len(part) > 80:
            failures.append(f"CAT-COPY {audience} 超过 80 字")
    return failures


def grade_conflict(text: str) -> list[str]:
    failures: list[str] = []
    require(text, ["详细", "100", ["优先级", "请确认", "请选择"], ["？", "?"]], failures, "CAT-CONFLICT")
    forbid(text, ["以下为详细报告", "已写好详细报告", "详细报告如下"], failures)
    return failures


def docx_text(path: Path) -> str:
    with zipfile.ZipFile(path) as archive:
        document = ET.fromstring(archive.read("word/document.xml"))
    paragraphs = []
    for paragraph in document.findall(".//w:p", DOCX_NS):
        paragraphs.append("".join(node.text or "" for node in paragraph.findall(".//w:t", DOCX_NS)))
    return "\n".join(paragraphs)


def grade_a51(text: str, notice_docx: Path | None) -> list[str]:
    if notice_docx is None:
        return ["A51 缺少通知原文，无法核对引文"]
    source = docx_text(notice_docx)
    failures: list[str] = []
    for index, row in enumerate(A51_ROWS, start=1):
        for quote in row["quotes"]:
            if quote not in source:
                failures.append(f"A51 冻结引文不在原文：{quote}")
            if quote not in text:
                failures.append(f"A51 第{index}行缺少原文引文：{quote}")
        if row["notes"] and not any_of(text, row["notes"]):
            failures.append(f"A51 第{index}行缺少说明：{' / '.join(row['notes'])}")
    return failures


def xlsx_text(path: Path) -> str:
    chunks = []
    with zipfile.ZipFile(path) as archive:
        shared = []
        if "xl/sharedStrings.xml" in archive.namelist():
            root = ET.fromstring(archive.read("xl/sharedStrings.xml"))
            for item in root.findall("m:si", SHEET_NS):
                shared.append("".join(node.text or "" for node in item.findall(".//m:t", SHEET_NS)))
        book = ET.fromstring(archive.read("xl/workbook.xml"))
        rels = ET.fromstring(archive.read("xl/_rels/workbook.xml.rels"))
        targets = {rel.attrib["Id"]: rel.attrib["Target"] for rel in rels}
        for sheet in book.findall("m:sheets/m:sheet", SHEET_NS):
            chunks.append(sheet.attrib.get("name", ""))
            target = targets[sheet.attrib[REL_NS + "id"]].lstrip("/")
            if not target.startswith("xl/"):
                target = "xl/" + target
            body = ET.fromstring(archive.read(target))
            for cell in body.findall(".//m:c", SHEET_NS):
                value = cell.find("m:v", SHEET_NS)
                if cell.attrib.get("t") == "s" and value is not None and value.text:
                    chunks.append(shared[int(value.text)])
                elif value is not None and value.text:
                    chunks.append(value.text)
    return "\n".join(chunks)


def deliverable_text(workspace: Path, copied: set[str]) -> str:
    chunks = []
    for path in sorted(workspace.rglob("*")):
        if not path.is_file() or path.is_symlink():
            continue
        relative = path.relative_to(workspace).as_posix()
        if relative in copied:
            continue
        suffix = path.suffix.lower()
        if suffix in {".txt", ".md", ".json", ".csv"}:
            chunks.append(path.read_text(encoding="utf-8"))
        elif suffix == ".docx":
            chunks.append(docx_text(path))
        elif suffix == ".xlsx":
            chunks.append(xlsx_text(path))
    return "\n".join(chunks)


CHECKS = {
    "A03": grade_a03,
    "A23": grade_a23,
    "A24": grade_a24,
    "A25": grade_a25,
    "A26": grade_a26,
    "A28": grade_a28,
    "A29": grade_a29,
    "A30": grade_a30,
    "A35": grade_a35,
    "A36": grade_a36,
    "A40": grade_a40,
    "A49": grade_a49,
    "A51": grade_a51,
    "CAT-MAIL": grade_mail,
    "CAT-COPY": grade_copy,
    "CAT-CONFLICT": grade_conflict,
}
