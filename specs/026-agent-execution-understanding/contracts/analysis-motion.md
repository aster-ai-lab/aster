# Contract: Arc 微动效与分析页组件

本合同把 Arc UI 的行为参考落实到 `WorkspaceAnalysisPage`。Arc 不是第二套设计系统，不批量安装组件，不使用公开示例中的模拟事件。实现继续使用 `@a3s-lab/ui`、现有 Motion／CSS、xyflow 和真实 `AnalysisRun`。

## 1. 总原则

每个动效必须同时声明：

1. **真实触发器**：用户动作或已观察事件，不能用装饰定时器制造工作。
2. **状态 owner**：原始事件、页面状态机、实际保存 Promise 或本地视图状态。
3. **终态与打断**：第二次输入从当前位置重定向，不从头重播；源变化立即给出可操作终态。
4. **reduced-motion**：移除位移、弹性和自动播放，保留相同内容、控制和即时状态。
5. **停止条件**：隐藏、离屏、遮挡、暂停、会话切换和 settled 后停止全部循环。

业务状态与可访问状态由 owner 更新，绝不等待动画结束。动画不能延迟 Stop、错误恢复、导出取消或键盘操作。

## 2. 时间、物理与渲染

沿用根 `DESIGN.md`，不创建第二套 motion token 或包 API：

| 场景 | 行为 |
| --- | --- |
| Hover／按压直接反馈 | 120ms，只改颜色或内部视觉，hit target 不动 |
| 图标、动作标签、状态交换 | 160ms opacity，固定 baseline 与槽位 |
| 本地视图、镜头和 playhead 重定向 | 240ms，无 overshoot；新输入从当前位置继续 |
| 小反馈 glyph | 260ms，bounce ≤ 0.12；hit target 不缩放 |
| 非 modal inline 内容展开 | 400ms，bounce 0；只在尺寸变化承载信息时使用 |
| SegmentedControl 选中底板 | 420ms，bounce ≤ 0.16，复用 house 组件 |
| 列表首次进入 | 现有 200ms／4px，前 12 行 16ms stagger；筛选和滚动不重播 |

优先 `transform`／`opacity`。不得动画 `filter`、全 CSS 属性、弹窗定位 transform 或桌面侧栏边缘位移。对话框继续遵守根 `DESIGN.md` 的 in-place 合同。

播放与镜头可以写 CSS variable／motion value；不得每帧 React render。隐藏、离屏、暂停和 settled 必须取消 rAF、timer 与监听。

## 3. 表面与 Arc 组件映射

| 表面 | Arc 行为参考 | Aster 实施合同 |
| --- | --- | --- |
| 理解决策／回放执行、步骤轴／时间轴、镜头 | `SegmentedControl` | 使用 house `SegmentedControl` 的实例级中性底板、单 tab stop、Arrow／Home／End 和 controlled value；首绘与 resize 不重播，reduce 立即到位 |
| 页面运行状态 | `agent-status` | 只显示真实 current activity、elapsed 和结果；单一 active signal。暂停／等待冻结，历史会话不伪装 running；状态文本不依赖颜色 |
| 多轨播放与 scrubber | `agent-timeline`、`line-replay` | 一个逻辑游标与一个 CSS／motion playhead；拖动 1:1，跳转无 overshoot，速度切换稳定槽位。手动播放始终可用，reduce 禁止 autoplay、跳转即时 |
| 工具／事件行 | `tool-call` | 行 ID 稳定，状态 glyph 短交换；输入输出只在展开时挂载。仅首屏有界进入，筛选、分页和选择不重播整列 |
| 检查器与原始证据 | `tool-call`、`expandable-text`、`source-cards` | 原位展开、关闭归还焦点；路径、标题和打开动作稳定。deferred 未返回时保留触发器与真实 loading／miss，不做假内容 shimmer |
| 放射资源图 | Arc 的 object continuity、chart／terrain；`sunburst` 仅借鉴可访问隐藏表 | 拖动 1:1，显式缩放，镜头从当前相机重定向；选择节点与当前路径短交换。筛选只淡化，不重排、不自动旋转。资源不是占比，**不采用 Sunburst 组件** |
| Context／Compaction | `context-window` | 只使用实测 contextTokens 和 compaction anchors；真实压缩时一次 ≤360ms 有界 sweep，pre／post 文本始终可见。未知 Token 不填零，reduce 即时变化 |
| 指标 | `animated-counter` | tabular figures 与预留槽位；实测小变化可短交换，权威修正、单位和未知值直接切换。不做老虎机滚动成本、Token 或错误数 |
| Story／任务完成 | `agent-run`、`agent-plan` | 只有对应确认事件才画一次 check；完成后可折叠执行细节，但未解决错误／审批／检查始终保留。无庆祝动画 |
| 导出／复制／确认 | `action-button`、`copy-button`、`confirm-morph` | 保存 Promise 驱动 pending／success／failure；目标与标签槽位稳定，成功只反馈一次，失败常驻，取消不是成功。原文导出未完成确认不得先播放完成态 |
| 加载、空态、无结果 | `skeleton-morph`、`empty-state` | 预留真实布局，短交换；同 query 刷新不重启 skeleton。EmptyState 只用现有 200ms／4px 首次进入，reduce 直接显示 |
| 产物到达 | `source-cards` | 真实产物出现时原位揭示，稳定标题、路径、预览和打开动作；不用飞入或成功庆祝遮盖未解决失败 |

不采用 `task-breakdown`：本页不是可编辑项目管理计划。`agent-plan` 只用于真实存在的计划步骤，不能从普通事件虚构计划。

## 4. 状态 owner 与打断

| 动效 | Owner | 第二次输入／源变化 |
| --- | --- | --- |
| SegmentedControl 底板 | 页面 controlled mode／lens | 从当前底板位置重定向，aria-checked 立即更新 |
| 播放 playhead | `workspaceAnalysisPlayback` 逻辑游标 + 共享 clock | 新拖动、步进或速度变化从当前位置继续；会话切换立即停钟并复位 |
| 时间线状态 glyph | 原始事件状态 | 不排队播放历史 check；新事件只更新对应行 |
| 放射相机 | `WorkspaceAnalysisMap` 本地相机 | 新镜头从当前相机插值；拖动立即接管，禁止自动旋转 |
| 节点淡化与路径 | 当前 filter／lens／selectedEventId | 只更新 opacity／颜色，不重算径向坐标 |
| Context sweep | 真实 compaction event | 新压缩从当前实测状态开始；历史加载不重播每次压缩 |
| 展开高度 | 当前检查器与证据可用性 | 新内容从当前高度展开；关闭后焦点归还触发器 |
| 导出结果 | 实际 save Promise | 重复点击不叠加成功；失败或取消可立即打断视觉 pending |

`prefers-reduced-motion` 在组件挂载期间变化也必须立即生效，不只读取初始值。

## 5. 可访问性与性能

- 播放 scrubber 使用 `role="slider"`、完整 value text 和键盘；只在 play／pause／finish／明确跳转时 polite announce，不按帧或计时 tick announce。
- 图形提供等价事件／资源列表；Canvas／xyflow 不是唯一信息通道。
- 状态永远有文字或图形之外的标签，不依赖颜色。
- 不自动播放、不自动旋转、不循环 shimmer、不按字母播放正文、不滚动数字制造成就感。
- 20,000 事件下，播放只更新游标与一个活动行／playhead；不得整图、整表每帧重排或重渲染。
- 所有 rAF、IntersectionObserver、ResizeObserver、media-query listener 和 timer 都由 owning component 清理。

## 6. 验收矩阵

| 场景 | 必须通过的断言 |
| --- | --- |
| 快速切换模式／镜头 | 选中值立即正确，底板从当前位置重定向，无双底板、无首绘／resize 重播 |
| 拖动后立刻步进／切速度／切任务 | playhead 不跳回起点，逻辑游标一致，切任务停止全部时钟 |
| 播放中隐藏／离屏／暂停／结束 | rAF 与 React 高频更新停止；恢复不重复已完成事件 |
| 径向图筛选后拖动 | 节点数量与坐标不变，当前路径清晰，相机不自动接管用户拖动 |
| 检查器连续展开／关闭 | 内容按需挂载，焦点路径正确，deferred miss 不显示空输出 |
| 实际 compaction | 只有一次有界 sweep，pre／post 文本正确；历史加载不重播 |
| 导出 pending／success／failure／cancel | 由实际 Promise 决定；无提前成功、无重复 toast、无目标跳位 |
| mounted 后切换 reduced-motion | 立即移除 travel／spring／autoplay，最终内容与操作完整 |
| 20,000 事件长会话 | 播放 100 步 0 请求、无每帧 React render、无全量 DOM 行、settled 后无活动循环 |

每项至少有一个 owning-component 行为测试；相机、播放和 reduced-motion 还需在 M5 的隔离原生旅程中验证。
