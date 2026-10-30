# Plan consistency review

Date: 2026-10-26. Reviewed spec.md, plan.md and tasks.md without changing their requirements.

| Finding | Severity | File / task | Resolution |
| --- | --- | --- | --- |
| 所有 FR-001–FR-010 均有 producing task，四个 user story 和最终门禁都有执行及证据任务 | note | tasks.md coverage table | 可以按任务顺序开始 |
| 既有 catalog 仅 25 项，扫描 PDF 与部分评分标准尚未就绪 | warning | T001,T004,T005 | 明确扩展到 28 项；未完成不计通过，执行 T004/T005 |
| 既有 SettingsDrive/CatalogDrive 有可能按多个匹配中的第一项寻址，某些旧窗口恢复路径使用非 CUA 操作 | warning | T003,T006 | 本轮动作必须绑定唯一当前窗口和唯一语义目标；歧义、过期或窗口丢失重新观察/停止，不使用坐标或旧前台恢复路径 |
| 旧模型代理手工解析 ACL，且对应 ToYou gateway | warning | T002,T003 | 本期明确不使用；复用 Aster 可信宿主配置和原 ACL parser |
| 历史断言不能作为当前版本通过，source/provider/renderer 改变后需要重新核对资格 | note | T007,T008,T009 | 每次尝试保留实际身份，重验受影响集合 |

**Verdict**：ready for implement。未发现规划中的 NON-NEGOTIABLE 违反或未覆盖需求。warning 是明确待执行的边界工作，不是通过或豁免；28 项资格仍未完成。
