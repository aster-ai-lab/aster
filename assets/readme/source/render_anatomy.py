#!/usr/bin/env python3
"""Render Aster's static README anatomy sheets without external dependencies."""

from pathlib import Path
from xml.sax.saxutils import escape

OUT = Path(__file__).resolve().parent.parent
FONT = "-apple-system,BlinkMacSystemFont,'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif"
MONO = "ui-monospace,SFMono-Regular,Consolas,monospace"


class Sheet:
    def __init__(self, code, title, subtitle, height=680):
        self.height = height
        self.parts = []
        self.parts.append(f'<rect width="1200" height="{height}" fill="#fff"/>')
        self.parts.append(f'<rect x="14" y="14" width="1172" height="{height-28}" rx="10" fill="#fff" stroke="#111" stroke-width="1.4"/>')
        self.text(38, 45, code, 11, fill="#555", spacing=1.8, family=MONO)
        self.text(38, 79, title, 27, weight=600, spacing=.6)
        self.text(38, 106, subtitle, 13, fill="#555")
        self.line(14, 128, 1186, 128)
        self.line(14, height-48, 1186, height-48)
        self.text(38, height-23, "ASTER · SOURCE-LOCKED ARCHITECTURE · NTS", 10, fill="#555", family=MONO, spacing=.7)
        self.text(1162, height-23, code.split("·")[0].strip(), 10, fill="#555", family=MONO, anchor="end")

    def add(self, value):
        self.parts.append(value)

    def rect(self, x, y, w, h, cls="box", rx=4):
        styles = {
            "box": ("#f7f7f7", "#111", 1.1, "none"),
            "obj": ("#fff", "#111", 1.4, "none"),
            "band": ("#e9e9e9", "#111", 1.1, "none"),
            "ghost": ("#fff", "#555", 1, "6 4"),
            "hatch": ("url(#hatch)", "#111", 1.1, "none"),
            "plain": ("#fff", "none", 0, "none"),
        }
        fill, stroke, sw, dash = styles[cls]
        self.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-dasharray="{dash}"/>')

    def line(self, x1, y1, x2, y2, cls="fine"):
        if cls == "flow":
            self.add(f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#111" stroke-width="1.3" marker-end="url(#arrow)"/>')
        elif cls == "dashflow":
            self.add(f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#555" stroke-width="1.1" stroke-dasharray="5 4" marker-end="url(#arrow)"/>')
        else:
            self.add(f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#666" stroke-width=".75"/>')

    def path(self, d, cls="fine"):
        if cls == "flow":
            self.add(f'<path d="{d}" fill="none" stroke="#111" stroke-width="1.3" marker-end="url(#arrow)"/>')
        elif cls == "dashflow":
            self.add(f'<path d="{d}" fill="none" stroke="#555" stroke-width="1.1" stroke-dasharray="5 4" marker-end="url(#arrow)"/>')
        else:
            self.add(f'<path d="{d}" fill="none" stroke="#666" stroke-width=".75"/>')

    def text(self, x, y, value, size=13, fill="#111", weight=400, anchor="start", family=FONT, spacing=0):
        self.add(f'<text x="{x}" y="{y}" fill="{fill}" font-size="{size}" font-weight="{weight}" text-anchor="{anchor}" font-family="{family}" letter-spacing="{spacing}">{escape(str(value))}</text>')

    def multi(self, x, y, lines, size=12, leading=17, **kwargs):
        for index, value in enumerate(lines):
            self.text(x, y + index * leading, value, size, **kwargs)

    def section(self, x, y, title, subtitle=""):
        self.text(x, y, title, 13, weight=600, spacing=1.2)
        if subtitle:
            self.text(x, y+19, subtitle, 10, fill="#555", spacing=.7)

    def box(self, x, y, w, h, title, subtitle="", cls="box", title_size=13):
        self.rect(x, y, w, h, cls)
        self.text(x+w/2, y+(h/2-2 if subtitle else h/2+5), title, title_size, weight=600, anchor="middle", family=MONO)
        if subtitle:
            self.text(x+w/2, y+h/2+17, subtitle, 10.5, fill="#444", anchor="middle")

    def save(self, filename, title, desc):
        body = "\n  ".join(self.parts)
        source = f'''<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="{self.height}" viewBox="0 0 1200 {self.height}" role="img" aria-labelledby="title desc">
  <title id="title">{escape(title)}</title>
  <desc id="desc">{escape(desc)}</desc>
  <defs>
    <marker id="arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto" markerUnits="strokeWidth"><path d="M0,0 L8,4 L0,8 Z" fill="#111"/></marker>
    <pattern id="hatch" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><line x1="0" y1="0" x2="0" y2="8" stroke="#b8b8b8" stroke-width="2"/></pattern>
  </defs>
  {body}
</svg>
'''
        (OUT / filename).write_text(source, encoding="utf-8")


def control_loop():
    s = Sheet("01 · CONTROL LOOP", "受控执行循环", "模型提出下一步；Core 判定权限；工具产生结果；事实日志驱动后续状态。", 700)
    s.section(42, 158, "PLAN VIEW / 一次任务的反馈回路", "SOLID = CONTROL PATH · DASHED = WAIT FOR ANSWER")
    s.box(48, 198, 190, 66, "USER / HOST", "task · confirm · answer")
    s.box(292, 198, 200, 66, "INFER", "model proposal", "band")
    s.box(546, 198, 210, 66, "K1 · PERMISSION", "allow / confirm / deny", "hatch")
    s.box(824, 198, 250, 66, "TOOL EXECUTION", "workspace · MCP · program")
    s.line(238, 231, 292, 231, "flow")
    s.line(492, 231, 546, 231, "flow")
    s.line(756, 231, 824, 231, "flow")
    s.box(824, 346, 250, 66, "F1 · FACT LOG", "record result + effect")
    s.box(292, 346, 200, 66, "VIEW + ACTIONS", "fold current state")
    s.box(48, 346, 190, 66, "NEXT INFERENCE", "updated context")
    s.line(949, 264, 949, 346, "flow")
    s.line(824, 379, 492, 379, "flow")
    s.line(292, 379, 238, 379, "flow")
    s.path("M143 346 V304 H392 V264", "flow")
    s.box(546, 346, 210, 66, "PARK / WAIT", "answer fact · no timer", "ghost")
    s.path("M651 264 V318 H641 V346", "dashflow")
    s.path("M756 379 H774 V412 H874", "dashflow")
    s.text(962, 313, "ALLOW", 10, family=MONO, fill="#555")
    s.section(42, 470, "DETAIL A / 修改后的完成门禁", "NO SUCCESS BEFORE EVIDENCE AND RESOURCE SETTLEMENT")
    s.rect(42, 494, 1116, 124, "obj")
    s.box(62, 522, 184, 60, "MUTATION", "changed paths")
    s.box(282, 522, 204, 60, "EFFECT DIGEST", "bind this exact change")
    s.box(522, 522, 250, 60, "VERIFY / WAIVER", "contract · attestor · explicit")
    s.box(808, 522, 308, 60, "K2 · SETTLE + COMPLETE", "cleanup · unresolved effects · reports", "hatch")
    s.line(246, 552, 282, 552, "flow")
    s.line(486, 552, 522, 552, "flow")
    s.line(772, 552, 808, 552, "flow")
    s.text(62, 603, "清理失败、结果不明的外部操作或与本次修改不匹配的报告，都不能发布成功。", 12)
    s.save("task-control-loop.svg", "Aster 受控执行循环", "用户输入进入模型推理，权限判定后执行工具，结果写入事实日志；修改完成后还需验证和资源收尾。")


def assembly():
    s = Sheet("02 · META HARNESS", "组件装配与单一事实日志", "宿主组合 Moore 组件树；运行时复用 AgentSession / FactRun，而不是另起一套消息循环。", 790)
    s.section(42, 158, "ASSEMBLY / 宿主输入到已准入组件图", "COMPONENT ORDER = ASSEMBLY ORDER")
    s.box(48, 184, 284, 58, "HarnessComposeOptions", "components · limits · system")
    s.box(374, 184, 284, 58, "HostHarnessRegistry", "registered host:<id>", "ghost")
    s.box(700, 184, 404, 58, "HostHarnessAssembler", "full tree · takes priority", "ghost")
    s.line(190, 242, 190, 268, "dashflow")
    s.line(516, 242, 516, 268, "dashflow")
    s.line(902, 242, 902, 268, "dashflow")
    s.rect(152, 270, 896, 40, "band")
    s.text(600, 296, "ADMIT · RESOLVE · HarnessGraph + KernelPolicy", 14, weight=600, anchor="middle", family=MONO)
    components = [("C1 · system", "prompt slots"), ("C2 · tools", "catalog view"), ("C3 · budget", "budget view"), ("C4 · compact", "threshold view"), ("C5 · infer", "scheduler")]
    for i, (title, subtitle) in enumerate(components):
        s.box(48+i*222, 348, 202, 68, title, subtitle, "band" if i == 4 else "box")
    s.text(1152, 436, "host:<id> · REGISTERED ONLY", 10, family=MONO, anchor="end", fill="#555")
    s.line(600, 416, 600, 444, "flow")
    s.rect(202, 446, 796, 42, "band")
    s.text(600, 473, "FOLD log → VIEW + UNIQUE KEYED TRANSITIONS", 13, weight=600, anchor="middle", family=MONO)
    s.line(600, 488, 600, 512, "flow")
    s.box(302, 514, 596, 48, "F1 · IMMUTABLE FACT LOG", "ingest · resume · one control source", "obj")
    s.section(42, 614, "DETAIL B / Moore 组件", "PURE STATE REBUILD")
    s.rect(42, 638, 344, 92, "obj")
    s.text(60, 664, "initial → step(fact) → output(state)", 12, weight=600, family=MONO)
    s.multi(60, 688, ["output = (view, keyed transitions)", "外部副作用只放在动作中"], 11, 17)
    s.section(426, 614, "DETAIL C / 动作选择", "KEYED + REPLAYABLE")
    s.rect(426, 638, 344, 92, "obj")
    s.multi(444, 664, ["key 必须唯一；排除已完成动作", "按 key 排序选择下一步", "infer/ 为子树分配命名空间"], 11, 20)
    s.section(810, 614, "KERNEL / 不可绕过的边界", "FAIL CLOSED")
    s.rect(810, 638, 348, 92, "hatch")
    s.multi(828, 664, ["K1：每次工具调用前判定权限", "K2：修改完成后检查证据与收尾", "未知组件或缺失注册表 → 拒绝"], 11, 20)
    s.save("meta-harness-assembly.svg", "Aster Meta Harness 组件装配", "宿主配方、注册表和装配器生成组件图，五个默认组件共享一份不可变事实日志，权限和完成门禁由 Core 保留。")


def extensions():
    s = Sheet("03 · EXTENSION SURFACES", "能力从哪里接入", "先选择正确的会话入口：资料、说明、实际操作、生命周期观察和组件装配不是同一件事。", 740)
    s.section(42, 158, "EXTENSION SELECTOR / 入口选择", "ALL ENTRIES JOIN THE EXISTING SESSION")
    rows = [
        ("E1", "Prompt slots", "角色、工作要求、回复格式", "system view"),
        ("E2", "ContextProvider", "项目、业务、检索资料", "context items"),
        ("E3", "SkillRegistry", "操作说明与流程知识", "instructions"),
        ("E4", "MCP / dynamic Tool", "真实外部操作能力", "tool catalog"),
        ("E5", "HookExecutor", "检查、改写、阻止或记录", "lifecycle"),
        ("E6", "host:<id>", "改变组件视图或动作", "Moore tree"),
    ]
    x, y, w, rh = 42, 184, 700, 58
    s.rect(x, y, w, rh*len(rows)+34, "obj")
    s.text(x+14, y+22, "CODE / ENTRY", 10, family=MONO, fill="#555")
    s.text(x+250, y+22, "USE FOR", 10, family=MONO, fill="#555")
    s.text(x+560, y+22, "SESSION SURFACE", 10, family=MONO, fill="#555")
    s.line(x, y+34, x+w, y+34)
    for i, (code, entry, purpose, surface) in enumerate(rows):
        ry = y+34+i*rh
        if i:
            s.line(x, ry, x+w, ry)
        s.text(x+14, ry+24, code, 11, weight=600, family=MONO)
        s.text(x+62, ry+24, entry, 12, weight=600, family=MONO)
        s.text(x+250, ry+24, purpose, 12)
        s.text(x+560, ry+24, surface, 11, family=MONO, fill="#444")
        s.text(x+62, ry+43, "HOST CONFIGURED", 9, family=MONO, fill="#777")
    s.section(794, 158, "TRUST BOUNDARY / 权限仍在 Core", "EXTENSIONS DO NOT GRANT THEMSELVES ACCESS")
    s.rect(794, 184, 364, 382, "obj")
    s.box(824, 216, 304, 58, "SESSION CAPABILITY SNAPSHOT", "selected for this run", "band")
    s.box(824, 314, 304, 58, "TOOL / MODEL / CONTEXT", "existing execution path")
    s.box(824, 412, 304, 58, "K1 · PERMISSION", "checked before every tool call", "hatch")
    s.box(824, 510, 304, 36, "HOST POLICY + LIVE CAPABILITY", "", "obj", 11)
    s.line(976, 274, 976, 314, "flow")
    s.line(976, 372, 976, 412, "flow")
    s.line(976, 470, 976, 510, "flow")
    s.box(42, 604, 348, 68, "SKILL ≠ TOOL", "说明不会自动注册执行实现", "obj")
    s.box(426, 604, 348, 68, "HOOK ≠ PERMISSION", "Hook 放行后仍执行 Core 权限检查", "obj")
    s.box(810, 604, 348, 68, "ID ≠ CODE LOADING", "模型写出名称不会加载宿主代码", "obj")
    s.save("extension-surfaces.svg", "Aster 扩展入口与信任边界", "提示、上下文、技能、工具、Hook 和宿主组件通过不同入口进入原会话，权限仍由 Core 判定。")


def verification_recovery():
    s = Sheet("04 · VERIFICATION + RECOVERY", "验证、资源收尾与恢复", "完成判定绑定本次修改；恢复重放控制状态，但不能凭空证明外部副作用的结果。", 790)
    s.section(42, 158, "COMPLETION / 证据与清理完成后才可成功", "DIGEST-BOUND VERIFICATION")
    s.box(42, 190, 174, 68, "MUTATION", "changed paths")
    s.box(250, 190, 186, 68, "EFFECT DIGEST", "exact change identity")
    s.box(470, 190, 256, 68, "VERIFICATION INPUT", "contract · attestor · waiver")
    s.box(760, 190, 180, 68, "K2 · GATE", "reports + effects", "hatch")
    s.box(974, 190, 184, 68, "RUN SETTLEMENT", "then complete", "band")
    for x1, x2 in [(216,250),(436,470),(726,760),(940,974)]:
        s.line(x1, 224, x2, 224, "flow")
    s.rect(42, 292, 1116, 54, "obj")
    s.text(60, 316, "send / stream / detached / resume", 12, weight=600, family=MONO)
    s.text(400, 316, "都等待所属 Run 的资源收尾；清理失败或超时阻止成功返回。", 12)
    s.text(60, 335, "DiskContentAttestor 只证明磁盘状态一致，不等于业务测试通过。", 10.5, fill="#555")
    s.section(42, 394, "RESOURCE LIFETIME / 两种资源寿命", "REGISTER CLEANUP BEFORE ACQUISITION")
    s.box(42, 420, 540, 76, "TURN · register_capability_effect", "当前模型轮次产生的工具调用结束后清理")
    s.box(618, 420, 540, 76, "RUN · register_run_capability_effect", "允许跨轮次使用；Run 收尾时清理")
    s.section(42, 548, "RECOVERY / 先区分要恢复什么", "REPLAY DOES NOT AUTHORIZE BLIND RETRY")
    s.box(42, 574, 344, 112, "SESSION REOPEN", "resume_session_async", "obj")
    s.multi(60, 662, ["恢复会话和原工作区", "宿主回调与外部连接需重新安装"], 11, 17)
    s.box(428, 574, 344, 112, "RUN CHECKPOINT", "resume_run / exact run id", "obj")
    s.multi(446, 662, ["继续检查点后的工作", "动作 key 决定哪些操作已完成"], 11, 17)
    s.box(814, 574, 344, 112, "UNRESOLVED EFFECT", "query original call / receipt", "hatch")
    s.multi(832, 662, ["外部写入结果不明时先核实", "直接重复可能造成重复提交"], 11, 17)
    s.text(42, 720, "确认与提问等待回答事实；计时器不能代替用户授权。取消请求也不等于资源已经清理。", 12)
    s.save("verification-recovery.svg", "Aster 验证、资源收尾与恢复", "修改摘要绑定验证报告，完成前等待资源清理；会话恢复、运行恢复和结果不明的外部操作分别处理。")


if __name__ == "__main__":
    control_loop()
    assembly()
    extensions()
    verification_recovery()
    print("rendered 4 static anatomy sheets")
