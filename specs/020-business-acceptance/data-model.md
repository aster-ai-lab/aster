# Data model: 原 Goal 上的验收关联与视图

## Ownership

Core 是 Goal、Run、检查证据、完成和恢复的唯一权威。ACP 传输；Studio 仅保留安全草稿与显示元数据。现有事实日志仍是唯一持久化来源，不增加数据库或平行事件账本。

下列名称是规划的类型化概念。实现优先复用既有类型；只有确实缺失的关联增加到 owning module，并提供明确的 schema 版本。保留既有 Rust API，避免直接给公开结构追加必填字段。

## CriterionRef

- 引用已有 `goal_id + goal_revision + criterion_index`；正文来自 Goal 的 `success_criteria`。
- 索引仅在该修订内有效；编辑、删除、重排均通过 Goal 修订使原关联过期。
- 不从规则文本推断身份、业务类型或授权。

## AcceptanceBinding

- 引用当前 session/workspace、Goal/Plan 修订、条件、用户确认 proposal/sequence。
- 引用已有宿主 VerificationContract 的精确身份与已声明命令；不提供覆盖范围或权限覆盖字段。
- 保存用户阅读过的检查/夹具路径与内容身份。存在缺失、不明依赖或不可读输入时，不授予当前通过。
- 关联和确认作为同一 Goal 事实链上的版本化关系记录；复用原 writer fence、operation identity、幂等和恢复校验。
- 首期选择已有检查。若模型更改检查，必须产生新 proposal、展示 diff 并重新确认。

## AcceptanceSnapshot

- Core 由已确认关联、原始工具回执和 VerificationReport 生成，不接受调用方写入 verdict。
- 每项包含条件引用、检查引用、执行/尝试身份、修改摘要、检查输入身份、证据引用、状态和原因。
- 业务层状态为闭集投影：`unverified`、`running`、`passed`、`failed`、`unavailable`、`stale`。不增加独立持久化进度状态；底层 VerificationStatus 保持原合同。
- `failed` 表示绑定检查实际失败；`unavailable` 表示拒绝、取消、超时、回执不完整等，不能据此认定业务逻辑错误。
- 豁免是独立来源/决定，永远不映射为 `passed`。历史尝试不覆盖当前结果。
- 当前性要求条件、确认、合同、检查/夹具和修改身份全部匹配；执行环境只陈述实际回执能证明的边界，不声称生产等价。

## HistoricalCriteriaRef

- 显式指向同一合法项目下的历史 session/Goal/修订；来源使用已存身份，不按文本相似性匹配。
- 经原宿主权限与来源验证后，把选择的条件作为新 Goal 的 proposal/修订；保留来源引用。
- 新任务重新确认检查关联，从 `unverified` 开始；不复制通过、豁免、权限、待审批或执行租约。

## Invariants

1. Proposal/确认不执行工具、不调用模型、不增加权限；模型拟稿只走原模型/预算路径。
2. 启用业务验收的 Goal：全部必需关联有当前通过证据，才有资格进入原完成判定；原门禁、等待与 Run 结算仍有效。
3. 任何缺失、陈旧、部分通过或篡改只能缩小结论；不能补成成功。
4. 运行期间修订采用既有忙碌拒绝；取消走原 RunControl，拒绝后不私建重试或重放外部副作用。
5. 新模式有版本与负对照；旧日志原样可读，新事实被旧 reader 拒绝时必须清楚报告，不覆写降级。
