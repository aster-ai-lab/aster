# Agent 执行理解与证据回放 — Result

**日期**：2026-10-29

**范围**：`specs/026-agent-execution-understanding`，T001–T041

**结果**：T001–T041 均已完成。T037 按用户后续确认，以无界面 Chromium 的相机拖动、筛选保持和 reduced-motion 结果替代原生鼠标相机验收；**仍不宣称真实原生相机已通过**。原生指针未执行，xyflow 也没有可用的键盘缩放路径。T038、T039 及 T040/T041 的证据与范围检查均已闭环。

## What landed

- 以 `AnalysisRun` 统一原始消息、parts、统计、父子 Agent、deferred 引用和可选 Trace spans；保留稳定身份、源顺序、覆盖和未知测量。
- 本地执行回放：任务／模型／子 Agent 镜头、事件筛选、多轨时间线、1×／4×／16×、键盘和共享选择；播放不读取原文。
- 必做放射资源图：中心任务、七类内圈、真实资源外圈、当前路径、拖动／缩放、可访问资源列表和确定性径向坐标；不是 Sunburst。
- 长资源集采用**当前路径绘制＋全量滚动窗口列表**：20,000 资源不同时交给 xyflow，列表仍保留全部资源身份与选择能力。小资源集继续绘制完整放射图。
- deferred 原文按需读取；永久 miss 显示不可用，不显示空输出假成功。
- 完整轨迹：状态、阶段、动作、原始记录、Story、版本树／执行顺序及无快照降级。
- AgentPlay 脱敏导出与含原始证据的完整轨迹导出分离；后者需要原地确认，保存 Promise 决定反馈。
- 单次 `AnalysisAnnotation`：独立内部目的会话、无工具 `sourceReview`、结构化校验、逐字 quote、sourceRevision 失效和迟到结果隔离。没有聊天、追问或 Smart Context。
- Arc 行为复用 house SegmentedControl、Button、CopyButton、EmptyState、rowMotion 与 Motion／CSS；没有第二设计系统、Arc 批量安装或装饰性 autoplay。

## Final qualification commands and counts

在 `apps/studio` 运行，除非另注：

| 检查 | 实际结果 | 原始日志 |
| --- | --- | --- |
| `node_modules/.bin/vitest run --project renderer --maxWorkers=1 <20 个分析文件 + WorkspaceWikiCatalog.test.tsx>` | **21 文件／125 测试通过**；其中原 `workspaceAnalysisProjection` 现为 9 个测试并保留最初 3 个行为 | `work/agent-execution-understanding/t039-feature-tests.log` |
| T036 功能基线 | 21 文件／99 测试通过 | `work/agent-execution-understanding/t036-feature-tests-final.log` |
| shared `agentSessionAnalysis` 与 runtime capabilities | 2 文件／12 测试通过 | `work/agent-execution-understanding/t036-shared-tests.log` |
| house EmptyState／SegmentedControl／barrel | 3 文件／21 测试通过 | `work/agent-execution-understanding/t036-house-tests.log` |
| save／Trace／history／CopyButton／plan／deferred／AgentRightPane 回归 | 9 文件／186 测试通过 | `work/agent-execution-understanding/t036-integration-tests.log` |
| `node_modules/.bin/rsbuild build` | 最终构建通过，10.6 s；只有既有动态依赖警告 | `work/agent-execution-understanding/t039-renderer-build.log` |
| `node_modules/.bin/oxlint <changed analysis + browser test files>` | 0 warning／0 error | `work/agent-execution-understanding/t039-oxlint.log` |
| `node_modules/.bin/oxfmt --check <same files>` | 通过 | `work/agent-execution-understanding/t039-format.log` |
| `rustfmt --edition 2024 --check apps/studio/src-tauri/src/agent_plane/session_archive_tests.rs` | 本功能夹具通过 | `work/agent-execution-understanding/t039-rustfmt-scoped.log` |
| `node --max-old-space-size=8192 node_modules/typescript/bin/tsc --noEmit -p tsconfig.web.json --composite false` | 全仓既有错误仍存在；完整日志中 `WorkspaceAnalysis`／`workspace-analysis`／`workspaceAnalysis` 诊断为 **0** | `work/agent-execution-understanding/t039-typecheck-web.log` |
| `node_modules/.bin/playwright test --config playwright.knowledge.config.ts tests/e2e/knowledge/workspace-analysis.spec.ts` | **1 个完整浏览器旅程通过**，含中英文、四档视口、相机、键盘、长会话 | `work/agent-execution-understanding/t039-browser.log` |
| `node work/agent-execution-understanding/native-journey.mjs`（仓库根） | 最终非相机原生旅程通过； owned 进程清理 | `work/agent-execution-understanding/t037-final.log` |
| `node work/agent-execution-understanding/native-legacy.mjs` | 无快照降级与内部会话过滤通过 | `work/agent-execution-understanding/t038-legacy.log` |
| `node work/agent-execution-understanding/native-long.mjs` | 20,000 工具事件＋1 用户事件、100 步、13 条可见时间线行通过 | `work/agent-execution-understanding/t039-long.log` |
| `cargo test -p a3s-studio --lib seed_analysis_e2e -- --ignored --nocapture` | 两个隔离数据根播种成功；只编译宿主 lib，未声称其他既有 integration target 通过 | `work/agent-execution-understanding/t038-legacy-seed.log`、`t039-long-seed.log` |

未把全仓 `pnpm lint`、全仓 typecheck 或全仓 `cargo fmt --all -- --check` 写成通过。全仓 Rust format 日志包含其他进行中文件的差异；本功能 Rust 文件单独通过。没有调用真实模型作为 hermetic 通过条件。

## FR evidence

| Requirement | Evidence |
| --- | --- |
| FR-001 统一事实 | Projection／model／Trace 测试；`AnalysisRun` 同时服务执行、Story、状态图和两类导出。 |
| FR-002 稳定身份与未知值 | 稳定 event ID、seq、EvidenceRef；旧消息缺少 runtimeTiming／Token 时显示未知，不补零。 |
| FR-003 覆盖 | 分页 prepend、局部加载和分页失败测试；失败保留局部事件并明确覆盖，不宣称完整。 |
| FR-004 deferred | 原生 `read-deferred` 原文与第二个永久 miss 均通过；只按需读取。见 `t037-deferred-original-ax.txt`、`t037-deferred-miss-ax.txt`。 |
| FR-005 共享选择 | 原生 Story 的 `write_file` 与执行页检查器选中同一事件；本地 reducer 与 stale session 测试通过。 |
| FR-006 镜头 | 任务／模型／子 Agent 镜头不复制事件，镜头指标单独计算。 |
| FR-007 状态证据 | 两状态夹具、Diff／哈希／产物正反例；无快照旧会话显示“没有已验证的状态快照”，无伪造 DAG。 |
| FR-008 注释校验 | 未知 ID、越界、非逐字 quote、超预算、陈旧修订均拒绝；注释与观察事实分离。 |
| FR-009 AgentPlay 脱敏 | 8 个秘密标记输入确实存在，导出出现 0 次；官方 2020-12 Schema 经 test-only Ajv 通过。 |
| FR-010 完整导出确认 | 原生取消不报告成功；再次确认后才保存。实际文件重开比对通过。 |
| FR-011 放射资源图 | 七类、至少 30 节点、真实选择、筛选坐标稳定；长资源集当前路径绘制，全部资源由窗口列表可达。 |
| FR-012 内部会话 | `agent-session-analysis` 封闭目的过滤；原生 `Internal analysis visibility probe` 未出现在普通任务列表。 |
| FR-013 入口拒绝 | Zod／封闭 union 校验未知枚举、重复 ID、坏引用、非法状态和非法注释。 |
| FR-014 可访问与窄屏 | 真实 Chromium 的 1440／1024／768／320 视口、键盘、焦点、中英文通过；无 native `<select>`。 |
| FR-015 边界 | 只在现有根仓库和 Studio 子模块；无 Python 服务、iframe Viewer、第二 workspace、第二模型客户端或新配置语言。 |
| FR-016 Arc 行为 | 下表 9 类场景均有行为断言；组件和浏览器通过，原生相机例外明确保留。 |

## SC evidence

| Criterion | Result |
| --- | --- |
| SC-001 | 最终 21 文件／125 测试通过；正反例覆盖模型、状态、放射布局、脱敏和注释。 |
| SC-002 | 夹具有 20,000 个动态工具 part，投影另有 1 个用户事件，合计 20,001。原生从第 1 步走 100 次到第 101 步；展开时间线仅 13 行。Reducer／页面测试证明 100 步 0 历史请求、0 模型请求，无每帧 React render。 |
| SC-003 | 8 个秘密零泄漏、引用解析和官方 Schema 测试通过。 |
| SC-004 | 两条原生旅程、键盘、中英文、stale-response 测试通过；相机由用户确认的无界面 Chromium 替代证据闭环，原生相机仍未执行。 |
| SC-005 | 七类资源、至少 30 节点和共享选择通过。原生筛选前后错误节点坐标均为 `646, 304`；浏览器相机拖动后筛选保持不变。 |
| SC-006 | 9 类矩阵有组件行为测试；连续切换、hidden／settled、mounted reduced-motion 与无每帧 React render 通过；相机采用用户确认的浏览器替代证据，不宣称独立原生相机通过。 |

## Radial graph and long-session evidence

- `t037-result.json`：七类资源，错误资源 `bash`，重试 `bash #6`，deferred 命中与 miss；筛选坐标 `646, 304` 不变。
- `t037-analysis.png`、`t037-errors.png`、`t037-retry.png`、`t037-deferred.png`：布局辅助证据，行为由 AX 断言和结果 JSON 决定。
- `t039-long-result.json`：`events=20000`、`nativeSteps=100`、`selectedStep=101`、`timelineRows=13`。
- 原生窗口 1440／1024 可调整到请求值；请求 768／320 时宿主最小窗口宽度钳制为 **960**，这两个尺寸没有冒充原生通过。
- Chromium 真实 CSS 视口 1440／1024／768／320 均通过；`analysis-replay-wide.png`、`analysis-replay-compact.png`、`analysis-replay-768.png`、`analysis-replay-320.png`。
- 长资源图只画真实分类与当前路径；`aria-setsize` 保留全部资源，滚动后 `src/long/file-9999.ts` 可选择并定位真实事件。

## Arc trigger / owner / interruption review

| Matrix scenario | Owner and observed evidence |
| --- | --- |
| 快速模式／镜头切换 | 页面 controlled state 与 house SegmentedControl；连续 5 次切换、单底板、无首绘／resize 重播测试。 |
| 拖动后步进／速度／任务切换 | `workspaceAnalysisPlayback`＋共享 CSS playhead；从当前位置继续，session switch 停钟。 |
| hidden／offscreen／暂停／结束 | owning clock 清理 rAF／listener；hidden 与 settled 后活动循环为 0。 |
| 径向筛选与相机 | `WorkspaceAnalysisMap` 本地相机；浏览器拖动接管、筛选不重置、无自动旋转；原生相机未验。 |
| 检查器连续展开 | 本地 details／证据状态；输出按需挂载，deferred miss 不是空输出。 |
| 实际 compaction | 真实 compaction anchor；一次有界 sweep、pre／post 实测文本、未知 Token 不补零。 |
| 导出 pending／success／failure／cancel | 实际 save Promise；原生取消与成功反馈、组件失败／取消测试。 |
| mounted reduced-motion | media-query listener；浏览器 `page.emulateMedia({ reducedMotion: 'reduce' })` 后 `data-camera-motion=instant`，键盘与内容仍可用。 |
| 20,000 长会话 | 本地游标、窗口化时间线／资源列表、当前路径图；100 步、0 请求测试、无全量 DOM 行。 |

## Export reopened and source comparison

实际原生文件：`analysis-trajectory-01a186a4-eee0-71c8-806b-f09a78a3e2f1.json`。

- `format=coding-agent-state-trajectory`，`version=1`。
- 38 个 timeline 事件、2 个状态、38 条 source records。
- `evidence.status=partial`：永久 deferred miss 明确保留，未伪造“完整证据”。
- 全部 `message_part` sources 均与 `native-messages.json` 中同 message ID／partIndex 的原始 part 深度相等；独立复核见 `work/agent-execution-understanding/t040-export-check.log`。
- 原生先取消、后确认保存；`t038-result.json` 记录 `cancelReportedSuccess=false`、`savedFeedback=true`、`rawSourceMatchesFixture=true`。
- AgentPlay 与完整轨迹的隐私语义没有混用；完整原文导出不出现在脱敏断言中。

## Explicitly not claimed

- **未通过真实原生相机打断／缩放验收**。OS 指针拖动、滚轮和双击按用户要求不执行；两次键盘缩放尝试均未改变坐标，xyflow 不支持该路径。只有 hermetic 与无界面浏览器相机证据。
- 不把 Chromium 的 reduced-motion／相机结果称为独立原生 WebView 结果；原生只单独证明了播放、键盘、筛选、deferred、Story 和导出。
- T037 按用户确认的替代验收关闭，T001–T041 均已勾选；但不宣称真实原生相机或 SC-004／SC-006 的独立原生相机条件完成。替代依据见 `t037-browser-camera-result.json`。
- 不做交互式 AI 问答、聊天输入、追问、Smart Context 或两轮证据选择。
- 不承诺逐 Token／逐流式状态录像，不承诺所有历史会话的完整版本树或外部 Agent 全日志解析。
- 不保证真实模型生成的决策、因果或总结语义正确；固定模型响应只验证协议，不测语义质量。
- 不提供跨重启注释持久化、团队共享、远程公开分享或自动上传。
- 不把公开价格估算当真实账单；未知 Token／成本／时间保持未知。
- 不批量安装 Arc，不使用 Pro 源码、Sunburst、自动旋转、持续 shimmer、逐字正文、装饰粒子或成功庆祝动画。
- 不宣称全仓 lint／typecheck／format、其他平台、其他模型、所有 Rust integration target 或所有真实模型在线旅程通过；真实模型证据仅限本次 `moonshot::kimi-k2.5` 的失败验证案例。
- 不宣称 20,000 个资源同时绘制；大资源集只绘制当前路径，全部资源通过可访问窗口列表保留。

## Follow-up: header action tooltips

红框中的生成语义注释、导出回放、导出完整轨迹和刷新按钮现均复用 house `Tooltip`，在按钮下方显示与可访问名称一致的提示；进行中提示随真实状态切换。`WorkspaceAnalysisPage.test.tsx` 18 个测试通过，覆盖四个 Tooltip、刷新调用及既有注释／导出行为；scoped Oxlint 为 0，格式检查通过。日志为 `work/agent-execution-understanding/tooltip-page-test.log`、`tooltip-oxlint.log`、`tooltip-format.log`。

## Real-model native E2E follow-up

使用真实 `moonshot::kimi-k2.5` 完成原生 Aster 旅程：先选择失败验证 `bash` 事件，点击“生成一次语义注释”，经独立内部分析会话和真实 ACP 返回 `a3s-review-finding`，再映射为 `AnalysisAnnotation` 并通过既有 Schema、目标范围和逐字 quote 校验。注释在“验证 · 失败”中发布，证据回跳定位同一 `message:assistant-1:part:3`／`bash` 事件。

- `real-model-annotation.json`：`sourceRevision=run-76882c5e`、`move=open`，quote 为原文 `pnpm test orders`，`evidenceSelection.reached=true`。
- 原会话消息数生成前后均为 4；内部分析会话只有 1 条用户消息和 1 条真实助手消息；任务列表不显示“内部执行分析”。
- 使用随包真实 review skill 和 picker，发布状态 `skill=true`；Provider 配置、runtime.json 与 ACP 临时凭据在结束后删除，未输出或保留密钥。
- `WorkspaceAnalysisTurn.test.tsx` 与 `WorkspaceAnalysisPage.test.tsx` 共 23 个测试通过，Oxlint 0、格式检查通过，最终 rsbuild 构建通过。日志为 `work/agent-execution-understanding/real-model-final-tests.log`、`real-model-final-oxlint.log`、`real-model-final-format.log`、`real-model-final-build.log`、`real-model-final-e2e.log`。
- 输出合同已统一为 finding 传输格式，页面确定性映射为注释；没有硬编码模型回答、聊天输入、追问或 Smart Context。早期缺失 review skill、错误调用格式、未绑定 quote 和后端错误均保留为失败诊断，没有改写成通过。
- 本结果只证明该模型、该夹具和该失败验证路径；不保证其他输入的语义质量、所有模型返回格式或所有真实模型在线旅程。

## T041 scope check

用户未要求 commit；没有 stage、commit、新仓库或第二 lockfile。大量其他进行中改动保持原样。验收只修改本功能规格、Studio 分析实现／测试、隔离夹具和本证据目录。原生与浏览器 owned 进程均要求清理；预览服务不纳入交付。

## Neutral trajectory naming follow-up

- 界面、源码、测试、规格和验收资料统一使用“完整轨迹”；实现采用 `AnalysisTrajectory`／`workspaceAnalysisTrajectory`，导出文件前缀为 `analysis-trajectory-`。`coding-agent-state-trajectory` v1 格式和原文确认／缺失标记语义保持不变。
- 3 个聚焦测试文件共 27 项通过；4 个源码文件 Oxlint 为 0 warning／0 error，6 个源码与 locale 文件格式检查通过，renderer 构建通过。日志为 `t040-rename-tests.log`、`t040-rename-oxlint.log`、`t040-rename-format.log`、`t040-rename-build.log`。
- 原生取消／确认／保存旅程通过，实际导出 38 个 timeline 事件、2 个状态和 38 条 source records；38 个 `message_part` 全部与源消息同 ID／partIndex 的原文深度相等。日志为 `t040-rename-native.log`、`t040-export-check.log`。
- 全仓旧名称与旧文件名残留均为 0；受影响的 `.studio-renderer`、历史验收构建及其缓存均已重新生成，相关构建日志保存在 `t040-rename-*-build.log`。
- 新原生窗口通过无障碍焦点实际展开导出 Tooltip，可见静态文本为“导出含原始证据的完整轨迹”；见 `t040-export-tooltip.png`、`t040-export-tooltip-ax.txt` 和 `t040-tooltip-native.log`。旧运行窗口必须重新加载前端资源，页面数据刷新不能替代。

## Analysis readability optimization follow-up

- 时间线增加编号任务段、当前任务强调、播放头前后层次、受限宽度 Token 柱、Context 区域和压缩标签；继续复用现有 Canvas 与真实测量，不新增图表依赖。
- 放射图修正内圈标签朝向，使用资源色强化选中路径并淡化非路径节点；筛选选中描边、播放进度／实测总耗时和检查器时间信息同步优化。混合时钟不再用于计算总时长，无法比较的当前进度显示为“—”。
- 语义注释失败显示中英文可读提示，原始错误码仅保留在辅助诊断 `title`，不再作为用户可见的唯一错误内容。
- 6 个聚焦测试文件共 43 项通过，Oxlint 0 warning／0 error，12 个源码／测试／locale 文件格式检查通过，renderer 构建通过。真实 Chromium 完整旅程通过，覆盖播放、筛选、证据联动、长会话、窄屏和 reduced-motion；最终截图为 `t042-analysis-optimized.png`，日志为 `t042-optimization-*.log`。
- 本次原生旅程未执行成功：`target/studio/aster/debug/bundle/macos/Aster.app` 在启动前被其他构建清理，日志 `t042-optimization-native.log` 记录 ENOENT。没有把 Chromium 结果称为原生通过，也没有启动竞争的 Rust 构建。
- 普通放射图连接线由接近背景的 `#ececec` 调整为 `#d4d4d4`，全部非选中边保留，选中路径颜色与线宽不变。真实 Chromium 直接断言 SVG `stroke=rgb(212, 212, 212)`；5 项地图测试和完整浏览器旅程通过，截图为 `t043-gray-edges.png`。
- 时间线保留外部播放头时，也把选中事件传给 Canvas，修复当前任务和未来轨迹状态未生效的问题；未来事件使用 `#c5c5c5`，未来 Token 使用 `#d4d4d4`，补充事件与 Token 浅灰基线。真实 Chromium 对时间轴直接 Canvas 做灰色像素断言，2 个测试文件共 26 项及完整浏览器旅程通过，中途截图为 `t044-gray-timeline.png`。
- Delay／Model switch 以类型化 `AnalysisEvent.signals` 挂在原事件上，不增加重复执行记录。实测耗时达到 10 秒生成 Delay；只有同一消息、相邻工具且时钟可比较时才计算空闲间隔。模型切换按同一 Agent 的 response／llm 模型序列生成，接入筛选、时间线、检查器、`M` 跳转及官方 Schema 导出。7 个测试文件共 64 项通过，浏览器验证 `model-a → model-b` 闭环，截图为 `t045-analysis-signals.png`。

## Scope correction: unrequested extensions removed

- 按用户红框审查，已移除 Story／状态树、镜头切换、语义注释与内部分析会话、完整轨迹及回放导出、资源列表、相机重置和底部事件详情。对应实现、专属测试和失效原生脚本已删除；项目任务选择入口保留。
- 当前页面只保留执行放射图、检查器、播放、筛选、时间线、真实证据和类型化执行信号。筛选恢复 house `secondary` 选中样式，播放进度恢复普通 muted 文本，不再使用自定义边框或底板。
- 7 个核心测试文件共 54 项通过，Oxlint 0 warning／0 error，格式与构建通过；中英文完整 Chromium 旅程明确断言被移除入口不存在，同时验证核心回放、相机、模型切换和长会话。最终截图为 `t046-core-replay.png`，日志为 `t046-scope-cleanup-*.log`。未宣称原生包验收。
- 按后续红框审查，继续删除“回到实时”、全部信号快捷按钮及其键盘动作、右侧事件计数、时间线标题和步骤／时间轴切换。播放只保留播放／暂停、前后步进、速度与普通进度文本；时间线固定为步骤视图。7 个核心测试文件共 53 项通过，Oxlint 0 warning／0 error，格式、最终构建及完整 Chromium 复验通过；错误、重试、组合筛选、模型切换、长会话和中英文均验证。最终截图为 `t047-controls-removed.png`，日志为 `t047-controls-removed-*.log`。当时未宣称原生包验收；补跑结果见下节。

## T048 native package validation follow-up

- 使用独立正式调试包 `target/native-validation-20261030-1145/debug/bundle/macos/Aster.app`、随包 ACP 和最终 `native-renderer`，以官方 development-launch 标记启动；`codesign --verify --deep --strict` 通过。未执行 Apple 公证，也未修改系统信任或录屏权限。
- `t048-native-journey.log` 对应的原生核心旅程通过：七类资源、错误筛选坐标稳定（`735, 279`）、失败与重试详情、End 键末步选择、deferred 原文及永久 miss 均验证；被移除入口仍不存在。结果见 `t037-result.json`。
- `t048-native-long.log` 对应的原生长会话旅程通过：20,000 个工具事件加 1 个用户事件，逐次确认 100 次有效步进并到达第 101／20001 步。窗口请求 1440／1024 时实际分别为 1440／1024；请求 768／320 时均由宿主最小宽度钳制为 960。结果见 `t039-long-result.json`。
- 两条旅程都只使用 AppleScript 无障碍读取、点击和键盘事件，`pointerInput=false`，未移动系统指针。owned 原生进程和 4318 预览端口均已清理。
- 长会话截图被外部“屏幕录制／系统设置”窗口遮挡，只保留为诊断，不作为布局通过证据；原生结论以无障碍断言和结果 JSON 为准，四档 CSS 视口的视觉证据仍由已通过的 Chromium 旅程提供。原生相机指针验收仍未执行。

## T051 timeline reference alignment

- 红框复核发现两轮局部缩放方向错误：短会话 12px 上限把时间轴缩到左侧，128px 高度又压扁了参考结构；`t049-compact-timeline.png` 与 `t050-compact-lanes.png` 只保留错误过程，不作为最终结果或提交证据。
- 已撤销这些缩放，恢复全宽任务段、事件、Token、Context、播放头和拖动区域，以及 160px 四轨高度。新增全宽末事件、全宽拖动区和轨道高度断言；最终截图为 `t051-full-width-timeline.png`。
- 参考图与当前会话的剩余密度差异来自真实事件数和 Token／Context 测量点数量，不通过伪造数据补齐。7 个核心测试文件共 53 项通过，Oxlint、格式、renderer 构建和完整 Chromium 复验通过；日志为 `t051-full-width-timeline-*.log`。本次未重跑原生包。
