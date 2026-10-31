# Research: 原生路径的事实与设计依据

核对日期：2026-10-22。仓库能力是源码观察，GPU 数据是各自时间戳下的实测，不代表后续实时状态。

## 已有事实

| 事实 | 仓库依据 | 对规划的影响 |
|---|---|---|
| 官方链路是 Power → recipe → SGLang | `scripts/h200_official/validate.py`；`docs/evidence/power/2026-10-22-h200-official/OUTCOME.md` | 它是参考路径，不能记为原生执行 |
| 原发布构建仅启用 `server` | 同证据目录 `power-build-provenance.json`、`build-power.log` | 新原生构建需要独立 features、二进制和实际内核证据 |
| 通用原生运行时不拥有模型架构 | `crates/power/docs/embedded-inference-architecture.md` | DeepSeek 图、tokenizer、状态和循环放在模型所有者；不能塞进通用缓存或调度器 |
| 静态图算子集合与 F32 边界有限 | `crates/power/src/inference/graph/plan.rs`、`tensor.rs` | 不是开启 `embedded-cuda` 就拥有 FP8/MXFP4、大 MoE 或完整 V4.1 支持 |
| 已有驻留、异步预取与精确专家 batch 联合 | `inference/residency/`、`weights/`、`routing.rs` | 复用状态与资源所有者，补模型调用与 H200 实证 |
| 已有设备 mesh 与受限 peer transfer 合同 | `inference/accelerator/mesh_execution.rs` | 不等价于已实现高效 NCCL all-reduce、all-to-all 或 EP 调度 |
| `picolm` 是另一种纯 Rust 逐层 GGUF 路线 | `backend/picolm.rs`、`backend/gguf_stream/architecture.rs` | 小内存能力不替代 V4.1 低比特 CUDA 实现 |
| Xolotl 后端是外部执行器适配 | `backend/xolotl/`；`docs/xolotl-h200-xq.md` | XQ 仍需独立原生格式适配；不能将进程外计算算作原生 CUDA |
| Power 子模块现有独立 manifest 边界、部分 Aster 非 path 声明 | `crates/power/Cargo.toml`、根 `Cargo.toml` | T001 必须验证根工作区接入、本地依赖和构建所有者，不能扩张存量例外 |

路径表中的 `inference/` 与 `backend/` 均相对 `crates/power/src/`。

## 参考性能的边界

同一轮 Power + SGLang 测量，三轮、每请求 256 个输出 token：

| 并发 | 总输出 token/s 中位数 | TTFT 中位数 ms |
|---:|---:|---:|
| 1 | 65.1760 | 176.8 |
| 4 | 160.3178 | 437.8 |
| 8 | 235.0647 | 712.7 |
| 16 | 316.4108 | 978.7 |
| 32 | 382.8811 | 1713.4 |

来源：`docs/evidence/power/2026-10-22-h200-official/deepinfer-batching/`。
183 次请求完成，统计含 reasoning token。这是共享后端、不同 offered concurrency 的容量观察，不是同负载的原生/外部执行器 A/B，也不是硬件上限。原生路线必须重建独占基线。

## 借鉴 DeepInfer 的机制

参考固定 revision：`9bd9cf243fc53ef9afbf8f04d391d989117d6ab9`。
来源文件及散列见既有 `deepinfer-reference-manifest.json`。

| 思路 | 在本方案中的落点 | 必须满足的条件 |
|---|---|---|
| 异步调度与执行重叠 | P3 连续批次、预取和 compute/communication overlap | 实测 GPU 空泡与重叠效果；状态提交和取消有界 |
| prefill 成本预算 | P3 用本机曲线选择 chunk | 校准 H200 上的 `cost(q,c)`，不照抄其他芯片系数 |
| EP / 专家负载均衡 | P2 路由、grouped GEMM、热专家副本 | placement 改变，专家身份和 gate 权重保持；真实通信可用 |
| DSpark 与 confidence 调度 | P5 先固定验证，再比较自适应验证预算 | 模型兼容草稿和真实 confidence；包含 draft/verify/通信总成本 |
| SWA bounded replay | P4 模型层的独立实验 profile | 语义与数值门槛独立；不能在网关删掉长提示 |
| KV 工作量与负载共同决策 | P3 admission / P4 有多个执行副本时的路由 | 使用实际状态和有效成本观测，不能以 VRAM 占用猜可用 KV 块 |

以上是机制改造，不是 SM100 内核的直接移植。可以通过进程内 ABI 复用有 SM90 支持的 CUDA 内核或通信库；保留 license、版本与实际 dispatch 身份，不要求重写所有数学内核。

官方来源：[README](https://github.com/deepseek-ai/DeepInfer/blob/9bd9cf243fc53ef9afbf8f04d391d989117d6ab9/README.md)、[部署 Q&A](https://github.com/deepseek-ai/DeepInfer/blob/9bd9cf243fc53ef9afbf8f04d391d989117d6ab9/doc/inference_deployment_qa.md)。公开引擎支持 SM100/Ascend 950；完整共享 KV 与跨副本负载均衡不包含在公开版。生产共享存储让 prefill 不需要 prefix-affinity；本机多副本若保留本地 KV，亲和只是另一种有条件的设计。

## 对照执行器与量化分支

SGLang 的 [V4.1 cookbook](https://github.com/sgl-project/sglang/blob/main/docs/cookbook/autoregressive/DeepSeek/DeepSeek-V4_1.mdx) 目前将该模型的推测路径称为 DSpark；不能套用普通 EAGLE/MTP 参数。该来源是浮动 main，实施时需固定新 revision 并核对安装源码。其低延迟和高吞吐 profile 分别选择 speculation 开与关，不能预设 DSpark 对所有 batch 都有增益。

- 官方主线：固定官方 snapshot，先关闭 DSpark 与 replay 数值变更，得到最小可信原生控制组。
- XQ 分支：现有社区 pack 含 PTQ1_0/Q2_0/Q3_K 和模型编辑，与官方检查点分开做数值及任务验收。不要将它称为通用 TQ1_0。
- XQ 的辅助 GGUF 完成不代表官方 DSpark 草稿已绑定；检查点、tokenizer、张量布局和目标散列必须分别核验。
- 八卡 H200 的 HBM 能容纳哪些权重、KV 与并行副本要按实际加载布局计算；优先测常驻，不能预设逐层 NVMe 读取更快。

## 调查后才能冻结的项目

1. 模型所有者文件位置及根工作区/本地 Power 构建闭包；不新增工作区、锁文件或仓库。
2. FP8 dense、MXFP4 expert、Engram、压缩 KV 的实际字节和受支持的 SM90 dispatch。
3. NVLink/NVSwitch/P2P 拓扑、进程内通信库可用性与独占八卡资源。
4. 数值误差、模型质量、延迟、内存与校准曲线的晋升预算；候选测试开始前冻结。

这些是工程调查任务，不是等待用户授权的事项。调查不通过时保留拒绝理由和下一依赖。

## SM90 精确 BF16 展开候选（未实施/未资格验证）

独立标量位证明检查了全部 16 个 E2M1 nibble × 255 个有限 UE8M0 比例因子：4,080 对中 4,068 对为有限 F32，均可按位往返 BF16；12 对 F32 溢出必须拒绝，包含负零/次正规数。记录为 `docs/evidence/power/2026-10-22-native-deepseek-h200/h200-exact-bf16-expansion-candidate.json`。这只证明存储表示，尚未证明 H200 BF16 tensor-core 次正规数运算、CUDA 数值误差或性能。

因此 T008/T011 可资格验证一种明确标记的 SM90 fallback：保持原始 FP4 身份，只有逐值精确时才展开 BF16 常驻/GEMM，并保持实际 FP8 激活量化。官方主干 routed expert 的 543,581,798,400 个逻辑值将需 1,087,163,596,800 字节；均匀五层/卡为 135,895,449,600 字节，尚未包含共享/注意力/Engram/embedding、KV/提案、CUDA/allocator/scratch 或放置不均。不能据此声称整模能容纳八卡，更不能声称提速；先测真实逐 rank 峰值与算术资格。当前原始字节控制实现没有晋升到此候选。
