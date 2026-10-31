# Tasks: Power 原生 DeepSeek V4.1 / 8×H200

**Input**: [spec.md](spec.md)、[plan.md](plan.md)、[research.md](research.md)、[contracts/native-execution.md](contracts/native-execution.md)。

**House rule**: 每项行为与测试同任务；勾选项必须有实际证据。新模型所有者路径先由 T001 验证。没有标记 `[P]`，不授权并行 agent 或共享状态操作。用户要求在原生部署后交付访问地址和临时 API Key。

## Phase 0: 能力、构建与基线（4 项）

- [x] T001 [US1] 审计根 `Cargo.toml`、`crates/power/Cargo.toml` 与本地 Aster 闭包，确定 `crates/deepseek/` 的根 workspace 接入及单向依赖。必要 manifest 修正作为独立完整切片，验证 `publish=false`、无新 workspace/lock、依赖守卫与最小 metadata/build；保留存量差异，不写机器缓存路径。
- [x] T002 [US1] 在 `docs/evidence/power/<实施日期>-native-deepseek-h200/` 固定源码/实际构建身份、原生算子/格式/collective 缺口矩阵；读取 `src/inference/` 与精确检查点 schema，不运行共享引擎变更。
- [x] T003 [US2] 在 `crates/deepseek/src/reference.rs` 及 sibling tests 定义微型权重/状态/输入夹具与 CPU 数学参考，在证据中冻结同模型数值/质量预算；不下载大模型即可证明夹具覆盖和坏格式拒绝。
- [x] T004 [US5] 在 `scripts/h200_xq/{remote_install,service,bootstrap,benchmark,autotune,deferred_launch}.py` 同步已测 Power 精确计数构建与基准/正常模式，测试 pins、取整计数拒绝、失败保留、finally 恢复、现有 launcher 所有权和不重复申请；真实 nonweight preflight 通过后才更新已有等待进程，不重启下载器。

## Phase 1: 原生正确性与服务接入（6 项）

- [x] T005 [US1] 在模型所有者 `model.rs` 和配置接入定义精确模型/执行身份与闭集原生能力；sibling tests 覆盖未知模型/schema、缺 CUDA、禁止代理或 CPU fallback、parse/as_str 与 ACL roundtrip。
- [x] T006 [US2] 在模型所有者 `weights.rs` 接入 shard/range 与 quantized tensor descriptor；sibling tests 覆盖缺 scale、坏块、溢出、截断、散列/布局错配及原始量化字节保持。
- [x] T007 [US2] 在模型所有者 `layers.rs`、`state.rs` 实现 CED/Engram、稀疏注意力与精确 MoE/KV 更新；微型逐算子/逐层参考 tests 验证 chunk/decode、索引、mask、top-k/gate、上下文边界与回滚。
- [ ] T008 [US2] 在模型所有者 `cuda.rs` 和必要的 `crates/power/src/inference/accelerator/` 通用接口接入 SM90 FP8/MXFP4 内核，保留原 F32 API。CPU 小夹具验证 dispatch/refusal，GPU tests 验证比例因子、输出误差、scratch 与显式 fallback；记录实际 kernel/ABI 身份。
- [ ] T009 [US2] 在模型所有者 `generation.rs`、`parallel.rs` 组合最小正确完整模型路径；容量需要时先实现模型语义安全的按层切分与显式 peer transfer，不依赖 T012 的高效 TP/EP。hermetic tests 验证 token loop/state/切分边界/格式；独立 H200 实测单层与完整八卡官方检查点，固定 logits 与任务门槛，禁止以单层结果代替完整推理。
- [ ] T010 [US1] 用 `crates/power/src/backend/mod.rs` 的注入边界与模型所有者 composition 入口接通原生服务；sibling tests 验证真实增量 text/tool、错误、取消、实际用量、未知约束拒绝，以及参考 HTTP 不可用时的小模型原生生成。

## Phase 2: 常驻与真实八卡效率（4 项）

- [ ] T011 [US3] 在 `src/inference/residency/`、模型所有者 `weights.rs` 实现量化权重/激活驻留与 Engram 放置；sibling tests 验证最坏峰值预算、pin/淘汰、预取取消、不隐式变精度及不必要的 host copy；H200 对照 HBM 与必要分层成本。
- [ ] T012 [US3] 在模型所有者 `parallel.rs` 与 Power 通用 accelerator 接口实现实际 TP/EP collectives 和受支持的 attention DP；CPU 分片模拟 tests 加真实 GPU collective/拓扑/rank 故障 tests，不以 mesh 声明充当通信实现。
- [ ] T013 [US3] 在模型所有者并行执行与现有 `routing.rs` 合同上实现 grouped expert execution 和有界热专家副本；tests 验证 top-k/gate/顺序保持、epoch 原子切换、冗余字节预算及异步拷贝失败回滚。
- [ ] T014 [US3] 在模型所有者 CUDA 执行及现有 `shape_profile.rs` 接入已测必要的融合、图复用或 launch 修正；sibling tests 验证 finite shape、旧绑定拒绝和输出边界，H200 trace 与逐项 A/B 证明瓶颈改善。

## Phase 3: Batch、预算与重叠（3 项）

- [ ] T015 [US4] 将模型 KV slot 接入 `src/inference/execution_batch/`；sibling tests 覆盖 ragged、晚到、序列完成、行级取消、原子提交和逐请求参考对照；H200 测真实 batch kernel，而非只提高 admission。
- [ ] T016 [US3] 在模型所有者调度/成本模块与 Power tuning 合同中增加基于 H200 校准的 prefill chunk 决策；sibling tests 覆盖单位、预算、单调性、过期/NaN/外推拒绝、公平与有界固定策略回退。
- [ ] T017 [US3] 接入异步预取、CPU 调度与 GPU/通信重叠；sibling tests 覆盖事件依赖、取消、资源回收及并行/串行结果；GPU trace 区分执行重叠与 host 提交，不从较短日志推断提速。

## Phase 4: KV 与长上下文（2 项）

- [ ] T018 [US4] 在模型所有者 `state.rs` 和 Power 现有 prompt-cache 能力边界接入精确前缀复用；sibling tests 覆盖隔离、分叉、model/template/tokenizer 变化、epoch/淘汰、取消和 cached/uncached 一致性，按需另测 DRAM/NVMe 传输总成本。
- [ ] T019 [US4] 在模型所有者 `layers.rs` 中实现独立 SWA replay 实验 profile；sibling tests 验证 window/backtrack 覆盖、KV 命中截点、长上下文与约束请求回退；真实质量/数值门槛通过才晋升，完整 prefill 控制组保留。

## Phase 5: DSpark（2 项）

- [ ] T020 [US4] 在模型所有者 `speculative.rs` 接入精确目标/草稿绑定、候选验证、采样与完整状态回滚；sibling tests 覆盖全拒绝/全接受/部分接受、所有 rank 决策一致、工具/语法混合和取消；H200 记录实际 draft/verify/accepted 成本。
- [ ] T021 [US4] 在 `speculative.rs` 与成本曲线模块中，仅在兼容 confidence 与可变验证能力可用时增加自适应预算；sibling tests 覆盖无 confidence、过期曲线、最坏预算、位置依赖与全部验证；匹配实验和最优支持固定策略比较，不只比较接受率。

## Phase 6: XQ、验收与交付（6 项）

- [ ] T022 [US2] 在模型所有者 `weights.rs`/CUDA 格式分支增加精确 XQ 适配，保留官方路径身份。CPU 解包/坏块 sibling tests 与 GPU 小张量数值对照一起实现；权重完成后验证 XQ 自身完整推理及 DSpark 兼容，不借官方结果代替。
- [ ] T023 [US5] 在 `scripts/h200_native/` 实现 source/model/kernel 绑定、失败保留与匹配实验；hermetic runner tests 覆盖 token/时钟/分位数/口径/隐私、拒绝跨权重倍率及遗漏失败；独占 H200 完成三次以上交替对照与 profile，单请求/总吞吐分别判定 1000。
- [ ] T024 [US3] 在 `scripts/h200_native/` 实现拥有者核验、drain、受限作业与 profile 晋升/回滚；hermetic tests 验证 PID/marker/启动时间、观察失败不重启、不重复作业、下载保留及异常恢复；实机演练原生控制组恢复与真实 GPU 工作停止。
- [ ] T025 [US5] 在 `docs/evidence/power/<实施日期>-native-deepseek-h200/OUTCOME.md` 保留每阶段命令、构建/权重/输入身份、tests/counts、原始负结果、实际性能和 Explicitly not claimed；闭合下列验证门槛后再报告对应阶段完成。提交只在用户对该实施明确要求时进行，并逐文件检查。
- [ ] T026 [US1] 在原生 H200 服务、鉴权所有者与 `scripts/h200_native/` 中完成用户入口和临时 Key 交付；同任务 hermetic tests 验证期限、时钟/重启不延长、撤销、常量时间校验和日志脱敏。实机从用户入口验证 stream/tool、匿名/无效/过期/撤销拒绝，再交付原生实例地址、模型别名、新 Key、到期时刻与撤销方式；Key 不进入仓库、证据、进程参数或公共日志。
- [ ] T027 [US3/US4/US5] 在 `scripts/h200_native/` 实现并运行升压/持续压力/过载与故障 campaign。hermetic tests 验证有限 worker/请求/时钟、开放负载拒绝计数、SSE usage、所有失败保留、取消回收和 finally 恢复；真实八卡上为交互/吞吐晋升配置各运行至少 30 分钟，含长短请求、工具、多轮/分叉/隔离、过载、取消、任务自有 rank/worker 故障和恢复。输入/输出/思考/缓存身份、资源/时延/质量门槛和停止预算在运行前冻结；保留所有请求和 trace。FR-014/015、SC-007/008 对应 T012–025/T027，不能以基本服务验收代替。

## Phase Dependencies

2026-10-24 多副本目标补充（原有正确性、优化、压力和交付门槛保持）：

- [ ] T028 [US3/US4/US5] 支持闭集 2/4/8 卡的完整 40 层副本放置、每组实际 CUDA/UUID/显存准入，保留单八卡控制组；hermetic tests 验证完整层覆盖、外部 ordinal 拒绝和未参与设备不计数；同一八卡上逐对验证四个独立双卡组的 peer/cache 路径。完整权重、Engram 放置、真实整模生成和逐组实际内存通过后才建立服务副本。
- [ ] T029 [US1/US3/US4/US5] 在模型服务 composition 与 Power 调度所有者边界建立至少两个独立完整原生副本，绑定互斥物理 UUID、模型/缓存 owner/epoch，使用有界队列与精确 KV 亲和路由；hermetic tests 验证同请求固定组、租户/分叉隔离、过载和取消，实机比较 4×2、2×4 与 1×8 控制组并完成 T023/T027 的性能、故障和压力验收。副本数按实测选择，不能将 stage、线程或网关 admission 当作完整副本。

T001–004 先完成。按 2026-10-23 的 XQ 优先部署补充，本轮执行顺序为：已完成 T005–007 → T022 的格式/CPU 小块证明 → T008/T022 的对应 SM90 内核资格 → T009/T011 的完整生成及容量 → T010/T024 的基本服务与入口验证 → T012–023 的性能优化/晋升 → T027 压力与恢复验收 → T025/T026 最终证据和用户 Key 交付。基本服务可作为中间里程碑，不能关闭用户新增的优化及压力目标。T008 的官方 FP8/MXFP4 硬件作业继续独立保留，不以 XQ CPU 格式测试关闭它。T022 的纯格式切片不依赖 H200 资源，CUDA 与整模验收仍保留，整个 T022 完成前不勾选。T023–025 持续保留各阶段验收与负结果。官方与 XQ 的完整验收独立，不以任一路径替代另一路径。

当前阶段状态（2026-10-23）：T001–T007 的规定范围已验收；XQ 主权重逐片下载校验完成。T008 的真实 H200 资格仍待资源，T009–T010 完整原生生成与服务、T022 XQ 原生格式和 T026 交付未完成。GPU 阶段继续依赖独占资源与可用控制连接。最新 XQ 优先部署方案见 [plan.md 的 2026-10-23 补充](plan.md#2026-10-23-补充h200-上优先部署-xq)；本轮已同步上述执行依赖，任务勾选不变。

2026-10-24 性能口径确认：用户要求**单个请求生成速度达到 1000 token/s**。
T014/T017/T020/T023 以降低单请求 decode/验证延迟为优先目标；T015/T027
的 batch/并发结果独立保留，不替代该单请求门槛。当前四完整副本及基本
API 已通过短请求实测，但基线仅约 0.51 token/s 单请求端到端、1.28
token/s 四请求合计，目标尚未完成。任务勾选保持不变，证据见 OUTCOME。

2026-10-24 优化续记：v29 的步内不可变校验复用通过 H200 451 控制/19 次
GPU 调用；四副本各预热一次后的三次原生 decode 为 1.703/1.694/1.747
token/s，同源码关闭缓存的 v30 整模对照仍在加载。下一轮 v33/v34 的原创
SM90 FP8/FP4 编解码融合仅完成 450 本地控制、严格 NVRTC 编译和暂存，尚无
GPU 数值或性能结论。上述结果不关闭 T014/T023/T027 或单请求 1000 门槛。

2026-10-22 实施记录：T001 的 locked/offline metadata、最小模型构建、31 项依赖适配测试和 3 项 Runtime 测试通过；依赖守卫中本地 Aster 与 unpublished 规则通过。全仓守卫另有桌面夹具失败、漂移和 Windows fmt 长参数限制，保留在最终门槛，不宣称全绿。T002 已检查官方 48 个 shard 的头部、96,085 个张量与实际 dtype，但没有核验 510 GB payload 散列或执行模型。证据见 `docs/evidence/power/2026-10-22-native-deepseek-h200/analysis.md`、`native-gap-matrix.md` 及各项日志。T003 的数学参考正在实施；模型状态夹具和同模型 GPU 预算仍待完成。

P0 续记：T003 现有 25 项数学/C1/C2 状态测试通过；六类官方原始输入各采集三次 teacher-forced 概率，固定输入 IDs、113 个评分位置及版本化数值/质量预算。实际原生 GPU/layer 质量仍在 T007–T009 验收。T004 的远程 Linux 68 项测试无跳过全部通过；H200 上真实 server-only Power + 合成上游验证精确计数及正常/异常恢复。新准备包经两次 nonweight preflight 后激活，新等待器 PID 335316 / start_ticks 259373957 已确认处于无 GPU 预留的等待状态；原下载器 PID 217634 / start_ticks 255375652 保持运行。T004 的外部 Xolotl 准备不替代 T005–T010 原生模型实现。详见同证据目录 `qualification-budget-v1.json`、`reference-compression-tests.log`、`xq-prepared-release.json`、`h200-frontend-usage-smoke.json`、`xq-preparation-activation.json`。

## Final Verification Gate

P1 续记：T005 的模型/执行/ACL 闭集校验通过，T006 的固定官方分片散列绑定、跨分片原始量化读取及 dense/Engram/FP4 不同 scale 布局通过，累计 54 项测试。T007 已实现第一段 Rust/Candle mHC 数学组合，新增六项小型 CPU 数值/形状测试，累计 60 项测试及 production check 通过；公开构造要求实际 CUDA 设备，尚无 CUDA/GPU 数值证据。Engram、稀疏注意力、MoE/KV、状态/回滚和官方参数绑定尚未完成，T007 保持未勾选。各命令/源码身份/局部与全仓检查限制见实施证据 `OUTCOME.md` 和 `native-mhc-*`。

T007 再续记：已新增 Rust 官方 tokenizer 压缩、固定 Engram 哈希参数、含图片边界的事务式 n-gram history 和带符号根门控，以及 MoE 稳定选择/带权激活。完整 129,280 项词表映射与远程参考散列一致，当前 80 项小型测试和 all-targets check 通过。尚未接入完整 embedding gather/量化投影、GPU score/专家 GEMM、KV 格式与稀疏注意力及整模执行；不勾选 T007，不把 CPU 数值检查作为 GPU 部署证据。

T007 压缩/注意力续记：`state.rs` 已实现 C1/C2 张量计算、单行待配对状态、提案租约和接受前缀提交；`layers/attention.rs` 已实现因果/SWA 候选验证及含 FP32 sink 的 decoded MQA 控制。全部分块与回滚、缓冲实际长度和独立 F64 对照通过，当前 98 项测试和 all-targets check 通过。尚无完整 KV 格式/索引器、规范化/RoPE 和整层组合、CUDA 构建或 H200 执行证据；不勾选 T007。字节预算为静态张量估计，不能代替后续 Power 全模型资源预留及 GPU workspace 实测。

T007 norm/RoPE 续记：有效权重 RMSNorm 和显式 FP32 cache 的旋转已加入 Rust/Candle 控制实现；不自动取指数或额外缩放 norm 权重，不以 FP64 频率替代上游 FP32 cache。测试覆盖两种配对布局、共轭保持幅度、区间外位保持、分块/逐 token 一致、位置/预算/取消拒绝，当前 112 项 CPU tests、all-targets check 与 package fmt 通过。频率/YaRN 构建、官方完整参数绑定、索引器/packed KV 与 GPU 数值验收仍未完成，T007 保持未勾选；全仓 fmt 仍遇 Windows error 206。证据见 `native-norm-rope-*` 与 `norm-rope-source-reference.json`。

2026-10-23 T007 索引/量化续记：新增因果/候选范围内的稀疏索引器控制、BF16 打分后 top-k、排序及 padding、强制最近块的 max 候选筛选；新增固定 group-32 的索引 FP4 RNE 编码、原始 U8 字节/比例因子所有权和按需解码，并已接到索引器。全部 65,280 种有限 BF16 编码与独立标量 oracle 一致，累计 138 项 CPU tests、all-targets check、package fmt 通过。DeepSelect 并列子集未指定，本控制路径采用最小逻辑下标；不声称逐个 tied ID 与参考相同。主 KV 两种格式、跨层索引/候选共享、整层/模型组合及 SM90/GPU 数值验证仍缺，T007 保持未勾选。证据见 `native-index-quant-verified-*`、`index-quant-source-reference.json`。

2026-10-23 T007 主 KV/拓扑/路由续记：主 KV 的 FP8/UE8M0 与 FP4/E4M3 精确记录、按选择解码的混合稀疏注意力、事务式 C1/C2/SWA KV 接受前缀状态已加入；shifted mHC 现在保持上一子层 pre mix，并正确处理初始/最终混合及 Engram 干预。新增官方源拓扑绑定与 source-only 状态工厂，明确各层 SWA 使用自己的隐藏状态；新增 BF16 参数、FP32 logits 的路由投影和保留极小值的 FP32 sqrt-softplus，接通现有 top-k。累计 182 项 CPU tests、all-targets check、package fmt 通过；全仓 fmt 仍为 Windows error 206。主 KV 的一次 scalar oracle 失败及孤立复现保留；修复 oracle 大数饱和比较后通过，没有降低生产精度。完整参数/整层组合、FP32 RoPE 频率构建、index cache/模型级原子状态、量化 GEMM、CUDA 构建和 H200 实测仍未完成，T007 不勾选。详见 `native-kv-topology-router-*`、`kv-topology-router-source-reference.json` 和 OUTCOME.md。

2026-10-23 T007 index-K/原子提交续记：新增保留独立 FP4/UE8M0 字节区段的 C1/C2 索引缓存及 source-only 工厂，已把准备/接受后的真实 packed rows 接入稀疏索引器验证因果屏蔽与可见性。新增包含压缩、主/窗口/索引 KV 和 Engram 历史的两阶段 StateCommitBatch；阶段失败会毒化批次，最终设备同步和取消检查后才交换所有者缓冲，旧张量保留到全部所有者更新后再回收。19 项新测试覆盖接受前缀、图片边界、取消、后续拒绝、预算和旧缓冲生命周期；累计 201 项 CPU tests、all-targets check 和 package fmt 通过，全仓 fmt 仍为 Windows error 206。尚未接入完整模型参与者集合、完整层/参数/投影、精确 FP32 RoPE cache 生成或运行 CUDA/H200；T007 保持未勾选。证据见 `native-index-atomic-*`、`state-commit-device-boundary.json` 与 OUTCOME.md。

2026-10-23 T007 原始投影/专家续记：新增 Power 验证范围读取到原始设备矩阵的连接、局部/重复行 gather、group-32 FP8 激活编码和按 32 个输出行展开的 F32 数学控制；量化投影必须接收实际 packed FP8 激活。新增 gate/up → BF16、SwiGLU 内 F32 路由加权、直接 FP8 cast → BF16 down 的完整单专家组合，尚无完整 MoE dispatch 或整层执行。17 项新测试覆盖多组/异号 F64 dot、原始 FP4 位、SF 布局、全部有限 BF16 编码和完整专家精度边界；累计 218 项 CPU tests、all-targets check、package fmt 通过。一次 Candle Var 拷贝语义的测试假设失败已孤立复现、修正并保留；全仓 fmt 仍为 Windows error 206。此路径不是 SM90 低精度硬件内核或性能结果；完整参数/模型组合、精确 FP32 RoPE 生成、CUDA/H200 资格仍缺，T007 不勾选。证据见 `native-raw-linear-final-*`、`raw-linear-source-reference.json` 和 OUTCOME.md。

2026-10-23 T007 MoE/FFN 续记：完整单设备 MoE 已连接实际 packed FP8 输入、MXFP4 路由专家、FP8 共享专家、分组分发和 F32 合并；shifted mHC → learned RMSNorm → MoE → mHC post 的 FFN 半层已完成。229 项 CPU tests、all-targets check 和 package fmt 通过。DeepEP 固定参考揭示共享输出必须先初始化 F32 累加器；保留了先加路由导致 0 而非 18 的孤立负结果及修复。此控制路径没有 local/scaleup grouped BF16 reduction，不宣称等价于其他 EP profile。无实际 GPU/TP/EP 或性能证据，T007 不勾选。

2026-10-23 T007 索引/主压缩器续记：新增索引 Q 的实际 FP8 输入投影、尾部 RoPE、FP4 编码与 BF16 head weights，以及索引 K 的主 KV post-norm/pre-RoPE 输入 → BF16 projection → learned norm → RoPE → FP4。固定源码调用省略 HadamardArgs；未按函数名添加 Hadamard。新增主 KV 的 BF16 A/Gate 投影、C1 BF16/C2 F32 精度边界、压缩 → norm → RoPE → 两种 packed main KV，并将主/索引/压缩状态一起接到原子提交集成验证。每种四 token 分块、部分接受、取消和独立数值参考通过，累计 240 项 CPU tests、all-targets check 与 package fmt 通过；缓存 fixture dtype 和负零标量 oracle 的失败、孤立复现及修正均保留。完整 attention/output/Engram 参数/表、模型状态参与者、FP32 cache 生成和 CUDA/H200 资格仍缺，T007 不勾选。证据见 `native-compressor-*`、`index-compressor-source-reference.json`、`p1-compressor-source-identity.json` 和 OUTCOME.md。

2026-10-23 T007 attention/Engram 续记：主 Q/SWA 与分组 Wv/Wo、共轭 RoPE、BF16 → FP8 输出精度边界、shifted attention 半层已完成；准备句柄保留压缩/索引所需的实际当前 hidden、packed Q 与窗口行。Engram 已连接 Power 验证/清零范围读取、排序去重/有界相邻行读取、原始 24×256 FP8/SF 拼接、FP8 fused projection → BF16 gate 与 mHC carry；图片负一整行不读表，checkpoint scale 0 保持 OCP 语义，live-cast padding 拒绝保持。新增权重快照、finite/mask/组合预算检查。累计 263 项 CPU tests、all-targets check 与 package fmt 通过；全仓 fmt 仍为 Windows error 206。文件删除不使 Power 已打开句柄失效的错误 fixture 假设、孤立失败和截断夹具修正均保留。实际 GPU/完整模型参数与状态共享、精确 FP32 cache 生成仍缺，T007 保持未勾选。证据见 `native-engram-attention-final-*`、`attention-engram-source-reference.json` 与 OUTCOME.md。

2026-10-23 异常关闭恢复续记：保留并重验中断前的 FP32 RoPE cache、53 参与者 BackboneState 和 attention/Engram/FFN decoder 组合，原有 281 项 tests 全部通过。新增官方 40 层参数 schema，与独立检查点 header 审计的 93,415 个描述逐层散列一致；按实际 source 层绑定 compressor/index 参数，保留专家 MXFP4、共享专家 FP8 和 Engram per-row SF，并通过 Power 有界读取小型 BF16/F32 参数。累计 287 项 CPU tests、all-targets check 和 package fmt 通过。第一次摘要对照因测试使用了字面量反斜杠 n 而非 LF 失败，已孤立复现并修正，独立参考散列未改动。全仓 fmt error 206、桌面构建脚本 fixture 失败和 61 项既有漂移分别保留。完整模型运行时组合、CUDA/H200 资格仍缺，T007 不勾选；见 `native-parameter-*`、`resumed-*` 和 OUTCOME.md。

2026-10-23 T007 本地层/状态验收完成：原始设备矩阵改为按比例因子块对齐的 Power 有界读取，支持完整矩阵大于单次预算、FP8 尾块和跨段 SF；新增官方解码层装配、source-only compressor/index 装配、40 层 top-k/candidate 复用与 `forward_pending` 状态事务连接。独立微型共享专家输出固定为 BF16 3072；真实索引/attention 的 40 层用例验证第 24 层起采用第 20 层的候选块。奇偶 C2 分块、无完成 pair、失败污染、取消、未提交 KV 和丢弃后重新申请均通过。302 项 CPU tests、locked/offline all-targets check 和 package fmt 通过；证据 `t007-loaded-step-acceptance.json`、`native-loaded-step-*`。该勾选仅关闭 T007 规定的 hermetic 算子/层/状态范围；实际 SM90 内核/CUDA 数值验证属于 T008，完整模型循环/放置和 H200 整模验证属于 T009，二者未完成。全仓 fmt error 206、桌面构建夹具错误和既有漂移仍单独保留。

2026-10-23 T008 CUDA 实施续记：新增原始 FP8/MXFP4 group-32 CUDA 投影、显式原始 FP32 scalar 与既有 tensor control 三种闭集 profile，保留 Power F32 API。CudaLayerDevice 显式携带 profile；架构、几何、比例因子范围、预算或驱动 local/shared 分配不合要求时拒绝，不自动回退。NVRTC、SM90 ptxas 和 CPU/CUDA all-targets check 通过；CPU 305 项、CUDA feature 305 项通过，另 2 项 H200 tests 明确 ignored。ptxas 把 E4M3 MMA 降为寄存器转换及 HMMA 半精度 Tensor Core 指令，不能宣称硬件 FP8 吞吐。实际独立 H200 双 profile 作业 `aster-sm90-probe-1792727645-57460086` 仍 Inqueue/Unschedulable，无 GPU 数值或 Rust adapter 运行结果；T008 保持未勾选，T009 尚未开始。身份、负结果、源码/SASS、调度及验证记录见 `sm90-*.json/log/txt/py` 与 OUTCOME.md。XQ 下载仍独立保留。

2026-10-23 T008 Linux 构建续记：当前根快照的 locked/offline metadata 与 `cargo test -p aster-deepseek --locked --offline --features cuda --lib --no-run --message-format=json` 已在 Ubuntu 22.04 / Rust 1.97.1 / CUDA 12.6.20 构建成功；实际 Rust 测试二进制 SHA256 为 `9d333f518b87735fd2bb2629e0ef17e65750ad34dac9111eb74f0a7793045f36`。期间工作区的 ORM manifest/lock 变化已单独绑定，原生本地源码与全部 registry/Git 锁定项未变，没有覆盖无关改动。二进制尚未执行；独立 Rust 硬件验收作业 `aster-sm90-rust-1792756991-93148350`（UID `96f7a6f4-1544-4dc8-8351-4eb258ea3c44`）已提交一次，20:06 +08:00 仍 Inqueue/Unschedulable。原 PTX 微测试作业保持独立。T008 不勾选，完整模型/生成与服务接入仍待 T009–T010；编译成功不替代 GPU 或整模验收。见 `linux-build-result-1792757322/`、`sm90-rust-job-1792757190.json` 和 OUTCOME.md。

2026-10-23 XQ 下载完成续记：20:15 +08:00 已观察到全部 16 片、总计 162,954,903,552 字节及原下载器的 `MODEL_PARTS_VERIFIED`；下载器逐片完整 SHA256 校验完成，观察器没有重读全部 payload。原等待器因缺少集群环境初始化而提交失败，确认无对应作业后，通过已有 launcher 锁、原 source/hash、真实 nonweight preflight 和已初始化的 RJob 子进程恢复一次申请，旧失败记录保留，未重启下载器。独立 XQ 参考部署作业 `aster-power-xq-h200-final-6522e5af2fc3-31676386`（UID `6230a46e-8868-4e2c-a803-0321962f98a6`）和两个原生内核作业在 20:27 均仍 Inqueue。下载完成不关闭 T022 的 XQ 原生格式/整模验收，也不关闭 T026 的原生地址/Key 交付。见 `xq-completion-inspection-1792757713.json`、`xq-environment-submission-recovery.json`、`h200-owned-job-states-1792758431.json`。

2026-10-23 T022 格式切片：已实现 `weights/xq.rs` 的精确混合量化布局、有界行解码及 I32 元数据，`weights/xq/gguf.rs` 的固定头部绑定和范围校验；新增 17 项 tests，完整 CPU 模型套件累计 322 项全部通过。真实 XQ 头部的 1,329 个张量与独立审计的名称、形状、格式、偏移和字节完全一致。此范围没有验证完整 payload、设备投影或模型生成，T022 不勾选。共享根目录发生并发 stash/pull 后，用户明确要求恢复；任务文件从已识别 stash 找回，当前工作位于仓库内 `.wt-h200-native-deploy` 独立 Git worktree，保留已测 native manifest/lock 和私有子模块边界，不覆盖主工作区的并发改动。`xq-format-acceptance.json`、`xq-native-header-comparison.json`、`isolated-worktree.json` 及 OUTCOME.md 保留恢复和验证范围。用户新增的充分优化与压力测试由 FR-014/015、SC-007/008、T027 覆盖，尚未完成。

2026-10-23 T022 存储/设备续记：Power 新增 `VerifiedRawWeightFile` 与清零范围所有者，完整 SHA256 与后续被触及块的散列使用同一持有句柄，拒绝路径别名/链接、坏身份、篡改、截断、越界及预算/取消失败。XQ 接通固定完整身份、原始行读取、80 个专家类型表的加载规则、独立 Gate/Up 与 Down 分区及原始设备上传。新增显式 F32 warp CUDA 数学对照，NVRTC 12.6 与 SM90 ptxas 成功（32 registers、无 spill/stack），不声称 Tensor Core 或性能晋升。CPU 模型累计 337 项通过；CUDA feature 338 项通过，3 项 H200 tests 仍明确 ignored；Power 存储回归 61 项通过。最新 23:06 +08:00 三个所有权作业仍 Inqueue，调度事件保留 CPU/内存/节点选择限制。此切片未执行真实 payload 的原生完整校验或 GPU 投影，更未实现完整 XQ 生成和服务；T022、T008、T009–T027 相关验收保持未完成。Linux 当前源码资格二进制在既有独立 CPU 构建容器准备，旧排队作业的输入身份不变。见 `xq-native-storage-cuda-control-acceptance.json`、`xq-cuda-compile.json`、`xq-linux-build-inputs.json`、`owned-jobs-1792768008.json` 和 OUTCOME.md。

2026-10-23 Linux 资格产物续记：本轮 XQ 源码快照 `47411554055156dcc3ebd04ecc882464a5d1c7eebd3ab43bee5c96bfd34da6d4` 的 Linux CUDA 测试二进制已在原有 8 CPU/20 GiB/无 GPU 容器构建成功，二进制 SHA256 `5fafca73b63b9c590d9ec060a7f8bdb825138a661c82af39cc7ae77bfcf55268`，未执行 H200。Linux 的无 CUDA 原始存储测试 12 项通过，包括文件和目录符号链接拒绝；初始离线公开缓存缺包及按 Cargo.lock 校验的补齐记录均保留。最终当前源码 CPU 337 项、CUDA feature 338 项通过（3 项 H200 ignored），已匹配构建输入且恢复原构建入口。此补充不关闭 T022 或硬件/服务/性能验收。见 `xq-linux-build-result/receipt.json`。

2026-10-23 深夜 T022 F32 组件续记：核对固定 Xolotl runtime/exporter 后，新增 XQ 小参数的 Power 范围加载、独立 F32 RMSNorm/shifted mHC、F16 embedding gather 与 carried-mix → norm → Q8_0 head。mHC 先 RMS 再 F16 投影，保留四路 F32 trunk、20 次 Sinkhorn、上一子层 pre mix 及最终 head reduction，不套用官方 BF16 层。SM90 gather 按原始 packed 行解码，不常驻展开全部 embedding。审查修复了组件组合时不同资源 limits 的绕过路径。CPU 全量 349 项、CUDA feature 350 项通过；5 项真实 H200 tests 保持 ignored。Linux 同一源快照 `84ca8ee1f2701357bc07e138204a132c664c72dbbb617146de457e16afb91c97` 构建成功，二进制 SHA256 `2c83234183164bd83d7c3f6d343b8398a4793adc3186e432badc7edf80e06db1`，Linux 存储 12 项通过；旧 GPU 排队输入未替换。23:55 +08:00 三个自有 GPU 作业仍 Inqueue。本项没有完整 F32 XQ MoE/attention、模型循环、服务、性能或压力结果，T022 不勾选。详见 `xq-f32-components-acceptance.json`、`xq-runtime-reference-manifest.json`、`xq-f32-linux-build-result/` 与 OUTCOME.md。

2026-10-24 T022 MoE/FFN 续记：新增 `layers/xq/moe.rs` 的 F32 router/score、常驻 Q2_0/Q3_K/PTQ1_0 专家、Q8_0 shared 分支和完整 shifted FFN half；精确类型表分别绑定 Gate/Up 与 Down，行投影只借用完整栈，不创建低估驻留字节的独立视图所有者。Down 后应用路由权重，路由 lanes 累加后再加 shared，全程不插入官方 FP8/BF16 激活边界。CPU 全量 357 项、CUDA feature 358 项通过，6 项真实 H200 tests 明确 ignored；CPU/CUDA all-targets 与 owning-package fmt 通过。新增小架构 H200 test 用独立 F64 组合比较混合专家、文本/图像路由、batch 和完整 shifted FFN，误差预算在硬件运行前固定。首轮一项 F32 十进制边界断言失败已保留日志，按实际 F32 加法位模式修正后单独重跑及全量通过。Linux 同源二进制构建成功（SHA256 `e1a485ab534e036ba145c6c5999a4371b883b837878c5e5fc89730ec5d4f07a7`），尚未执行 GPU；旧排队输入保持原样。仍缺 F32 attention/Engram/cache/state、完整模型/生成和服务，T022 保持未勾选。见 `xq-moe-components-acceptance.json`、`xq-moe-linux-build-result/` 与 OUTCOME.md。

2026-10-24 T022 attention 续记：新增 F32 除法版三类激活编码、显式 F32 RoPE、保留 F16 KV 边界的 sparse attention，以及固定 Q8_0 Q-A/Q-B/window 和 grouped-output 投影。量化采用固定 `ds41_ops.hpp` 的除法顺序，已用精确反例证明 compressed FP4 中倒数相乘会改变中点编码；拒绝超出支持范围、非有限及上端舍入溢出，不先转 BF16。原官方 BF16 API 合同保持不变。输入/输出组合核算各自的单个 RoPE 快照和完整 packed 矩阵，分组 A 借用整栈行；XQ F32 attention 在 GEMM 前拒绝已开启的全局 TF32 模式。CPU 全量 370 项、CUDA feature 371 项通过，7 项真实 H200 tests 明确 ignored。新增 H200 逐阶段独立 F64/编码字节对照覆盖 Q8 输入/分组输出、三类激活编码、F32 RoPE、F16 KV、sink 与精确除法/负零/subnormal 反例；误差预算运行前固定，不充当整模 teacher-forced logits 证据。仍缺 XQ F32 compressor/indexer/Engram、事务状态接线、完整生成及服务，T022 不勾选。见 `xq-attention-components-acceptance.json` 与 OUTCOME.md。

2026-10-24 T022 compressor/index 续记：新增显式 `CompressionNumerics` 和 XQ 原始 F16/F32 C1/C2、index Q/K/head 投影及 F32 top-k。默认官方数值合同保持，已知 profile 不匹配在投影前拒绝；F32 C1/接受前缀与位置元数据按实际精度预算，C2 unmatched half 只保留已接受快照。XQ C2 readiness=pair end、RoPE=group start；主 pre-RoPE F32 latent 明确供 index K，head 权重显式缩放一次，候选末块使用实际最大值 + 有限 `1e30`，top-k 分数不转 BF16。两类源的全部 32 种六 token 分块组合保持同一 F32/量化结果。CPU 全量 385、CUDA feature 386 项通过，8 项真实 H200 tests ignored；新硬件项覆盖 F16 投影、F32 状态/index 以及取消/部分接受的三参与者原子缓存，不是整模/八卡证明。初始测试夹具 F64 缓存被正确拒绝，独立复现后仅显式改为 F32，负结果及重跑均保留。Linux 冻结源码与二进制构建/被动 staging 记录保留，未替换旧排队输入。仍缺 XQ F32 Engram、完整 backbone/model/生成与服务；T022 不勾选。见 `xq-compressor-index-components-acceptance.json`、`xq-compressor-index-linux-build-result/` 和 OUTCOME.md。

2026-10-24 T022 Engram 续记：完整 129,280 项 U32 token map 的 SHA256 与官方压缩词表一致，两个层的表长、48 primes、8 multipliers、encoding/PAD 均经固定 schema 与 Power 校验范围绑定。新增 XQ 原始 sparse E4M3/E8M0 表 → F16 fused projection → normalize-first F32 gate 和 carried mHC contribution，不提交 history；同任务补 sparse host metadata admission。CPU 全量 396、CUDA feature 397 项通过，9 项真实 H200 tests ignored；CPU/CUDA all-targets 与 owning fmt 通过。新增独立 F64/原始解码位硬件项尚未执行。负结果保留：一个预算夹具取到可接受边界，独立复现后改为低一字节；CUDA 夹具改用精确 F16 位而非添加未声明依赖。Linux 同源二进制构建成功（SHA256 `16413062b5b44012c7ff3850282c5d47419d19c244e18a6b42f2c6fceb13fa63`），新 v9 私有被动 stage 未执行或替换旧排队输入。仍缺 giant table 原生打开/放置、完整 XQ architecture/backbone/model/生成及服务；T022 不勾选。见 `xq-engram-components-acceptance.json`、`xq-engram-linux-build-result/` 与 OUTCOME.md。

2026-10-24 T022 逐层接线续记：XQ 模型身份、revision/alias/config SHA 和 packed kernel inventory 独立于官方；固定 GGUF config 与官方 metadata 的 JSON 值相同，但序列化身份不混用。F32 profile 从 verified architecture 贯穿 40 bindings 与 53 参与者 backbone，官方 loader/state 路径在读取前拒绝串用。新增完整 XQ shifted attention half 与 `LoadedXqLayer::forward_pending`，接通 Engram、window、source compressor/index K、声明的 index Q/top-k、sharing、attention 和 FFN；failure poison 全部 proposal，聚合预算含缓存视图与候选工作区。CPU 全量 409、CUDA feature 410 项通过；10 项 H200 tests ignored；CPU/CUDA all-targets 和 owning fmt 通过。新增硬件项只真实执行 2/3、20/21 source/consumer pairs，其他层明确合成状态，含分块/接受前缀/取消/foreign projection/TF32 拒绝；未运行且不充当整模证明。Linux 同源 qualifier SHA256 `194a7e0d6569ce30245b507117194a7b25bcd5a90e33e28237f44a6772419a68`，未改旧排队输入。仍缺真实原生 payload 打开、Engram 放置、peer transfer、40 层 model/token loop/backend/service 与八卡/压力验收；相关任务均不勾选。见 `xq-loaded-pipeline-components-acceptance.json`、`xq-loaded-pipeline-linux-build-result/` 和 OUTCOME.md。

- [ ] Owning package focused tests 全部通过，报告实际 features/命令/数量。
- [ ] Rust 修改执行 `cargo fmt --all -- --check`；存量失败单独记录，不修改无关源。
- [ ] 根工作区依赖/漂移守卫通过；无新增 crates.io/git Aster 依赖、独立 workspace 或 lockfile。
- [ ] 没有 native `<select>` 或其他产品 UI 改动；配置为 ACL。
- [ ] 原生执行、模型/内核、八卡参与和真实 GPU 取消均有实际证据。
- [ ] 正确性、质量、峰值内存、延迟与恢复门槛通过；单请求与总吞吐结果分别保留。
- [ ] 官方与 XQ 验收独立；原下载和旧证据保留；没有用参考服务冒充原生完成。
- [ ] 用户入口与临时 Key 的有效期、撤销和真实推理已验证，交付信息对应原生部署。
- [ ] FR-014/015、SC-007/008：真实八卡充分优化的匹配矩阵、瓶颈 trace、两种晋升配置各至少 30 分钟压力及过载/故障恢复验收通过；不是仅有服务启动或瞬时吞吐。
