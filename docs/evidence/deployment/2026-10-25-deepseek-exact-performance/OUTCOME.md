# 单卡 4090D、1M 上下文的精确推理优化

已完成多组真实引擎对照。24 个 CPU 线程、144 GiB 专家缓存、PCIe 0.00、FP4 KV 的短提示词、128-token 生成中位数为 **26.25 token/s**，对照新 BF16 基线 **5.4 token/s**，约 **4.86 倍**。完整 **1,000,123-token 冷输入检索已通过**，该长输入下实际生成速度为 **18.8 token/s**。配置上下文保持 1,048,576。

## 已完成的测量

| 配置：精确路由、固定 1M 上下文 | 小型正确性 | 生成中位数 | 约 16.8K 输入预填充中位数 | 实际 SSE 首 token 中位数 |
| --- | --- | --- | --- | --- |
| BF16 KV，8 线程，60 GiB 专家缓存，PCIe 0.75，历史基准 | 5/5 | 5.3 token/s | 608.7 token/s | 未测 |
| FP4 KV，8 线程，60 GiB 专家缓存，PCIe 0.75 | 5/5 | 5.9 token/s | 429.0 token/s | 40.244 秒 |
| FP4 KV，8 线程，144 GiB 专家缓存，PCIe 0.75 | 5/5 | 18.65 token/s | 646.8 token/s | 26.856 秒 |
| FP4 KV，24 线程，144 GiB 专家缓存，PCIe 0.50 | 5/5 | 21.6 token/s | 647.1 token/s | 26.902 秒 |
| FP4 KV，24 线程，144 GiB 专家缓存，PCIe 0.25 | 5/5 | 24.4 token/s | 661.7 token/s | 26.243 秒 |
| FP4 KV，24 线程，144 GiB 专家缓存，PCIe 0.00 | 5/5 | 26.25 token/s | 656.8 token/s | 26.423 秒 |
| BF16 KV，8 线程，60 GiB 专家缓存，PCIe 0.75，本轮 SSE 复测 | 5/5 | 5.4 token/s | 658.8 token/s | 26.271 秒 |

上述生成对照使用同一提示词，输出预算 128 tokens，优化组均取 6 次非预热测量，新 BF16 复测取 3 次非预热测量；预填充各取 3 次完整请求，唯一前缀，缓存命中为零。生成 token 包含模型思考。原始响应、精确上游用量和引擎时间均保留；没有用网关经过隐私取整的用量计算速度。

两组 8 线程 FP4 的预填充与首 token 采用补充 SSE 测量，证据分别在 `fp4-60g-8t-sse-ttft/`、`fp4-144g-8t-sse-ttft/`；没有重跑其已通过的质量和生成对照。原非流式预填充中位数分别为 491.5、620.0 token/s，原始记录仍保留。不同批次的加载状态及透明大页差异也保留，不能将差异全部归因于单个参数。

24 线程／PCIe 0.50 组的实际 SSE 首 token 延迟中位数为 26.902 秒；PCIe 0.25 组为 26.243 秒，均对应上述约 16.8K、无前缀命中的输入。计时从本机提交 HTTP 请求到第一个非空生成内容或思考 delta，不把响应头当作首 token。新 BF16 复测的同口径首 token 中位数为 26.271 秒，PCIe 0.00 组为 26.423 秒；两组预填充和首 token 差异很小，主要改善是生成吞吐。PCIe 0.00 只表示未驻留专家的此项 DMA 分配为零，注意力等主体仍在 GPU6 上执行。

144 GiB 预算实际固定了 142.4 GiB、15360 个专家，文件层专家为零。启动日志记录了各批次的透明大页状态；其状态受系统可用页影响，因此不能把复合配置的全部差异归因于单个参数。PCIe 0.25 组曾因透明大页分配使 arena 加载耗时 268.7 秒，这属于加载时间，未计入生成吞吐。

## 正确性和部署边界

旧默认专家路由偏置在算术题上重复思考直至耗尽输出预算，其约 31–34 token/s 结果不计作同等质量的优化。Power 与桥接器现在共同约束三个路由偏置为零。

原请求级 PCIe 扫描无效：固定版本引擎只在启动时读取 PCIe 分配。历史 BF16 基准文件已将相关请求的实际 PCIe 更正为 0.75。上述后续 PCIe 对照均通过真实 ACL 启动配置切换，并核对了运行 argv。

小型正确性覆盖算术、Python 切片、Rust 溢出、Unicode JSON、SQL NULL 语义。FP4 数值内核检查在物理 GPU6 上通过：4,194,304 个压缩 KV 值、1,048,576 个 indexer 值与 fake-quant 逐位一致，含 BF16 回环。此证明不等于所有任务与 BF16 模型输出等价。

运行 Power 二进制 SHA-256 为 `458ef8f70211d0809840b59c89bda6242452f4bc505a69aae1cb3962e64066bb`，源指纹 `c6be3cc41e94cab5ace30210d8a8e123a69c0ee80d67eee0cc7120c93c2cd0e0`；桥接器 SHA-256 为 `426fbe8b80a8d36e0c1df88dd3afb9eabda282a70eb3f06c9fa28ea506ac7528`。170 项 Rust 范围测试、18 项桥接测试通过；修改的精确路由 Rust 文件按实际 Rust 2021 edition 格式检查通过。完整代码补丁和源证明保留，未提交代码。

24 线程／PCIe 0.50 组资源样本保存在 `resource-observations.jsonl`，未见 cgroup 上限或 OOM 事件。后续复测增加了每 5 秒的独立资源观察器。

144 GiB 首次切换发生过一次配置文件属主未保留导致的启动失败，已恢复属主并重启；切换脚本已修复原子复制和备份的属主保留。后续切换使用修复后的脚本。

统一网关已完成 WorkBuddy 排队注解修复及公网复验，记录见同级 `2026-10-25-workbuddy-queue-metadata/OUTCOME.md`：DeepSeek、Prism 共 22 个完整请求通过。最终选定性能配置另完成 10 个公网请求：auto JSON、named SSE、required JSON、auto SSE 各一轮工具调用与结果续答，再加 JSON/SSE 两个带工具的 Hi；全部 HTTP 200，随机工具结果被正确消费。请求包含 27 条左右的历史消息与排队注解。证据在 `final-workbuddy/`，复验脚本为 `test-final-workbuddy.ps1`。

最终恢复已验证：Power PID 2791588、引擎 PID 2796245，ACL 摘要 `622336d46f8c20b3829671e97b1e4d2ccc8626465b73b04db6eeafc7ad556942`，与完成 1M 测试时相同；二进制与引擎 argv 相同。DeepSeek 只占物理 GPU6；其他模型服务 PID 全部保持。正式平台模型 published、上下文 1,048,576，supplier enabled/routable，受管网关快照 revision 47 就绪；公网 `/v1/models` 返回原有 8 个模型标识。最终 Power loaded=1、admission active=0/waiting=0、原生后端空闲；语音健康接口均 HTTP 200。见 `final-selected-profile-restoration.json`、`final-runtime-health.json`、`official-model-and-supplier-state-final.json`、`final-public-model-catalog.json`。

## 1M 工作负载

`fixture/prompt.txt.gz` 保存合成重复档案，原始 UTF-8 输入 5,357,485 字节，SHA-256 `539aadd60c25d9abfbfc4defab2a3c658b9abc9e78cd45355b3b0930bdd12489`。三个校验值各出现一次，分别位于字符位置约 5%、50%、95%；问题不包含答案。真实引擎实际处理 1,000,123 个输入 tokens，缓存命中为零，生成 98 tokens，正常 stop，三个校验值全部精确返回。此为合成重复档案检索测试，不代表所有真实代码工作负载的性能或质量。

冷输入原始证据保存在 `native-one-million-final/`。预填充 4015.662 秒（66.93 分钟）、249.1 token/s；提交请求至第一个非空思考或内容 delta 为 4018.892 秒，至第一个内容 delta 为 4022.378 秒；生成 18.8 token/s，总耗时 4024.211 秒。资源 397 次采样，服务 cgroup 内存峰值 147.343 GiB、GPU6 显存峰值 23100 MiB；cgroup OOM、内存上限事件和 CPU 节流均无增加。

公网 `http://117.162.0.179:8080/v1` 同一份输入亦通过，HTTP 200、完整 SSE 与 DONE、正常 stop、三个校验值精确返回，总耗时 10.873 秒。此请求命中了完整前缀缓存：上游精确记录 cached=1,000,123、uncached=0、生成 101 tokens、19.9 token/s；请求计数仅增加一次，运行 PID 未变。公网用量经过隐私取整，不能直接据其计算速度。请求还携带 WorkBuddy 消息排队及 agent 注解，经过统一网关兼容处理。证据见 `public-one-million-retrieval/` 与 `public-one-million-native-accounting.json`。

长测试脚本核对单卡 GPU6、运行 PID、二进制与 argv；保留实际 SSE、首 token、精确用量、引擎时间和资源记录。前台请求到来时，它只关闭自己的请求连接，让出服务。准备脚本及先前被中断的部分预填充不计作完整 1M 验收。

## 命令和证据

- Rust 1.97.1，现有 Cloud 工作区，锁定依赖、离线执行 shared OpenAI、Prism、Xolotl、OpenAI API 与启动/配置/健康范围测试并构建 server-only Power；完整命令和测试数量在 build receipt、`power-exact-kv-*.log` 中。
- 使用 `stage_native_exact_resource_profile.rs` 经 a3s-acl 解析、生成 ACL，并由 Power `serve --config ... --check-config` 验证。
- 使用 `deploy-native-exact-resource-profile.py` 等待空闲、核对摘要、保留属主、原子切换，仅重载 GPU6；运行检查失败时恢复上一份精确路由配置。
- 使用 `benchmark-native-kv-profile.py` 保存质量、生成和预填充请求；`observe-owned-native-benchmark.py` 记录资源；`benchmark-one-million-foreground-aware.py` 执行长输入验证。

## Explicitly not claimed

未宣称实际 1M 请求相对 BF16 的速度改善比例，未把约 16.8K 或短提示词的改善比例外推为 1M 的实测改善。未宣称全任务质量无损或达到硬件极限。XQ 是混合三值、2-bit、3-bit 等量化的既有版本，没有换权重。没有启用或宣称 MTP/DSpark 提速，也没有覆盖本地另一版本的 Power 源码。完整工作区格式检查的其他既有差异未在本轮顺带改动。

受控 1M 取消验证已通过：实际引擎接受 1,000,123 输入 tokens，前台请求到来后压测只关闭自身连接并以让出状态结束；公网 Hi 在 3.688 秒内 HTTP 200、正常 stop、返回 Hi。正式冷输入测试 PID 2515395 已退出 0，完整完成。两组 FP4 对照的实际 SSE 首 token 补测、选定配置恢复、最终工具调用和正式平台健康复验全部完成，已无本轮压测任务运行。

运行 `python3 audit-final-evidence.py` 可核对六组实际测量、冷输入与公网缓存重放、最终配置、工具调用和正式发布状态；审计输出为 `completion-audit.json`。凭据扫描只记录命中数量与扫描范围，不保存凭据。
