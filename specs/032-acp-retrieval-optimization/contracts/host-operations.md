# Host operations：保留现有契约，按需补最小入口

## 1. 既有路径

| Agent 入口 | ACP 到宿主 | owner／规则 |
| --- | --- | --- |
| `search` | Core WorkspaceServices | 当前工作区 search modes、manifest／source verification，semantic 受信任 opt-in。 |
| `search_wiki` | `DocumentBridge::search_workspace_wikis` → `aster/wiki/search-workspaces` | 宿主解析 current／all、原有 Wiki publication 与 library binding；保留 cursor／pending／complete。 |
| `search_memory` | `DocumentBridge::search_workspace_memories` → `aster/memory/search-workspaces` | 宿主解析 profile、原有 durable owner 与 live session／working owner，不恢复关闭任务。 |
| `office_read` | 原生 Office 或 admitted document host | PDF／OCR 使用宿主资源，来源 digest 与页／node continuation 保真。 |
| `studio_document_read` | 既有 editor read bridge | 当前编辑模型／未保存文本；不能冒充持久 Office／PDF 解析结果。 |

本表不是新注册表。实施在原代码的闭集与测试中维护映射；提示词不授予能力。

## 2. SAG 原文检索的决策与候选增量

T006/T016 先运行“Agent 找到宿主资料原文片段”用例。若已有 operation 满足，就只补验证和开销优化。Wiki 生成页可作为派生知识，但不能伪装为原文 SAG 的结果。

若确认缺桥，候选设计如下，在 T016 的 contract test 中先冻结再实现：

- Agent 工具候选名 `search_sources`，只读；对应宿主候选 operation `aster/library/search-sources`，通过已有 `DocumentBridge`／`turns.rs` 分派。二者是拟定新增项，当前未实现。
- 宿主 metadata 增加显式、默认 false 的资料搜索能力（候选字段 `sourceSearch`）；必须同时满足实际 bridge 存在、该会话读范围获准。不能借用 `workspaceWikiSearch=true` 自动扩大权限。
- 请求只含 session 关联、query、typed scope、limit 和 opaque cursor；宿主解析资料根目录与 binding。候选范围值优先复用现有 `current`／`all`；不接收模型给出的任意文件系统根。
- host 内使用原 SAG `Index`、原 source generation／publication 和 model identity；返回原文片段、source ID、path、revision、页／行／passage anchor、通道及 coverage。数据在边界校验一次；不将整个资料目录传入 ACP 再索引。
- 优先沿用现有搜索上限：query ≤2,000 chars、最多 12 hits、片段预算 ≤48 KiB、外层 response ≤64 KiB。若 SAG 原接口更严格则取更严格值；cursor 和 query／scope 绑定。
- timeout／cancel 沿当前 access／run owner 传播；未建索引、部分失败和 continuation 使用已有语义可表达的字段，不靠渲染错误字符串推断重试。
- 旧客户端省略新能力即保持旧工具列表。撤销／断连／scope 变化按当前 admitted invocation 规则拒绝新调用，旧回执必须核对其原 request／source binding。
- 对应正反例：Q04、Q05、Q19、Q20、Q28、Q29。没有真实源片段和 host route 证据时不记完成。

这是待证实缺口的最小协议方案，不是创建通用 search service 或注册中心的理由。

## 3. 记忆语义启用与兼容决策

T021 必须对照 `LazyFileMemoryStore` 与 Core 已有 `DurableMemorySemanticRecall`／refresh API。默认保留现有 MemoryStore 为权威；typed v2 的 Active-only 政策和存量记录政策不一致时，不能自动搬库、镜像或升级状态。

复用已有 provider／vector／refresh 机制。若接口不兼容，先在原 owner 收敛共享原语，再用薄 adapter 保持旧公开 API 与记录政策；不因此另建一个类似投影、刷新流程或权威 repository。算法不复制到测试中。

记忆语义策略必须由 owner 明确声明，默认关闭。先复用现有 typed owner policy；若 ACP 本地确实没有可配置入口，最小候选为受信任 ACL 的 `memory { semantic_recall = true }`，并要求已选本地 embedding backend。此字段是候选，T021 核对现有 codec 后确定；不得默认继承 `workspace_retrieval.enabled` 的启用含义。模型／workspace 文本不能开启它；既有 memory 配置项不被整体改成另一种信任语义。

改变模型、tokenizer、分块／规范化或源 revision 时，派生状态必须失效。旧 embedding 列填零的兼容逻辑不能用于声称 semantic ready。只读 owner 不写回；失败保留原记录和词法路径。配置 parser／render／layer trust／session binding 和宿主调用者的测试同批完成。
