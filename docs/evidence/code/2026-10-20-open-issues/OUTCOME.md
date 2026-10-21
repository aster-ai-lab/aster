# Aster 开放 issues 修复结果

2026-10-20。发布前置步骤已完成：[AsterDream v0.1.2](https://github.com/aster-ai-lab/aster/releases/tag/v0.1.2) 的麒麟 x86_64 安装包及另外四项附件已公开，五项服务端大小与 SHA-256 均符合冻结 R4 产物。[发布收据](../../work/2026-10-20-kylin-ime-hardening/RELEASE.md) 单独记录该交付。下面的 issue 修复发生在发布之后，未回写该安装包、标签或 updater feed。

初次及结束前的开放清单都是 #1–#7，没有新增项：[原始正文](ISSUES.json)、[结束盘点](issues-final.json)。这七项的本地源码修复与定向验证已完成；没有提交、推送、评论或关闭 GitHub issues。工作区原有和其他线程新增的修改均保留，不能把整个 dirty tree 当作本次交付。

## 修复与证据

| Issue | 根因与最终行为 | 验证 |
| --- | --- | --- |
| [#1](https://github.com/aster-ai-lab/aster/issues/1) | unattended 默认停滞上限由 1000 改为 3，保留 1000 slice ceiling、期限及显式 ACL 优先级。只有当前 slice 的完整原始回执证明实际文件身份变化，才刷新进展；真实改动不依赖 goal judgment 接受，同字节重写、只读验证和计划文字不算进展。停滞且目标未达成时，写 `failed / execution_failed`，明确 `next=needs_input`，不伪造 Completed。 | 最终 [control 40](issue-1-control-after-final.log)、[execution 51](issue-1-execution-after-final.log)、[receipt 4](issue-1-receipts-after-final.log)、[unattended unit 29](issue-1-acp-unit-after-final.log)、[unattended E2E 8](issue-1-acp-e2e-after-final.log) 均通过。 |
| [#2](https://github.com/aster-ai-lab/aster/issues/2) | headless 对非 Grok 的空 auth_methods 复用 TUI 本地 ACL 策略；Grok、已声明的认证方法和禁止浏览器交互的边界保留。 | [headless 35](issue-2-after.log) 通过；真实成对 CLI + ACP 的 ACL-only、ACL + 合成 API key 均成功，无超时。 |
| [#3](https://github.com/aster-ai-lab/aster/issues/3) | ACP 严格校验并消费 initialize rules，使用共同会话 options 追加到 new/load/replace 路径；保留宿主合同和权限。非空完整 systemPromptOverride 在握手明确拒绝并指向追加参数。 | [setup 10](issue-3-after-setup.log)、[wire 4](issue-3-after-wire.log) 通过。真实本机 Anthropic fixture 的五次请求均只有一份规则；成对 CLI 另确认 rules 到达实际 OpenAI-compatible 请求、override 在模型调用前拒绝。 |
| [#4](https://github.com/aster-ai-lab/aster/issues/4) | program 保留并公布准确模板目录、参数和默认值，可按名实例化两个内置模板。实例数据作为 JSON 进入原 QuickJS / scoped nested tool 链；schema、权限、预算、取消和递归限制保留。code presentation 同时保留目录描述。 | [program 88](issue-4-after-final.log)、[presentation 5](issue-4-after-presentation.log) 通过，包括真实模板、schema、引号/中文数据及受控拒绝路径。 |
| [#5](https://github.com/aster-ai-lab/aster/issues/5) | Lazy memory adapters 按 canonical directory 共享弱引用注册的 Arc 句柄；首次打开/旧数据导入/发布串行，失败可重试。最后释放与新打开使用同一锁，避免死 Weak 与尚未析构完的集合竞争。直接 VecMemoryStore 独占和存储格式不变。 | [memory 17](issue-5-core-after-final.log)、[真实 ACP 双会话 1](issue-5-acp-after-final.log) 通过，涵盖并发首次使用、共享读写、失败重试、释放后重开及有界关闭竞争。 |
| [#6](https://github.com/aster-ai-lab/aster/issues/6) | Gateway 三项内部依赖改为本地 path，保留版本/API/optional；Gateway、Sentry、Updater 不可发布。锁文件移除 registry A3S 副本。Work 守卫核对真实 Cargo ownership，合法 gitfile 不再误判为第二工作区。 | [内部来源 Python 精确守卫](issue-6-local-guard-after.log)、[Rust workspace 守卫](issue-6-rust-workspace-after.log)、[Gateway wire 离线检查](issue-6-gateway-wire-check.json) 通过。完整 Python 为 5/6，见下方已知限制。 |
| [#7](https://github.com/aster-ai-lab/aster/issues/7) | workspace Unknown gate 提示开关未确认及既有本地开启方式，保留 Unknown/Disabled、env/server 优先级、sandbox 和退出语义。 | [4 项门控回归](issue-7-after.log) 全通过。没有启用隐藏功能或操作真实 leader。 |

各行是独立过滤范围，不将 before、interim、exact 复跑、相互覆盖或子进程输出重复加总。详细根因与命令见 [#1/#5 报告](ISSUES-1-5.md)、[#2–#4 报告](ISSUES-2-3-4.md)、[#6 报告](ISSUE-6.md)、[#7 报告](ISSUE-7.md)。机器命令和身份见 [#2–#4 机器收据](issue-2-3-4-targeted-results.json)、[core 最终测试二进制身份](issue-1-5-core-final-validation.json)、[双会话身份](issue-5-acp-final-validation.json)。

## 对抗审查与兼容

[#2–#4 独审](ISSUES-2-3-4-INDEPENDENT-REVIEW.md) 发现并闭合实际 code projection 丢目录的问题。[#1/#5 独审](ISSUES-1-5-INDEPENDENT-REVIEW.md) 又闭合最后 Arc 析构竞态和旧目标日志兼容问题。

旧 small-cap 日志可能包含同字节 write 后接受的步骤；直接更换历史进展含义会使随后原本合法的 admission 无法重放。维护回归在修改前准确失败，[修复后通过](issue-1-replay-after.log)。私有 typed progress basis 缺省省略，保留旧 digest 和旧 admission 校验；新宿主明确使用实际 mutation 语义。两种计数分别派生，新准入不能借旧 accepted-receipt reset 绕过停滞边界，未知 basis 拒绝。ACL 字段和公开终态 schema 未改变。

Issue #1 描述的“所有 StepAccepted 无条件重置”在本次基线已经不存在，未冒充本次新增修复。新增修复处理的是默认 1000 停滞预算、真实未接受 mutation 的进展，以及同字节写入等边界。

最终 default-goal E2E 先实际创建 answer.txt 并达到 verified，再只读重验；目标 judgment 故意不接受。最终 4 slices、1464 ms 写出明确未完成结果，60 秒期限尚未耗尽，且没有 StepAccepted/Completed。修复前同一前提在 10 秒测试边界仍空转、没有结果文件；原始前提及合成事实均保留。

## 最终成对 CLI 与检查

- `cargo build --locked -p aster -p a3s-code-acp` 退出 0。[构建收据](paired-build-final.json)
- [真实 CLI 四场景](headless-after-result.json) 全部符合预期：前三项退出 0，分别用时 6.707、0.417、0.413 秒；override 在 0.163 秒明确拒绝且 0 次 provider 调用。每个成功场景恰好一次有效 pre-analysis 和一次主 streaming 请求，没有规划 fallback。使用独立临时 HOME/XDG/ACL/workspace、合成 key 和回环 provider，40 秒单场边界。[可复现 probe 与范围](probes/README.md)
- `cargo fmt --all -- --check` 退出 0；最终补充注释后重查。[格式收据](fmt-final.json)
- 自有 41 文件哈希与实现代理 manifest 一致，scope `git diff --check` 退出 0。[源码快照](source-final.json)
- Cargo.lock 保留另一个任务原有的六项新增依赖；main.rs 门控区域之外逐字节保留。[锁文件核对](issue-6-lock-semantic-preservation.json)、[main 核对](issue-7-main-preservation.json)。这两项共享文件后续需要按本次 hunk 暂存，不能整份混入其他任务。

## Explicitly not claimed

- **完整 Python 守卫不是全绿：5/6。** 唯一失败为既有 `apps/web/public/tenant/legacy/INFO.md` 缺失。历史提交 e07fd7c 明确移出 Web 树，但 Desktop/tenant 契约仍有旧引用；没有造占位资产、删除断言或把该迁移扩展为本次工作。[原样结果](issue-6-python-after.log)
- 没有运行整仓库全部测试、外部真实模型/账号、完整模板 HITL 或物理麒麟设备验收。模板新增 scoped-deny fixture 不冒称完整 HITL；共享真实 session script 治理回归另有覆盖。
- #2 附带的 180 秒挂起没有复现，不声称已定位其独立根因。本次明确验证修复后的有界本地 ACL / API key 场景正常退出。
- 没有实现完整系统提示替换；现在明确拒绝，避免静默忽略。
- 七项修复目前在本地工作区，未提交推送、关闭 GitHub issues 或发布为另一个安装包。已发布 v0.1.2 继续指向冻结的麒麟 R4 交付，不能声称包含此后修复。
