# 工作区 Wiki 与原知识模块端到端测试入口

当前工作区 Wiki 采用产物／看板／百科／记忆；已删除独立百科问答。当前合同见[工作区 Wiki](../../specs/021-workspace-wiki/contracts/workspace-wiki.md)，执行结果及未通过项见[2026-10-26/27 证据](../../docs/evidence/desktop/2026-10-26-workspace-wiki/OUTCOME.md)。下方 015 计划与个人知识夹具是历史／独立范围，不代表当前产品恢复独立问答。

`workspace_wiki_native.py` 通过正在运行的原生应用、真实 ACP 和配置的模型生成自有合成文件的 Wiki，校验秘密排除、来源不变、陈旧审阅拒绝、整代发布与无变化零生成；`--cancel-running` 另验运行中取消。它不操作 GUI，不替代原生桌面验收。仅使用为测试选择的 aster Agent；记录 token 是运行时凭据，不能提交或复制到证据中。

```sh
python3 tests/e2e/workspace_wiki_native.py --runtime "$TMPDIR/aster-studio/aster/runtime.json" \
  --agent-id YOUR_FIXTURE_AGENT_ID --evidence target/workspace-wiki-native.json
```

该命令会创建独立合成任务并调用所选真实模型，保留任务及其发布页面供检查，不向模型发送现有用户项目。后台增量、项目归档／恢复与重启另见当前证据中的独立原生接口记录。

`--fixture code`（默认）验证跨文件入口；`--fixture documents` 使用合成 DOCX 规则、CSV 台账和
Markdown 报告；`--fixture mixed` 再加入读取台账的 Python 脚本。每次独立创建任务，不复用候选页。
发布后逐条通过原生来源接口检查引用的当前性、页面及原文件摘要，并再次确认无变化不创建新任务。
脚本解释报告来源时必须区分“资料中声明派生”和“程序实际写出”；这些夹具不证明真实业务程序已执行。
运行 `python3 -m unittest discover -s tests/e2e -p 'test_workspace_wiki_native.py' -v` 可离线检查夹具及引用判定。

权威计划：[e2e-plan.md](../../specs/015-knowledge-workbench-ux/e2e-plan.md)。
完整清单：[e2e-cases.md](../../specs/015-knowledge-workbench-ux/e2e-cases.md)。

本入口保存完整规划；执行进度分别记录在[知识模块](../../docs/evidence/desktop/2026-10-24-knowledge-real-llm-e2e/OUTCOME.md)和[编辑器](../../docs/evidence/desktop/2026-10-25-knowledge-markdown-editor/OUTCOME.md)证据中，尚无全量通过结论或现成全量suite。
知识整理的参数、暂存目录、完成验证和发布闭环另见[整理回归](../../docs/evidence/desktop/2026-10-25-knowledge-maintenance-e2e/OUTCOME.md)。
146个场景族（含12条业务旅程）、全部参数变体、47项功能覆盖；其中8个目标场景族依赖尚未完成的产品能力。
144个族要求真实原生GUI参与，另2个仅为协议/owner故障补充，不当作原生E2E。

1. 先冻结功能/Office工具栏清单、隔离fixture、匹配构建及后台驱动能力。
2. 保留空工作区初始化等已知红例，完成正确owner修复；不得预建wiki绕过首次使用。
3. 用Aster Test MCP观察真实应用，再按当前观察执行；UI、真实磁盘/SDK/ACP、执行身份同时核对。
4. 脚本模型只替代模型供应方；真实Agent、工具、存储和百科发布仍执行。带L的用例另跑真实模型。
5. 稳定路径再生成私有Studio里的`native_*.acl`，先`aster-test check`再用相同可信host配置`run`。
6. 参数变体、失败/阻断/待实现与实际通过分别计数；组件测试、旧stub API测试或截图不代替原生业务验收。

现成可执行的预检和底层回归命令、未来runner的明确缺口、清单之外新增功能的登记规则、
进程归属/清理、异常恢复、真实IME/拖放能力门及发布门槛均见权威计划。
本入口不另建测试框架，不声称尚未观察的ACL已经可以运行，不恢复此前暂停的产品实施目标。

## 本地知识模型夹具

`knowledge_model.py` 只提供明确的 `knowledge-basic-ingest-v1` 模型用例。
它要求真实 ACP 先读取 `.wiki-task.md` 和 `实验室计划.md`，再通过真实文件工具
写入暂存页和 `.wiki-changes.json`。输入原文在模块的 `SOURCE` 常量中；不预置输出。
工具回执按宿主返回的调用身份、参数、退出码和写入字节数核对。
无工具的辅助请求只返回空记忆，不证明记忆提取或预分析能力。

```sh
python3 -m unittest discover -s tests/e2e -p 'test_*model.py' -v
python3 tests/e2e/knowledge_model.py --events target/knowledge-e2e/model-events.jsonl
```

服务只绑定动态 loopback 端口，启动时输出模型名和地址。它只替代模型供应方；
原生 GUI、暂存 owner、发布合并及独立磁盘校验仍须执行，不能以该夹具代替真实模型。

协议补充使用 `cargo run --locked --offline -p aster-okf --example knowledge_maintenance_fixture -- begin ROOT`
创建真实暂存任务，ACP 结束后用同一入口的 `finish ROOT JOB_ID` 发布。`ROOT` 必须是自有隔离的
`wiki/local` 测试资料库；此入口不会预置候选页或绕过发布校验。ACP 的 `session/new` 必须像
原生 Studio 一样声明 `_meta.asterHost.completionAttestation = "disk_content"`，并对实际写入逐次审批。
这些补充命令不构成完整 GUI runner。
