# WorkBuddy 排队元数据统一兼容修复

WorkBuddy 历史消息中的 `queuePosition`、`queueTotal` 未被公共消息规范化入口去除，导致 DeepSeek 和 Prism 的严格消息解析都在 `messages[25]` 返回 400。现已将这两个已确认的客户端字段加入统一网关的消息注解契约，原生模型分发和普通代理共用同一处理。

只移除消息对象顶层的这两个注解；工具参数、工具结果、内容、推理内容及请求级 metadata 中的同名数据保持原值。未知后端控制字段仍交给后端校验。

## 验证

公网地址：`http://117.162.0.179:8080/v1`。

- 修复前，同一份 27 条消息历史在两个模型上均准确复现 `unsupported message field(s): queuePosition, queueTotal`，HTTP 400。
- 修复后，相同请求均返回 HTTP 200、正常 stop 和 `OK`。
- 正式工具复测：2 个模型 × 4 组完整工具往返，加每模型 2 组带工具定义的问候，共 20 个请求全部成功。连同上面的 2 个复现请求，共 22 个完整请求通过。
- 覆盖自动、指定、required 工具选择，流式与非流式；工具结果使用调用结束后新生成的随机值，核对最终答案。第 26 条工具消息同样带两个排队字段。
- 28 项 OpenAI 请求测试、13 项前缀亲和测试、5 项计量测试，共 46 项通过。新增测试检查第 26 条消息、双分发路径、双响应模式，以及工具参数内同名键的完整保留。
- 修改文件按 Gateway 的 Rust 2021 edition 格式化并通过检查。`cargo fmt --all -- --check` 仍有其他既有文件差异；最终日志中没有本次修改文件的差异。

构建命令：Rust 1.97.1，仓库根目录 `cargo test --locked -p a3s-gateway --lib openai_request -- --test-threads=1`；分别运行 `inference::prefix_affinity::tests` 和 `inference::usage_report::tests`；`cargo build --locked --release -p a3s-gateway --bin a3s-gateway`。

已部署二进制 SHA-256：`17d6d7c86ce2680503d56f6d1c1d33177e89bdc851d302ddfd16ea4d7036a674`。最终源码指纹为 `4a0dcf76f4337c5e939936d0a5e3e0c6784a0bca18595891ea7c2de6e307b33a`；修正测试文件的 import 排序后重新测试、构建，二进制与上线版本逐字节相同。运行 PID 2011098。正式托管快照恢复 ready，现有 8 个模型目录保留，其余模型服务 PID 未因这次网关上线而改变。

## 证据说明

`before`、`after` 保存相同请求的修复前后响应。`tool-roundtrip-qualified` 是正式通过记录，包含全部请求与响应、SSE 和随机工具结果核对。

首次工具测试的构造脚本误将校准历史插在系统消息之前，触发 Prism 模板要求系统消息置首的 500。该批次及原脚本保留在 `tool-roundtrip`、`test-queue-tool-roundtrip-system-order-before.ps1`，不计入上述通过数量。修正测试构造后重新完成全部正式请求；没有为此改变生产消息顺序规则。

## Explicitly not claimed

没有宣称任意未知客户端字段都会被忽略，没有更改实际请求队列、工具参数或模型权重。没有宣称完整工作区格式检查全绿，也没有把这次兼容验收算作 1M 输入或推理提速验收。代码未提交。

