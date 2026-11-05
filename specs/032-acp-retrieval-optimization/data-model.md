# Data model：沿用权威模型，新增有界观测

## 1. 生产数据

不在本期引入统一数据库、顶层 search registry 或通用 transport envelope。

| 数据 | 权威 owner | 本期约束 |
| --- | --- | --- |
| 工作区来源、chunk、revision | Core manifest／catalog | 词法／向量都只是相同来源的投影；返回前沿用原校验。 |
| Wiki publication、资料 source／SAG generation | Studio + OKF／SAG | 宿主保持目录、入库、publication 与 source fence；ACP 传递原归因。 |
| 记忆记录与生命周期 | 当前 memory owner | ID、revision、type、activation、retention、delete 保持；semantic 不产生第二份权威记录。 |
| 会话／Run／权限／取消 | 现有 Core 与 ACP owner | 模型对象可共享，session index 和 scope 不因共享而混用。 |

如 T021 选择新的派生 memory vector 投影，其身份必须绑定 owner namespace、原记录 ID／revision、provider／model／asset revision、dimension、normalization 和投影算法版本。先采用已有 typed identity；缺项只在该边界补齐。状态沿原有 Disabled／Building／Ready／Degraded／Closed 能力表达，不能以部分向量假称完整 Ready。

回填／重建有界，查询不等待无界全库工作；old publication 是否可服务由其 source version 决定，不能因更新慢继续引用旧记忆为当前。取消后可保留已验证的派生数据，但旧 worker 不得覆盖新代。

本期默认不增加持久向量缓存 schema。若确有 owning-store schema 变更，在 T023 写清版本、失败恢复和旧读取兼容；不能手改 WAL／生成文件。

## 2. 测试观测模型

这些是 `docs/evidence` 的测试数据，配置仍为 ACL。

- **AttemptIdentity**：source／submodule／binary／fixture／model digest、features、OS／arch、toolchain、profile、seed。
- **CaseResult**：Q-ID、capability source、PASS／FAIL／NOT_RUN／BLOCKED、实际请求／结果归因、失败来源及证据路径。
- **PhaseSample**：原有 session／run／request ID，进程内 monotonic elapsed、stage、duration、queue／compute 区分；不保存正文和凭证。
- **ResourceSample**：provider calls／inputs／bytes、file reads／bytes、records、accounted bytes、RSS、owner／worker／queue 计数；单位明确。
- **QualitySample**：query／mode、真实 ranking、gold fixture identity、指标与当前引用结果；控制 provider 另列。

宿主与 ACP 的现有 DTO 可以不同。测试 adapter 可以归一化观测用于报告，不能反过来要求生产代码迁移到新的 envelope。原始结果必须保留，汇总可以复算。
