# 知识模块完整端到端测试计划

**日期**：2026-10-24。**状态**：规划完成待实施，本计划没有运行端到端场景、修复产品或恢复此前暂停的实施目标。
**依据**：[015 spec](spec.md)、[UX](ux.md)、[018 本地记忆](../018-local-memory-studio/spec.md)、[012 百科发布](../012-living-knowledge-wiki/spec.md)、[JSON Render](../../tests/e2e/JSON_RENDER.md)。
**目录**：[146个场景族](e2e-cases.md)，其中144个要求真实原生GUI参与、2个为协议/故障补充；12条完整业务旅程包含在146之内，不额外相加。参数化变体分别出结果。
**范围**：当前实际可见的笔记、百科、记忆全部能力、跨页业务、共享Office编辑器、条件云端入口、实际aster/ACP及其已承诺的后续功能。没有新增手动记忆编辑、批量清空、云同步等产品范围。

## 1. 验收结论如何成立

每个业务场景同时取得：

1. **真实操作证据**：从主导航进入真实原生窗口，执行当前具名控件、快捷键、确认与取消；不是组件挂载、DOM注入或API替用户按保存。
2. **持久结果证据**：核对授权工作区文件/发布索引的实际字节、版本与变更集合，或通过真实SDK/新ACP重开记忆。取消、拒绝和失败验证零额外写入。
3. **执行身份与因果证据**：核对tenant、instance、canonical workspace、job/generation、request/tool_call_id、模型/工具事件和实际owner回执。UI显示成功、文件存在、模型说完成，单独任何一种都不够。

`G`为原生GUI链路，`D`为真实进程/协议回读，`L`为另加真实模型运行，`H`为owner受控故障补充。D/H用来证明边界，不能代替G；含L的场景也先跑本地确定性模型版本，再独立跑真实模型版本。两种模型结果分列，不能互相替代。

现有100项前端通过、472项较广前端检查和历史原生局部通过只作基线。带拦截请求、`window.api`注入、模拟Tauri的测试标记为组件/服务集成，不重新命名为原生E2E。

## 2. 两个完整性门

- **现有能力门E**：所有当前用户可见动作、既有业务与明确拒绝行为均有场景和真正结果核验。缺入口、点击不生效、400初始化、假空态、错误范围都算产品失败。
- **承诺目标门T**：`KG09/KG10`多文档预览/固定与历史，`KR10`安全自动保存，`KW16–KW18`百科层级，以及`KJ11/KJ12`对应完整旅程。当前待实现；必须完成契约、实现和验收后才能宣称“全部功能与业务流程通过”。

P0/P1/P2仅决定次序，不是豁免。文件夹整体移动、未支持的发布引用改名、记忆手动增改等当前明确未支持的操作验证安全拒绝；这不证明未来对应成功能力已经具备。新增可见动作先补功能清单和场景，再进行全量验收，禁止用词表猜功能归属。

## 3. 当前必须保留的阻断

| 问题 | 对应场景 | 当前处理 |
| --- | --- | --- |
| 空工作区`wiki/local`准备走通用创建API，`wiki`被保留名规则拒绝400 | KG01、KJ01 | 先跑真实红例；在正确owner修复后再绿验收。不得预建wiki、吞400或放宽通用保留名保护 |
| 首次原生观察出现空白，历史运行通过额外Tab才绘制 | KG04、KP01 | 冷启动仅观察直到规定截止时间；不得按Tab/点窗口唤醒后算启动通过 |
| 新版百科生成→发布→审核整条链路未通过原生验收 | KW04–KW15、KJ06 | 有效Agent/模型、真实暂存与merge、当前发布字节和历史全部核对 |
| 原生正常退出/异常终止恢复未完整验收 | KR01–KR09、KJ05 | 先完成写入/退出保护；未通过不得启用自动保存或脏预览替换 |
| 分类层级、多文档历史与自动保存尚未完成 | T门8个场景族 | 待实现，不计skip、expected-failure或已通过 |
| 后台驱动对真实拖放、右键、IME、缩放/尺寸等能力不足 | KU02、KU04/05/08/09 | 先证明能力或接入受认证且同样后台约束的worker；缺能力记基础设施阻断，不用模拟事件补过 |

原故障证据保留于[统一实现记录](../../docs/evidence/desktop/2026-10-24-knowledge-unified-implementation/OUTCOME.md)和[实际记忆/ACP记录](../../docs/evidence/desktop/2026-10-24-knowledge-native-profile/OUTCOME.md)。修复不删除红例，也不扩张任何legacy allowlist。

## 4. 功能覆盖矩阵

下列每个功能至少对应一个G场景；支持性协议/原子故障另有D/H。范围限定与所有条件入口也进入清单。实际UI/Office/命令注册新增动作造成未映射项时，全量覆盖检查失败。

| 功能ID | 明确功能 | 场景 |
| --- | --- | --- |
| F01 | 主导航、三Tab、默认/最近入口、自动绘制 | KG02、KG04、KG08、KU01 |
| F02 | 顶栏图标、提示、完整菜单/快捷键及搜索位置 | KG03、KU03、KU05–KU07 |
| F03 | 跨资源保留草稿/选择/查询、工作区/租户/实例切换 | KG05–KG08、KQ06 |
| F04 | Markdown新建、名称/扩展名/保留名/碰撞、失败重试 | KG01、KN01–KN03 |
| F05 | 文件夹树、展开折叠、深层/末尾滚动、文件夹移动拒绝 | KN04、KN16、KU09 |
| F06 | 确切身份阅读、空文件、长文、不可读与删除 | KN05–KN07、KN19 |
| F07 | Office全部实际工具栏动作、插入/取消、三种编辑模式、撤销重做 | KN08、KN09、KU06 |
| F08 | 标题、文件名与frontmatter身份区别 | KN10、KN14 |
| F09 | 图标/快捷键保存、确认时间、保存中继续编辑、外部冲突 | KN11–KN13 |
| F10 | 文件改名/移动、审核影响、碰撞、journal恢复 | KN14、KN15、KN17、KS02、KS10 |
| F11 | 删除/取消、草稿处置、外部删除后的另存 | KN18、KN19 |
| F12 | 大纲、标签、限定/歧义/别名/锚点链接、反链、未解析目标、图谱 | KN20–KN22 |
| F13 | 条件云端目录、显式下载/安装、取消和坏包 | KN23、KN24、KS12 |
| F14 | 全库标题/正文检索、中文/Unicode与匹配位置 | KQ01–KQ03、KP02 |
| F15 | 标题/路径快速打开、最近身份、快捷键、取消及焦点 | KQ04、KQ05 |
| F16 | Notes/published查询与列表位置独立 | KQ06 |
| F17 | 目录/标签过滤、所有排序和清除条件 | KQ07、KQ08 |
| F18 | 完整分页、稳定顺序、精确计数与续页身份 | KQ09、KQ11、KP05 |
| F19 | 搜索迟到、失效、忙、过期、部分覆盖与重试 | KQ10–KQ12 |
| F20 | 草稿确认、正常/异常重启、比较/恢复/放弃、范围隔离 | KR01、KR04–KR09 |
| F21 | 真正退出、窗口关闭、renderer失联、未确认/清理失败 | KR02、KR03、KR09、KU11 |
| F22 | 安全自动保存及整理去重（T） | KR10、KJ11 |
| F23 | 多文档预览/固定/关闭、后退前进（T） | KG09、KG10、KJ11 |
| F24 | 百科已发布列表、只读正文与原始来源返回 | KW01–KW03 |
| F25 | 首次/增量自动整理、未变内容、暂停/失败/恢复与发布 | KW04–KW06、KA08–KA10 |
| F26 | 候选比较、采用、保留、代数/证据校验、连续处理 | KW07–KW12 |
| F27 | 资料搜索/状态/分页/源集选择、历史、检查百科 | KW13–KW15 |
| F28 | 显式主题/子主题/未分类、面包屑、层级搜索与分类变化（T） | KW16–KW18、KJ12 |
| F29 | 真实记忆空态、全部分页、稳定顺序、详情 | KM01–KM03、KM06 |
| F30 | 记忆字面内容查询、显式提交、工作/会话/持久三层、四种类型及清除 | KM04、KM05 |
| F31 | 取消/确认删除、持久回读、邻项/空态、缓存失效 | KM07–KM11 |
| F32 | 无模型管理、live ACP、同一SDK fallback及限时 | KM12–KM14 |
| F33 | 记忆profile/工作区隔离、旧终端目录与旧会话 | KM15、KM16 |
| F34 | 记忆详情/页/扫描限制、坏数据和未知契约 | KM17、KS07、KP07 |
| F35 | 真实Agent工具写入笔记 | KA01、KJ08 |
| F36 | 真实Agent合法提取、存储与后续召回记忆 | KA02、KJ07 |
| F37 | ask_user、JSON Render校验/提交/取消/重放及跨页 | KA03–KA07、KJ08 |
| F38 | 阅读零模型请求、源代数调度、会话sender身份与生命周期 | KA08–KA10 |
| F39 | 路径/根替换/私有目录/原子写/发布引用边界 | KS01、KS02、KS06、KS09、KS10 |
| F40 | 跨scope、窗口capability、构建/进程绑定与调用者 | KS03、KS04、KS08、KS12 |
| F41 | 不执行内容、未知输入、敏感数据/日志/提取authority | KS05、KS07、KS11 |
| F42 | 键盘、真实IME、读屏、焦点、右键/拖放/滚动 | KU01–KU03、KU06–KU09 |
| F43 | 宽窄/400%/长文案/主题/减少运动及所有状态布局 | KU04、KU05、KU10、KU11 |
| F44 | 不抢前台、不动指针、精确进程清理 | KU12、KS08 |
| F45 | 5000篇/约50MiB、真实UI时延、更新滞后、内存/资源与并发 | KP01–KP08 |
| F46 | 十二条可独立重置/执行/核验的完整业务旅程 | KJ01–KJ12 |
| F47 | 当前不支持能力的明确拒绝/不呈现 | KN02、KN16、KM18、KS10 |

## 5. 环境矩阵和执行分层

| 层 | 环境与用途 | 不允许替代的部分 |
| --- | --- | --- |
| L0 | 现有Core/OKF/ACP/native/Office/renderer hermetic回归；故障点和边界 | 不叫原生E2E，不代替GUI或真实持久化 |
| L1 | 同次构建的Aster原生宿主、ACP、Office、真实工作区与VecMemoryStore；本地确定性HTTP模型 | 只替换模型供应方；不拦截文件/管理/发布API，不seed目标业务最终答案 |
| L2 | Aster Test GUI，生产renderer与真实L1链路，实际用户操作及独立oracle | 不用Playwright Electron/HTTP预览代替Tauri，不注入DOM/window.api |
| L3 | 同一隔离实例接入受控真实模型，执行所有标L族的真实模型变体 | 独立报告，脚本模型通过不能抵消真实模型失败；人工语义审阅也需留证 |

**产品矩阵**：Aster与ToYou分别构建和启动，各用独立profile、实例和动态loopback端口；Aster无账户前提，ToYou使用专用合成测试成员/团队。认证夹具通过现有合法测试渠道提供，不复制用户会话或绕过鉴权。`TEAM`用例在ToYou授权范围执行，Aster核对相应入口/能力边界；不是随意标记N/A。

**平台矩阵**：当前先执行工具已认证的macOS原生门；Windows/Linux做各自owner/renderer回归并保留真实GUI适配阻断。没有各自原生证据时不得写跨平台通过。GUI适配器/host版本以本机`capabilities`/`gui-certification`和schema回执为准，不相信过期README中的版本值。

**表现矩阵**：zh-CN/en-US；1440/1024/768/320 **CSS内容宽度**及400%。记录窗口点、物理像素、缩放与内容宽度的换算，不能把1440物理截图当作1440 CSS验收。当前公开的主题/减少运动能力分别核对。关键破坏性流程在两租户正常宽度各跑；全部宽度跑KU和KJ10规定的业务链，而非只截图。

最小必需参数集在I01冻结，不对无关维度盲目做笛卡尔积，也不以pairwise替代以下风险交叉：

| 参数族 | 必需展开 |
| --- | --- |
| 基本功能 | 每个适用E/T场景在Aster与ToYou、zh-CN、正常内容宽度分别执行；TEAM在ToYou授权正例及Aster无新增账户/权限的负例，不能无理由N/A |
| Office | KN08每项实际工具栏/文件菜单动作×可视化/源码/分栏；明确禁用项验证禁用；KN09另验证模式转换、选择和撤销历史 |
| 查询 | 标题/正文/Unicode、空/非空查询、目录/标签/组合、三种排序×升降序、首/中/末页/过期cursor分别出子结果 |
| 记忆 | 四类型×无查询/有命中/无命中；空/单条/137条/10000边界；首/中/末/唯一记录删除及当前/过期版本 |
| 写入风险 | 保存前/在途/已提交未确认×编辑/切页/切范围/退出；外部改动/删除/根替换；普通/真实IME输入（无能力保持阻断） |
| 文案与可访问性 | en-US重跑所有入口、图标/菜单/确认、Office对话框及KJ02/04/06/07；与zh-CN分别保留选中/禁用/错误和完整身份证据 |
| 宽度与缩放 | KU规定用例和KJ10在四个CSS宽度逐一完成，正常缩放/400%另列；页面截图仅是过程证据 |
| 真实模型 | 每个含L族分别跑loopback及真实模型；P0旅程三次独立profile记录，模型与管理/只读零调用断言分段计数 |

精确变体ID、适用性理由和期望owner结果在执行manifest逐条列出；任一未冻结/未执行的必需子结果均阻断“全量通过”。

## 6. 合成夹具与独立预期

所有fixture根为`target/knowledge-e2e/<run-id>/`。工作区在本目录内；profile由native owner按独立instance派生，可处于正常应用存储根，manifest只记录归属。禁止改HOME/CODEX_HOME，禁止复制真实用户笔记、记忆、凭据或session。run-id和精确拥有进程均记录。

| 夹具 | 必须包含的内容 | 预置方式与预期 |
| --- | --- | --- |
| EMPTY | 完全没有wiki、恢复或记忆记录的工作区/profile | 只创建测试工作区根。KG01/KJ01不得准备wiki；记忆先核对真空 |
| BASE | 多级目录、同名文件、中文/emoji、空正文、frontmatter、排序时间/标签和可编辑A/B | 通过现有文件/公共owner准备；保存独立全路径、哈希和预期命中清单 |
| LINKS | 限定/歧义/别名/锚点/百分号、围栏、反链、断链、孤立点、发布引用 | 冻结精确目标与引用跨度；不靠页面自身搜索结果生成期望值 |
| LARGE | 5000篇约50MiB；250/5000位置有未提前读取的唯一正文词 | 生成recipe、seed、逐文件哈希；另建大/超限文件，不污染正常性能语料 |
| MEM | 137条覆盖四类记录；相同ID不同scope；50/10000边界、坏字段及超限变体 | 只用Core公共VecMemoryStore/既有owner样例seed；关闭seed owner后再启动实际GUI；不是新Python SDK |
| REVIEW | 两个以上候选、当前版本、冻结来源、待审/保留/采用/历史状态 | 管理UI用owner产生的合成job；生成业务用例必须经Agent真正创建，不预写最终发布物 |
| FAULTS | 可控制的只读、不可读、锁、迟到/丢失回执、根替换和进程中断 | 仅操作本次拥有资源；精确owner原子边界用现有hermetic seam；不得新增生产后门 |
| SCOPES | 两tenant×两instance×两workspace及旧终端默认目录，同名路径/ID | 每处不同内容和哨兵；校验canonical owner；所有伪造请求也限定合成环境 |
| ASK | 普通问题与共享root/elements/state表单；合法/旧/错绑定和取消 | 复用JSON Render工具夹具，实际ACP工具调用，不用宿主直接提交模拟工具结果 |
| HIER | owner明确的主题、子主题、未分类条目、错误/循环父关系 | T门实施前先确定最小分类契约和版本；不按文件名猜层级，不造前端假树 |
| TEAM | 专用合成成员/团队，loopback目录和有效/无效/中断知识包 | 既有授权/下载/安装API；测试服务不替换安装校验与文件写入逻辑 |
| LIMITS | 搜索/文件/记忆读取、页数、字节/未知字段等边界值 | 值来自当前owner常量和DTO，manifest冻结实际值；不可通过调大预算过关 |

预置用于管理/边界测试是允许的，必须记录`prepared_by`。标记“Agent生成/首次使用”的旅程禁止通过fixture、API或数据库提前制造被验收结果。受控故障只发生在产品owner/API边界，不在renderer拦截成功/失败。

用于纯阅读、查询和性能的BASE/LINKS/LARGE夹具，预置完成后通过既有OKF owner使源代数与发布目录处于current；核对无pending/active job后才开始统计零模型请求。不得临时关闭现有自动整理来压过请求计数。自动整理旅程则使用未整理的源，禁止预产目标百科。针对确认丢失/原子中断的精确故障点，若没有现成受控原生能力，具体分支记阻断并保留H层补充，不能制造生产后门或伪造回执。

## 7. 模型与Agent链路

- 复用[test JSON Render HTTP模型](../../tests/e2e/json_render_model.py)的loopback、脱敏事件和真实ACP工具回执模式。拟新增knowledge测试模型及其小型协议测试，按明确`case_id`、模型请求结构和运行状态分派，不从自然语言猜业务意图。当前JSON Render模型的记忆分支返回空items，不能直接用于“记忆生成成功”。
- 笔记工具名从真实握手/工具定义冻结，不虚构`remember`等工具。记忆按`crates/code/core/src/agent_api/conversation_runtime/memory_retention_tests.rs`及`agent/memory_extraction_runtime/`的真实触发、schema、source/scope、importance/confidence和authority合同走提取。
- KA02/KJ07既跑合法提取，也跑空items、坏JSON、未知source、临时进度、敏感信息等拒绝变体。是否写入由实际authority决定，提示中出现“记住”不能授予存储权限。
- 普通`ask_user`和`ask_user_form`分别验证；后者必须收到`aster.code.user-form.v1`、正确number/boolean等值、原tool_call_id和`permission_grant=false`，之后只恢复原任务一次。
- JSON Render同时验证未知组件/动作、循环/超限JSON、未完成流和旧OpenUI语法的拒绝；只接受当前root/elements/state协议，不通过回退另一套UI机制获得成功。
- 后台知识整理保留012现行策略：普通新内容可自动发布，替换/冲突/重要新结论按实际policy审核；staging不是已发布正文，不改原始资料。阅读/搜索/导航允许检索准备，但不调用模型；业务整理和显式Agent任务的模型请求另列。
- 真实模型只使用测试scope和合成输入。预设每场景最多20次请求、单次120秒、累计900秒及50000 tokens；超过预算记失败/基础设施原因，不盲重试。正式执行前根据实际工具链记录并冻结预算，不能事后为了通过放宽。
- 判定优先使用明确ID、公共解析器结构、源事实、版本、事件数、持久字节与集合。自由文案不做逐字匹配；无法机械核验的语义由人按“关键事实正确、无无源新结论、来源可追踪”的冻结rubric审阅，记录人和证据，不另调一个LLM猜正确性。

## 8. 原生驱动能力门

先绑定测试bundle/PID/window/profile，再`test_session_start → observe → act → observe → finish/abort`；每个动作引用最新observation_id/ref。正文必须观察真实输入并最终核对存储字节；AX赋值或输入回执成功不足以证明编辑成功。

| 能力 | 允许验证方式 | 未具备时 |
| --- | --- | --- |
| 点击、已聚焦窗口文本/按键 | 现有后台语义guard，拒绝坐标/指针重新聚焦/foreground | 记录工具失败，不改用直接OS输入 |
| selected/focused等状态 | schema支持则用；当前布尔断言不支持时，使用fresh语义值、键盘Enter/Space的真实效果与截图 | 不把unsupported记为assert passed；替代证据明确区分 |
| 右键、拖放、指针滚轮 | 必须是已认证且符合后台约束的语义能力；菜单/键盘替代只证明对应业务路径 | 独立交互case阻断，不放开现有guard、不调用前台/坐标工具 |
| 真实IME组合 | 受认证worker能驱动真实预编辑、候选确认和取消，记录输入法与链路 | committed Unicode/DOM composition模拟只算相应补充，KU02仍阻断 |
| 原生尺寸、400%、主题 | 现有产品/宿主允许的真实动作，记录实际内容宽度及结果 | Web预览可补设计检查，不能替原生参数变体通过 |
| 杀进程和恢复 | Aster Test已证明拥有的进程；异常终止场景通过受控owner handle | 不按进程名杀用户实例，不跟随绑定丢失后的替代窗口 |

`test_finish/abort`若报告retryable cleanup_error，仅重试同一终止操作，不再观察或输入。超时/协议/能力问题与产品失败分列。没有cleanup_error为空和零存活拥有进程的结果，整个session不能作为合格通过证据。

## 9. 断言与证据格式

每个场景族和参数变体至少记录：`case_id / variant_id / E或T / tenant / instance / workspace指纹 / model_lane / locale / CSS宽度 / zoom / app与ACP和Office构建SHA / 前置夹具哈希 / 实际步骤 / 结果 / 原因分类 / evidence路径 / cleanup`。

结果使用执行器原生passed/failed/aborted；总报告另列blocked/not_run/not_implemented及原因，不把它们折成passed或从分母移除。必要的case ID、operation、scope和revision皆按已定义协议核对，不猜错误文案含义。一次失败后重跑，各次结果都保留。

| 业务 | 最少独立oracle |
| --- | --- |
| 新建/编辑/保存 | UTF-8/Markdown持久字节、确切变更文件集合、原确认修订；按Office公开规范处理格式归一化；关键frontmatter保留 |
| 改名/移动/删除 | 审核意图、旧/新路径、依赖版本/引用跨度、journal、未涉及文件哈希、外部哨兵 |
| 查询 | 由fixture冻结的预期路径/ID集合、命中位置、owner覆盖/游标、源代数；不拿被测结果当预期 |
| 百科 | staging/job/decision generation、当前发布文件/索引、冻结来源、历史与原文不变；候选不算发布物 |
| 记忆 | 实际公共SDK/新ACP list/read、条件删除收据、后续recall与缓存、保留记录不变 |
| 询问 | 真实tool_call_id/request/revision/owner、类型值、取消与重放拒绝、同任务继续和最终产物 |
| 取消/失败 | 操作前后持久集合/哈希不变或对应最后确认代数；不凭toast/模型叙述定成功 |

保留最少应用窗口截图、AX子树、脱敏原始工具/owner事件、mutation diff、耗时与清理回执。剔除菜单栏历史和桌面其他应用；不录真实token/Cookie/凭据。父仓库证据只含私有Studio源路径和hash，不复制其源码。失败证据不可删除。

## 10. 执行顺序和运行频率

1. **E2E-I01 清单/能力冻结**：核对真实三个页面、Office实际工具栏、所有菜单/快捷键/条件入口和命令；补齐F项遗漏、fixture与oracle schema；运行平台、后台语义/IME/拖放、二进制/profile/端口预检。
2. **E2E-I02 基础红例**：先KG01/KG04及scope探针。KG01实际失败仍记录，然后可在**另一个已预置工作区**继续其他功能的诊断；KJ01保持失败/依赖阻断，不以诊断结果覆盖首次使用故障。
3. **E2E-I03 基础业务**：KG/KN/KQ、KM的无模型管理及主窗口权限，取得真实字节/新ACP回读。
4. **E2E-I04 写入安全**：KR和文件移动/并发/异常终止；完成前禁止启用KR10和替换脏预览。
5. **E2E-I05 Agent业务**：KA、KW真实整理/审核/模型两条lane；随后KJ01–KJ10。
6. **E2E-I06 目标能力**：完成分类、tabs/history、自动保存的各自owner契约与实现，再跑全部T项和KJ11/KJ12。
7. **E2E-I07 表现与负载**：KU/KP的宽度/语言/真实IME/大库/长时参数；不得用warm-owner指标替代UI。
8. **E2E-I08 稳定ACL与全量重跑**：全部必需变体一次完整通过；P0业务旅程在三个独立新profile各连续通过；两租户分别有证据。
9. **E2E-I09 最终资格**：合并权限/locale/构建/类型/fmt/依赖等真实检查与覆盖分母、字节和清理证据；有未完成或失败门时不得宣布全部通过。

PR快速组只用于及时反馈，不叫全量通过：KG01/02/03/05/06、KN01/09/11/13/18、KQ01/03/06、KW02/08/10、KM04/07/08/09/12/15及KS01/04。每日跑全部可执行E和全部已实现T；发布资格跑全目录、全部必需参数、L变体及12旅程。未实现T持续显示在资格报告，不quarantine。没有预估测试时长的基线前不承诺“几分钟全覆盖”；GUI按一个独占桌面lane串行。

每个case独立重置自己的fixture，不能依赖上一case污染的工作区；KJ内部步骤是有意串联。批量失败后每个不同失败先单独复现，再改实现或fixture。未知写入结果先回读对账，禁止自动重放。测试仅因已知写前busy进行owner允许的有界重试，不能通用重试到绿。

## 11. 性能和等待门槛

- 使用015 SC-005：5000篇约50MiB；warm UI搜索P95≤300ms，≤100KiB笔记warm打开P95≤250ms。每类至少30个样本，分别保留输入/选择时间、owner阶段和真正可见/可编辑时间；冷/热单列。
- MCP调用/轮询常耗数秒，不能直接拿observe返回时间测300/250ms。I07须使用实际renderer渲染/Office可编辑就绪时标与同一单调时钟关联，先核对现有可观测事件；必要的最小测量探针不改业务、不代替GUI输入，附时钟/单位小测试并记录探针构建哈希。没有合法、可校准的UI时标则本项保持阻断，不能拿HTTP/owner耗时冒充UI结果。
- 当前历史owner首次ready约21.57秒、warm约117.97ms只作基线，不能改口为UI达标。
- 正常操作等待具体owner/GUI可见状态，不加固定sleep。原生启动20秒内须自动显示基础页面；搜索/读写场景等待上限30秒，真实模型单次/场景按第7节，真实Memory RPC沿现有15秒上限；达到上限保留失败并分类，不增加deadline掩盖产品卡住。
- 冷索引耗时、源更新滞后、Memory响应和RSS曲线原规格未给产品SLA：E2E-I01记录并冻结同机基线、测试采样法和owner限制，计划不伪造已有SLA。任何无法解释的无响应、持续资源增长、重复完整冷建或无界排队都阻断资格；报告中这些指标没有批准阈值时不得写“性能全面达标”。
- 页负载与记录限制沿owner，不为测试调大。200次导航后编辑器/订阅/worker/Agent会话数须收敛；RSS需结合稳定期、样本与进程解释，不能仅截图一次内存作为通过。

## 12. 复用点和拟实施文件

| 层/用途 | 已有文件/模式 | 后续实施位置（不是本轮已生成的运行器） |
| --- | --- | --- |
| 测试引擎 | Aster Test MCP agent与[test JSON Render ACL](../../tests/e2e/json-render-desktop.acl) | `apps/studio/tests/e2e/knowledge/native_*.acl`，先观察稳定路径再逐条提取 |
| 私有桌面夹具/回读 | `apps/studio/tests/e2e/towork/knowledge_profile.py`、实际SDK/新ACP种子和回读模式 | `apps/studio/tests/e2e/knowledge/native_fixture.py`、`native_oracle.py`；仅测试进程编排，不新增Python SDK或生产接口 |
| 防打扰和安全输入 | `apps/studio/tests/e2e/task-catalog/cua_guard.py`及已保留的background语义约束 | 复用guard，先补已获准能力；不直接复用会raise窗口/像素点击的旧ToWork suite |
| 确定性模型 | `tests/e2e/json_render_model.py`与其小型协议测试 | `tests/e2e/knowledge_model.py`、`test_knowledge_model.py`，闭集case_id和最少状态；不模拟被测存储/工具/发布服务 |
| Core与ACP | `crates/code/acp/tests/memory_management.rs`、Core `durable_memory_vec/management_tests.rs`与`memory_retention_tests.rs` | 扩展对应owner现有测试及公共fixture，保持root workspace、publish=false和本地依赖 |
| 知识写入/移动/查询 | Studio native `knowledge_draft_tests.rs`、`knowledge_move*_tests.rs`、`knowledge_query_tests.rs`、`workspace_snapshot_tests.rs` | 故障点在现有owner兄弟测试；不新建数据库、watcher或生产注入后门 |
| 发布和调度 | `crates/okf/src/personal/maintenance/`、`KnowledgeMaintenanceRuntime`及其测试 | 复用既有job/staging/merge/审阅权限；核对actual generation及事件 |
| 编辑器 | `packages/office`真实Markdown editor/toolbar/source命令与兄弟测试 | UI验证可见真实Office；确定性格式/选择边界留在Office owner，宿主不复制编辑器 |
| 旧测试核对 | `apps/studio/tests/e2e/knowledge/knowledge-hub.spec.ts`是旧界面、stub API且有fixme；e2e README仍描述Electron | 在测试实施中更新/降级标注、替换旧选择器和未实现断言；不能直接拿旧suite结果当当前Tauri验收 |
| 证据 | `docs/evidence/desktop/`现有路径/hash/AX/字节diff模式 | 实施当日`<date>-knowledge-e2e/`，按case/variant/session分目录；失败和最终结果并存 |

不另建测试平台、第二仓库/Cargo workspace/锁文件。拟新增的ACL/脚本在观察/实现之前不存在，因此本轮不提供一个貌似可运行的虚假全量命令。

## 13. 现在可执行的预检与底层回归

以下命令来自已有入口；本轮仅规划，未执行它们。先确认私有Studio已初始化，使用仓库固定工具链和本机真正的just程序，记录实际解析路径；不要调用指向其他仓库的包装脚本。

```sh
# 从根目录；读取元数据/能力不启动用户应用
 git submodule status apps/studio
 rustup show active-toolchain
 aster-test capabilities --json
 aster-test gui-certification --json
 aster-test agent schema

# root workspace已有底层/真实进程回归
 cargo test --locked --offline -p aster-core --lib durable_memory_vec -- --test-threads=2
 cargo test --locked --offline -p aster-acp --test memory_management -- --nocapture
 cargo test --locked --offline -p aster-studio --no-default-features --features tenant-aster --lib local_memory -- --test-threads=2
 cargo test --locked --offline -p aster-studio --no-default-features --features tenant-aster --lib knowledge_ -- --test-threads=2

# 本机已核对的真实just二进制；其他worker换成其实际验证并记录的二进制路径
# 两个产品分别构建宿主+匹配ACP+Office，不调用另一个仓库的just包装脚本
 /opt/homebrew/Cellar/just/1.58.0/bin/just --justfile "$PWD/justfile" build-studio aster
 /opt/homebrew/Cellar/just/1.58.0/bin/just --justfile "$PWD/justfile" build-studio toyou
 cargo build --locked --offline -p aster -p aster-acp

# 既有协议表单夹具回归
 python3 -m unittest discover -s tests/e2e -p test_json_render_model.py -v
 aster-test check tests/e2e/json-render-desktop.acl --json
```

Studio内现有前端回归、非原生select与构建门可直接执行：

```sh
node node_modules/vitest/vitest.mjs run --project renderer src/renderer/pages/knowledge src/renderer/pages/documents src/renderer/components/__tests__/FileTreeSearchField.test.tsx src/renderer/components/command/__tests__/CommandProvider.test.tsx src/renderer/services/__tests__/documentChanges.test.ts src/renderer/windows/main/__tests__/MainApp.test.tsx src/renderer/windows/main/__tests__/MainExitGuard.test.tsx src/renderer/windows/main/__tests__/MainExitGuardKnowledge.test.tsx src/renderer/__tests__/noNativeSelect.test.ts
node node_modules/tsx/dist/cli.mjs scripts/i18n-check.ts
```

原生执行由已配置MCP host绑定隔离bundle/profile/window，协议先initialize(2025-06-18)并notifications/initialized，再列tools、schema与start。具体launch及可信host配置随worker manifest记录，禁止通过agent临时改捕获范围。观测稳定后才生成private Studio的ACL，并对每个实际存在的文件执行`aster-test check <suite.acl> --json`和同一host配置下的`aster-test run <suite.acl> --json`。`check`只证明协议合法，不证明业务或GUI通过。

## 14. 任务、依赖与小型测试义务

这些是对现有T001/T004/T006/T009/T013/T015–T020和018 T005的测试切片，不重复宣称产品任务完成，也不标并行。

| 测试任务 | 工作与已有任务关联 | 必须同任务提交的校验 |
| --- | --- | --- |
| E2E-I01 | 功能/参数/能力/性能基线、构建与profile归属冻结；T001/T016/T017 | 清单无未映射动作、schema拒绝错scope、能力缺失如实阻断；不启动用户主实例 |
| E2E-I02 | 新空库红例和正确owner初始化修复；T002/T011 | native缺目录/已有目录/只读/碰撞/symlink的兄弟测试，KG01/KJ01真实重跑；不弱化reserved保护 |
| E2E-I03 | 私有native fixture/oracle、共享模型夹具及G/D业务覆盖；T001/T018、018 T005 | seed/reopen/exact-delete/cleanup错误小测试；模型闭集分派与坏协议小测试；不把脚本当SDK |
| E2E-I04 | 写入/移动/退出/恢复故障和真实中断；T002–T006 | 相同修复附owner测试、state组合测试和指定原生复现；每个不同失败单独复现 |
| E2E-I05 | 发布/审核、Agent写笔记/提取记忆、ask-user/JSON Render；T010/T014/T018 | 真实工具/owner回执、代数/权限/零额外模型调用；L与本地lane分列 |
| E2E-I06 | T门分类/历史/自动保存各自最小owner契约与实现；T004/T009/T013及百科层级子任务 | 每项先有小型owner/组件拒绝测试，再有G旅程；不引入猜测分类、第二存储或SDK |
| E2E-I07 | 背景真实IME/右键/拖放能力、响应式/无障碍/大库测量；T015/T017/T019 | 功能本身和驱动能力分别证明；不放开guard；测量脚本单测或断言校验单位/百分位/时钟 |
| E2E-I08 | 经观察路径提取最小ACL、执行全目录和新profile重复；T018/T020、018 T005 | 每个ACL先check再run，保留首失败和最后通过；coverage/结果合并校验重复ID、缺证据和错误分母 |
| E2E-I09 | 最终格式/类型/依赖/权限/构建/证据与差异检查；T020 | root与submodule分开核对；无stage/commit；真实检查失败单列，禁止减少门槛或扩allowlist |

任务顺序：I01 → I02；I03可准备独立夹具但不覆盖I02故障；I04/I05依赖owner基础；I06依赖安全门和产品契约；I07/I08/I09完成完整资格。当前用户请求只完成本计划及静态一致性核对，不执行这些产品/驱动实施任务。

## 15. Constitution Check

| 原则 | 规划状态 | 落点 |
| --- | --- | --- |
| I Fail closed | PASS | 400、未知、错scope、工具无能力、故障/过期明确失败，未实现不充通过 |
| II Evidence retained | PASS | case/variant/真实字节/回执/清理保留，计划与执行证据分开 |
| III Product hermetic tests | PASS | 每项修复伴随owning小测试，G/D/H分列，批量不同失败单独复现 |
| IV Closed sets | PASS | 真实operation/枚举/版本与明确case_id，不猜名称或语义 |
| V root workspace/public API | PASS | Core/ACP/OKF公共API、Studio私有子模块、无额外workspace或SDK |
| VI Schema owner | PASS | 层级等T门先在真实owner定义契约，计划不引入影子schema/数据库 |
| VII Shrink-only | PASS | 不扩权限/legacy allowlist或quarantine来压过必需失败 |
| VIII Transports | PASS | 模型lane/测试host在测试层选择，不泄露到业务共享envelope |
| IX ACL | PASS | 产品/可信launch配置仍ACL；未来确定性suite使用aster-acl；无新产品配置格式 |
| X Commands/windows | PASS | 主窗口权限负例和受限调用者全覆盖，不加生产测试后门 |
| XIII One repository | PASS | 所有源码/夹具/工作区均在此仓库或owner合法隔离profile；不创建外部仓库 |
| XIV/XV Names/meaning | PASS | 目录和分类依显式owner身份；业务断言用冻结事实/公共parser，模型质量不靠词表猜 |
| XVI Pickers | PASS | House picker实际键盘/选择/渲染，禁止native select，ratchet不扩张 |
| XVII Dialogs | PASS | 现有确认/插入/恢复对话框，焦点/减少运动/关闭语义实际验证 |

这里的PASS表示计划遵守原则，不代表实施、测试或发布已通过。

## 16. 最终验收门

- [ ] F01–F47及实际可见控件/命令全部有有效场景；146个族的每个必需参数均有结果和独立oracle。
- [ ] E与T全部通过；无必需skip/fixme/quarantine、未执行、未实现、阻断或未知写结果。
- [ ] 12业务旅程独立通过，P0旅程三个新profile重复通过；两产品scope和真实模型变体分别有证据。
- [ ] 真空库成功，无额外按键唤醒启动；完整写入/退出/恢复门通过后才验自动保存和脏预览。
- [ ] 真实文件/发布/记忆/询问回执、版本和字节一致，取消/失败不产生意外写入。
- [ ] IME/右键/拖放、宽度/400%/语言/键盘及工具能力分别验收，无Web/组件代替原生。
- [ ] SC-005 UI性能达标，冷启动/索引/更新/RSS数据如实报告；未批准SLA不虚报全面达标。
- [ ] 每个session cleanup_error为空、无存活拥有进程、用户主实例/前台/指针未被测试操作影响。
- [ ] 相关Core/OKF/ACP/native/Office/renderer、权限、locale、no-native-select、本地依赖及两产品build通过。
- [ ] 实际运行Rust fmt与相关类型检查，基线/无关并发失败逐项列出；未通过不得宣称全仓绿色或完整发布资格。
- [ ] `OUTCOME.md`有结果分母/失败分类/命令/版本/哈希/证据/Explicitly not claimed；父仓库未复制私有Studio源、凭据或用户状态；未请求的stage/commit未执行。
