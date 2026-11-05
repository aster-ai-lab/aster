# Data model and ownership

这是实现约束，不要求逐项创建新结构/表。先复用现有类型，只有现有表达不足时增加最小类型。封闭类型有 parse/as_str；外部输入只在边界验证。

| 概念 | 必需身份/状态 | Owner 与现有来源 | 不变量 |
| --- | --- | --- | --- |
| Client key | stable id、label、verifier、generation、enabled/revoked、model grants、quota refs | Preferences/本地网关策略；Gateway inference credentials/grants | 原 key 轮换不改变历史 id；secret 不出现在 Debug/导出/用量 |
| Provider key | 现有 key id、label、enabled、credential value/ref、proxy ref | ProviderStore/providers_keys.rs | 不建立第二 key store；所有候选保留各自真实 id |
| Account | stable account id、provider id、授权来源、状态、凭据引用、模型/窗口 | ProviderStore + Core opt-in imports/现有登录实现 | imported/OAuth/CLI/plugin 等来源显式；不读取未授权来源 |
| Account state | ready、refreshing、quota-limited、disabled、auth-required 等参考确认状态 | owning provider/account module | 状态由 typed response/code 决定，不匹配错误文案 |
| Model catalog | provider/model id、upstream id、协议/能力、effort、context、price revision、enabled | ProviderStore/已有 provider registry | upstream 明确 ID 不猜；价格未知不写 0；alias 不改变 canonical 身份 |
| Route/group | stable id、typed strategy、model/group 引用、规则、classifier、fallback、revision | LocalModelPolicy + Preferences | 空组/循环/越界拒绝；保留旧 ModelReference 的兼容读取和公开 API |
| Route decision | request/turn/session、selected target、rule、candidate attempts、原因 | 本地 ModelControl 执行，accounting 保存事实；独立 Gateway 保留其产品职责 | 本地一次请求只进入一条业务执行链；分类器只选显式候选；实际 target 决定 key/account 归因 |
| Quota policy | scope(client/account/global)、period、timezone、token/cost limit、currency/price version | model_control/本地策略 | 日周月边界明确；硬预算需可确定消费上界 |
| Reservation | stable request/attempt id、budget/window id、reserved/settled/unknown/released、amount、revision | model_accounting 持久化 | 同一 attempt 的同一预算窗口只记一次；真实重试保留必要预留和消费；崩溃恢复不猜测释放未知消费 |
| Request usage | request id、client id、session/turn、protocol、logical model、final status、time | model_accounting | 一次客户端请求一条 final；保留未知/估算计量来源 |
| Attempt usage | attempt id、request id、actual target/account/key id、status、usage/cache/cost/latency | 同一 accounting owner | 失败尝试也记录实际消费；不能将首选身份写给备用回答 |
| Provider window | account/key id、window kind、used/remaining/reset time、observed_at、source | provider quota adapter + accounting | 观测过期与不可用可区分；warm-up 显式 opt-in |
| Agent binding | exact agent id、installation/WSL instance、paths、managed fields、before/after revision | 既有 cliConfig 和 native file APIs | 不猜路径、不跨实例；只回滚已管理字段 |
| Profile/Library | profile id、binding snapshots/model visibility；instructions/MCP/skill/subagent 内容 refs | Aster 配置/内容 owner | Profile 不复制不属于自己的用户状态；目标原生文件不加入 ACL 配置发现 |
| Mask mapping | typed placeholder→original、scope/lifetime、bounded memory、policy revision | Gateway wire/Sentry 内存 | 不落盘；缓存/签名稳定；无法恢复时不得发送破损工具内容 |
| Plugin binding | package id/version/source digest、runtime id、hook/capability grants、status | 既有 use lifecycle/runtime | 新协议 adapter 不改变主机权限；mandatory 与 optional hooks 分开 |
| Tool invocation | invocation/request/client id、tool kind、provider/account/key id、status、artifact refs | 现有 tool/MCP + accounting | media/task 取消与已发生费用如实记录；外部/内置走同一策略 |
| Shared config | tenant/instance、schema/revision、member manifest、content digests、secret handling | 既有 Preferences/备份 owner | 默认不导出 raw secret；错误租户/冲突/损坏拒绝；写入前完整验证 |

## 持久化与迁移

- 产品策略写 ACL，统计/任务状态留 owning store。不要把全部数据塞进一个新增配置文件。
- 当前 ProviderStore 已持有 credential 值，本计划不把“生成的网关 ACL 不含 secret”错误扩大成“所有现有 owner 都不能持有 secret”。新增网关配置引用现有身份；不新增通用 vault。
- 保存 schema 升级先解析旧数据、验证、原子写入；旧 key id 和历史用量保持稳定。失败不删除旧数据。保留现有 tenant/instance 隔离。
- Profile 多文件无法靠单次 rename 保证全部原子；必须使用既有文件写入/恢复模式，记录已写集合并逐项恢复。并发编辑冲突应拒绝覆盖。
- 无头、桌面、CLI/TUI 通过同一 owner API 读写；UI 不持有第二份账户或预算权威。

## UI 与执行的一致性

- 每个持久化对象只在 owning store 写入。UI 草稿是临时状态，不是第二份配置。
- 模型服务、路由、预算、客户端、Profile 与内容各有唯一编辑入口。其他页面保存稳定引用，
  读取派生摘要并深链到原编辑器。客户端 key 与 provider key 不因外观相似而合并。
- 本地 Gateway 的已授权请求交给既有 `DispatchInput` / ModelControl 执行。client/source、
  request/session 身份从准入边界传入，不从可伪造 header 恢复，也不冒充 built-in 来源。
- 内置/外部调用共享同一本地预算与账本。Gateway 传输事件可关联业务 request/attempt，
  不能重复生成产品消费或扣减相同 reservation。
- 能力快照属于 native owner 的只读投影。Code、模型选择器、目录与网关说明使用同一
  revision 的可用状态；不能分别写“支持三个协议”的静态列表。
- 原生文件、Profile、Library 和导入内容仍由原 owner 管理。导入协调器调用这些 API，
  不新增与既有 ProviderStore、配置写入器或备份内容相竞争的存储。

## 请求顺序

鉴权/模型授权 → 有界解析 → 预算预留 → 显式路由/账户选择 → 必需隐私与受控插件处理 → 上游尝试 → 首语义响应承诺 → 流式恢复/取消 → 计量结算。

可选 hook 改模型后必须重新验证目标授权与预算条件；不能以最初授权的 model 调用后来未授权的 model。必需 privacy 是出站最后约束之一，任何插件新增内容都不能绕过它。响应承诺后的失败只终止当前响应，不向备用候选重放。
