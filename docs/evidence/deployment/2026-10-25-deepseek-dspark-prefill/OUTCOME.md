# DSpark 与 GPU6 单卡 1M 冷输入优化验收

本轮已部署并通过完整验收。公网入口为 `http://117.162.0.179:8080/v1`，模型名为 `deepseek-v4.1-flash`。平台官方模型保持 published，发布上下文为 1,048,576；请求仍经 A3S OS 统一网关、a3s-power 和 GPU6 原生 Xolotl 引擎。没有提交代码。

## 实测结果

| 项目 | 对照 | 当前配置 | 结果 |
|---|---:|---:|---|
| 短输入生成中位数，6 次非预热、每次 128 token | 26.0 token/s | 34.8 token/s | +33.8% |
| 同一 1,000,123-token 完全冷输入预填充 | 4015.662 秒 / 249.1 token/s | 2865.007 秒 / 349.1 token/s | 耗时 -28.7%，吞吐约 1.40 倍 |
| 完整冷输入三处检索后的生成 | 18.8 token/s | 20.7 token/s | 实际长上下文结果 |
| 约 16.8K 的唯一前缀预填充中位数，3 次 | 661.0 token/s | 625.9 token/s | 短输入读取略慢 |
| Windows 经公网重放同一完整输入 | — | HTTP 200，11.405 秒 | 100% 前缀缓存命中，非冷输入速度 |

完整冷请求实际输入 1,000,123 token、缓存 0，输出 101 token，正常 stop；EARLY、MIDDLE、LATE 三处答案全部正确。原生 SSE 首 token 为 2867.608 秒，首个最终正文为 2871.314 秒，全请求 2872.769 秒。正式结果见 `native-one-million-optimized/result.json` 与原始 `response.sse`。

公网重放携带 WorkBuddy 的 queuePosition、queueTotal、agent、startsNewUserRequest 字段，经过统一网关返回正确三处答案。原生请求计数只增加 1，精确缓存 1,000,123、未缓存 0、实际输出 109 token；公网用量有取整，未用于速度计算。见 `public-one-million-native-accounting.json`。

## 当前已生效配置

- GPU6 UUID `GPU-ab93a347-c678-1d71-0e85-0f520463debe`，单卡 4090D。
- V3 权重和上下文不变，FP4 KV；三个专家路由偏置均为 0。
- DSpark depth=4、confidence=0.6、长输入暂卸阈值=4096；sidecar 完整 SHA-256 已验证，当前进程日志证明实际加载，失败时拒绝就绪。
- 24 个计算线程、144 GiB 专家 arena；单 token PCIe=0、批量 PCIe=0.25、prefill chunk=256、prompt batch=8192、indexer tile=2^26。
- 1 个活动请求、8 个等待名额、120 秒准入期限；MemoryMax=400 GiB、CPUQuota=3200%，保持既有 CPU 绑定。
- 实际草稿 GPU 权重占用约 5.14 GB，主模型 GPU 驻留专家 949。短生成 480 个送检草稿接受 408，冷 1M 生成 83 个送检草稿接受 75；使用真实引擎计数。
- 真实长输入批量约 8137–8139，对照约 5129–5150。

Power SHA-256：`b47d78130348a2c006dc9b3ad1024cb0160f7727bbe9e32caaed85673951e326`。

Power 源指纹：`5275dc9d598d3fb8460bd221884cf3e9754bc446e5d0f35bdbab7729924d95057`。

桥接器 SHA-256：`ca4b0f9319df0875e10970b17215e09298be203dc669a8eb3ebce2fb600a57b6e`。

引擎 SHA-256：`a99fe7fc22e10081f67e24a04bb78a3f4d6ea39011951e4f19f52a5bb85cec20`。上游固定为 `e9d0826d71c59072cbff77d2a31dd51f27ab7a12`，源码原件摘要已独立从固定 GitHub 版本核对。C++ 补丁只将真实草稿计数放入既有 DONE 字段，未修改采样、算子或路由。

## 验证与证据

- 四组对照各通过 5 项小型正确性检查；各有 6 次非预热生成和 3 次无前缀缓存预填充，原始响应与 SSE 均保留。
- 342 项范围 Rust 测试、17 项桥接器测试和修改范围格式检查通过。全工作区 `cargo fmt --all -- --check` 仍因既有、未修改的 gateway/grpc_handler 等文件差异返回 1，完整输出保留，未改动无关代码。
- 从实际编译源逐字提取生产采样函数，36 组各 400,000 次分布检查全部通过；另有 2,400,000 次普通采样对照。源码、许可证、摘要和输出均保留。
- 最终公网 WorkBuddy 6 个场景、10 个 HTTP 请求全部通过：auto JSON、named SSE、required JSON、auto SSE 的真实工具调用与随机工具结果回传，以及携带工具定义的 Hi JSON/SSE。
- 完整冷输入 283 次资源采样：服务内存峰值 148.571 GiB，GPU6 显存峰值 23082 MiB，主机可用内存最低 272.510 GiB；内存事件全部为 0，CPU 配额限流为 0。
- 正式模型、供应商、Power 注册模型、运行二进制/ACL/argv/env/GPU UUID 与测试身份一致。网关 revision 48 已应用且就绪；公网目录保留 DeepSeek、Prism、OCR、决策与 embedding 等 8 个模型 ID。
- 网关、六个 Prism、六个 Prism Power、两个语音服务的 PID 均与本轮开始一致。两个语音健康检查均为 HTTP 200。
- 长测试遇到前台请求时只取消自身连接并退出 75；结构化前台请求 HTTP 200、精确 answer=250。首次抢占测试的严格 Hi 字面断言因模型输出 `Hi.` 失败，原始失败保留；后续最终 Hi JSON/SSE 测试均通过。
- 总体验收为 `completion-audit.json`，资源为 `native-one-million-resource-summary.json`，运行状态为 `final-runtime-health.json`；构建来源、代码补丁和完整测试输出在本目录。

## 适用范围

1M 输入是同一份合成重复档案，三处检索值各出现一次，最终问题没有透露答案。结果不能直接推广到所有真实代码任务。当前配置组合改善了长冷输入和短生成，短输入预填充略慢；未将收益归因于某一个参数，也不声称达到硬件上限或所有任务质量完全等价。未更换 V4 权重，未启用有损专家路由。

源代码变更保留在服务器受控工作树及本目录的补丁/源码快照中；root Power 与 Cloud Power 副本一致。旧对照证据位于同级 `2026-10-25-deepseek-exact-performance/`。本目录的凭据扫描、文件摘要清单和 Windows 同步核验在最终保存时生成。
