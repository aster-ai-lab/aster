<a id="tt-用例矩阵desktop--asterdream-共用-id"></a>

# TT 用例矩阵（旧版 AsterDream / 共享宿主共用用例 ID）

| ID | 步骤 | 断言 |
|----|------|------|
| TT-01 | 首页 Composer 发送进入任务 | `#workbench-sidebar` 折叠（`aria-hidden=true`） |
| TT-02 | 侧栏展开后点历史会话 | 侧栏保持展开 |
| TT-03 | TT-01 后 reload | 仍折叠 |
| TT-04 | held 长轮次 | Composer 占位含「排入下一条」或英文 queues 句；不含「Agent 运行中」；pause 可见 |
| TT-05 | held 中已有成功 tool | 最新 turn：`data-task-turn-state=live` 且 `task-pending` 可见 |
| TT-06 | held 末尾 prose 看似写完 | 仍 live + `task-pending` |
| TT-07 | pause 或 release 到 done | `task-pending`=0；turn idle；send 恢复 |
| TT-08 | 最后一块为 approval pending tool | 无 `task-pending` |
| TT-09 | live 时 | aria-label=正在处理；存在 `task-pending-verb`；不锁死具体动词 |
| TT-10 | 流仍 live、非最新助手行已 success | 旧行无 `task-pending`；仅最新行可 awaiting |

触发 held：用户句含「保持进行中」。
Approval 变体：用户句含「保持审批」。
