# Aster 开放 issues 修复计划

用户目标：发布完成后，修复 `aster-ai-lab/aster` 全部未关闭 issues。发布是实施前置步骤。初次清单为 #1–#7，无评论；完整正文见 [ISSUES.json](ISSUES.json)。后续新增问题在结束前再次盘点，不靠关闭记录替代代码验证。

## 范围与验收

| Issue | 根因与验收重点 | 负责 |
| --- | --- | --- |
| #1 | 自动目标 continuation 具备真实停滞边界；verified 后无新交付改动可结束并写终态；只读重验/计划文字不重置进展；实际持续修改不被误停。保留 ACL 配置、手动 goal 和完成门禁语义。 | 原生/目标代理 |
| #2 | headless 与 TUI 对非 Grok、空 auth_methods 的本地 ACL 认证策略一致；需要登录的 agent 保持拒绝；隔离 ACP/provider stub 验证有输出与正常退出。 | ACP/CLI 代理 |
| #3 | rules 与 systemPromptOverride 在实际 ACP 握手/会话路径到达模型；明确 append/replace 语义、输入校验与信任边界。 | ACP/CLI 代理 |
| #4 | program 内置模板可发现并按名运行，仍走既有工具调用与权限治理；保留通用 PTC，并验证未知模板/非法参数拒绝。 | ACP/CLI 代理 |
| #5 | 同进程同集合多会话共享安全句柄，召回和提炼可用；不改存储格式、LLM 策略与跨进程锁。 | 原生/目标代理 |
| #6 | gateway 三项内部依赖使用本地路径，相关成员不可发布，锁文件无 registry A3S 副本；根工作区双守卫通过；不放宽依赖守卫。 | 依赖/CLI 代理 |
| #7 | workspace 未设置开关时明确报告功能门控，保留 Unknown/Disabled 与服务器语义、退出码；不引导错误的网络/登录排障。 | 依赖/CLI 代理 |

## 开发约束

遵循 AGENTS、constitution 与开发标准的 product regime；不编辑云/API 控制平面，不增加工作区、配置语言、依赖、全局工具次数上限或猜测语义的词表。修改前追踪调用方，分别保留小型 hermetic 回归与修复前证据。各代理独占文件；共享文件先协调，其他任务在途修改保留。

发布只使用已冻结的 R4 包；新 issues 修复不回写该安装包或改动 v0.1.2 标签。新行为在所属模块测试，并做独立对抗审查；相关测试通过后执行根格式检查和工作区 CI 守卫。失败单独复核，既有失败不当作本次通过。

## 当前状态

[v0.1.2 已公开发布](https://github.com/aster-ai-lab/aster/releases/tag/v0.1.2)，五附件服务端校验及公开下载确认完成。#1–#7 本地修复与定向验收完成，结束盘点没有新增 issue；未提交、推送或关闭 GitHub issues。根格式、成对构建、四场景真实 CLI、目标/记忆/模板及来源守卫结果见 [OUTCOME.md](OUTCOME.md)。完整 Python 5/6，既有 tenant 资产缺失原样保留。基线见 [BASELINE.json](BASELINE.json)。
