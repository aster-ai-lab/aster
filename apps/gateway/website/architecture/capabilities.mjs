export const CAPABILITY_STATES = {
  base: "代码路径",
  conditional: "按配置",
  reserved: "未开放",
  absent: "未提供",
  separate: "独立路径",
  experimental: "实验能力",
};

// These nodes describe checked source paths, not a target platform or parity catalog.
export const CAPABILITY_GROUPS = [
  {
    id: "callers",
    title: "调用方",
    subtitle: "SDK / HTTP 客户端 / 应用",
    owner: "请求边界",
    source: "apps/gateway/src/entrypoint.rs",
    description:
      "所有数据面请求在图中只进入 Gateway 接入层。接入层是同一 Gateway 实例的逻辑边界，不表示源码只能绑定一个端口。",
    features: [
      ["caller-http", "模型 SDK 与普通 HTTP 客户端", "base"],
      ["caller-transport", "TCP / UDP 调用方使用对应监听器", "base"],
    ],
  },
  {
    id: "entrypoints",
    title: "Gateway 接入层",
    subtitle: "唯一逻辑入口 · 多协议监听器",
    owner: "entrypoint::listener / GatewayRuntime",
    source: "apps/gateway/src/entrypoint/listener.rs",
    description:
      "Gateway::start 为配置中的 entrypoints 建立监听器。HTTP(S) 与 Unix HTTP 进入 HTTP 请求处理；TCP、HostSNI、UDP 使用各自分支。HTTP 请求开始时取得一个 GatewayState 快照，业务路由在本地完成。",
    features: [
      ["http-listener", "HTTP/1.1、HTTP/2、TLS 与 Unix HTTP", "base"],
      ["raw-listener", "TCP / HostSNI / UDP 监听分支", "base"],
      ["listener-limits", "连接准入、信任转发头、优雅排空", "base"],
      ["multi-listeners", "同一实例可配置多个地址与监听器", "conditional"],
    ],
  },
  {
    id: "http-route",
    title: "HTTP 请求分发",
    subtitle: "标准检查 → RouterTable → RoutePlan",
    owner: "handle_http_request_inner",
    source: "apps/gateway/src/entrypoint.rs",
    description:
      "先处理前置本地接口，再拒绝 Reserved 标准，随后按入口、Host、Path、Method、Headers 匹配 RouterTable，取得预编译 RoutePlan。命中托管推理路由时，在请求中间件和收集 Body 之前认证凭据并移除客户端 Authorization。",
    features: [
      ["standard-guard", "Reserved 路径不回落到普通 catch-all", "base"],
      ["http-routing", "规则匹配、优先级与预编译 RoutePlan", "base"],
      ["early-identity", "托管推理先认证、建立身份并去除调用方凭据", "conditional"],
      ["reserved-standards", "/v1/mcp、API、Images、Gemini Embeddings 仍 Reserved", "reserved"],
      ["direct-http", "无策略、无推理、无观测的合格普通 HTTP 可走直接转发", "conditional"],
    ],
  },
  {
    id: "middleware",
    title: "请求中间件",
    subtitle: "请求策略 · 可提前返回",
    owner: "MiddlewarePipeline::process_request",
    source: "apps/gateway/src/middleware/mod.rs",
    description:
      "RoutePlan 绑定中间件管线。非 WebSocket 的请求阶段在处理 Body 之前运行；策略可修改请求、拒绝或直接响应。WebSocket 有自己的协议分发路径。提前返回的 HTTP 响应仍运行对应响应阶段策略。",
    features: [
      ["auth-policy", "API Key / Basic / JWT / Forward Auth", "conditional"],
      ["request-policy", "限流、IP、CORS、Header 与 StripPrefix", "conditional"],
      ["body-policy", "Body 限制、压缩与有界请求/响应处理", "conditional"],
      ["resilience", "重试、熔断、镜像及失败观察", "conditional"],
      ["middleware-extensions", "类型化 Rust 扩展；Redis / Wire 按构建与配置启用", "conditional"],
    ],
  },
  {
    id: "inference",
    title: "推理请求准入",
    subtitle: "Body 校验 · Grant · 额度预留",
    owner: "InferenceAuthorizer / OpenAiJsonRequest",
    source: "apps/gateway/src/entrypoint.rs",
    description:
      "只在配置所拥有的推理路由中执行。已认证身份进一步校验原生端点、模型别名、能力和 Grant；按有界 Body 解析请求，执行 RPM、TPM、并发与支出准入，建立 request / attempt 身份。不是所有普通 /v1 代理路由都执行这一管线。",
    features: [
      ["model-admission", "OpenAI、Anthropic 与原生 Gemini 的已准入端点", "conditional"],
      ["request-bounds", "有界 JSON / 模型 / 端点与能力校验", "base"],
      ["grants", "Principal / Credential、模型与端点 Grants", "conditional"],
      ["reservations", "RPM / TPM / 并发 / 支出预留与许可生命周期", "conditional"],
      ["audio-scope", "Audio 走普通 ACL 代理，不等于托管推理计量", "separate"],
    ],
  },
  {
    id: "model-policy",
    title: "模型目标决策",
    subtitle: "resolve_routing → prepare_next",
    owner: "InferenceDispatchState",
    source: "apps/gateway/src/entrypoint/inference_dispatch.rs",
    description:
      "resolve_routing 根据已声明模型策略与回合身份取得决策，保留本请求有效目标和路由证据。prepare_next 在授权、可用性与优先级窗口内准备一次尝试，选择目标和 Backend，改写模型/凭据并做需要的协议转换。失败决策回到主力/声明集合，不按关键词猜测任务。",
    features: [
      ["routing-modes", "Fixed / SmartSaving / Branches / FreeFirst", "conditional"],
      ["turn-decision", "决策服务、回合冻结与质量保护", "conditional"],
      ["target-window", "优先级、目标权重、健康过滤与尝试边界", "base"],
      ["translation", "OpenAI ↔ Anthropic、Responses / Gemini 对应处理", "conditional"],
      ["worker-scheduling", "Worker 调度、前缀亲和与 P/D 编排", "conditional"],
    ],
  },
  {
    id: "backend-select",
    title: "服务与 Backend 选择",
    subtitle: "ServiceRegistry · 健康 · 容量",
    owner: "entrypoint selectors / service",
    source: "apps/gateway/src/service/load_balancer.rs",
    description:
      "普通路由从 RoutePlan 的服务池选 Backend。推理分支先确定模型目标，再使用普通服务选择或声明的 Worker / P-D 选择器。Sticky、Revision、并发容量和 Failover 在各自配置生效时参与，不能把模型策略和服务负载均衡当成同一层。",
    features: [
      ["load-balancing", "轮询 / 权重 / 最少连接 / 随机", "base"],
      ["health", "主动与被动健康、可用后端剔除", "conditional"],
      ["sticky-revision", "Sticky、Revision 比例与容量约束", "conditional"],
      ["service-failover", "备用服务与失败目标选择", "conditional"],
      ["standalone-scaling", "Standalone Box / Kubernetes 扩缩", "experimental"],
    ],
  },
  {
    id: "proxy",
    title: "HTTP 协议处理器",
    subtitle: "HTTP / SSE / WS / gRPC",
    owner: "entrypoint::protocol / proxy",
    source: "apps/gateway/src/entrypoint/protocol/mod.rs",
    description:
      "Backend 确定后进入对应协议处理器。HTTP、SSE、WebSocket、gRPC 是不同实现分支。请求体可重放且失败允许前响应回退时，才尝试重试/下一个目标；已经开始的流不能被另一模型接管。",
    features: [
      ["protocol-handlers", "普通 HTTP、SSE、WebSocket、gRPC 专用处理", "base"],
      ["upstream-tls", "上游 TLS 与私有 CA", "conditional"],
      ["retry-boundary", "可重放检查、前响应失败和受限重试", "base"],
      ["timeouts", "连接、首响应、流空闲与总时长限制", "base"],
    ],
  },
  {
    id: "upstreams",
    title: "服务 / 模型上游",
    subtitle: "配置声明的 Backend",
    owner: "外部服务或模型执行服务",
    source: "apps/gateway/src/service/load_balancer.rs",
    description:
      "上游负责真实服务或模型执行。Gateway 使用运行快照中已声明的目标地址、凭据和策略。主图的原生推理动画以此作为实际尝试终点，不把 Cloud、Box 或 Harness 画成一个虚构的统一执行器。",
    features: [
      ["backend-addresses", "配置中的服务地址和模型目标", "base"],
      ["upstream-identity", "目标级凭据与实际尝试归因", "conditional"],
    ],
  },
  {
    id: "response",
    title: "响应与流生命周期",
    subtitle: "响应策略 · 背压 · 取消",
    owner: "ResponseBody / protocol handlers",
    source: "apps/gateway/src/entrypoint/protocol/streaming_handler.rs",
    description:
      "HTTP 分支运行响应阶段策略，并在对应路径观察 JSON / SSE 用量、执行所需回译。响应体保持背压、取消与时长边界；推理许可和用量生命周期覆盖整段 Body，结束或断开后再核对释放。TCP / UDP 不运行这条 HTTP 响应管线。",
    features: [
      ["response-policy", "响应阶段中间件与需要的协议回译", "base"],
      ["stream-lifecycle", "流背压、取消、Trailers 与时长边界", "base"],
      ["usage-observation", "真实上游用量与 Token 预算核对", "conditional"],
      ["permit-release", "Body 完成/取消时释放相应准入资源", "base"],
    ],
  },
  {
    id: "usage",
    title: "用量与观测",
    subtitle: "Request / Attempt · Usage Spool",
    owner: "usage / observability",
    source: "apps/gateway/src/usage/mod.rs",
    description:
      "本例第 6 步先开始 request / attempt 耐久记录，失败即拒绝，不调用上游。第 8 步随响应体观察用量，Body 完成或取消后核对。失败尝试保留归因。Cloud 上报、ACK、回收在后台按配置执行，不是响应后的同步关卡；日志、Metrics、TraceContext 各自按配置启用。",
    features: [
      ["durable-usage", "请求/尝试生命周期、完整性与崩溃恢复", "conditional"],
      ["spool-replay", "顺序重放、ACK、回收与压缩", "conditional"],
      ["telemetry", "访问日志、Metrics、TraceContext", "conditional"],
      ["cloud-usage", "Bearer / mTLS 上报至 Cloud 用量入口", "conditional"],
    ],
  },
  {
    id: "configuration",
    title: "配置来源",
    subtitle: "Standalone / Managed / Embedded",
    owner: "GatewayConfig / provider",
    source: "apps/gateway/src/gateway/startup.rs",
    description:
      "Standalone 读取 ACL 和声明的 Providers。Cloud-managed 接受完整受约束快照；Studio 嵌入由宿主提供配置与供应商键。动态来源先合成并验证候选，不在每个业务请求中同步访问控制平面。",
    features: [
      ["acl-config", "ACL 加载、验证和 File Watch", "base"],
      ["providers", "Discovery / Docker / Kubernetes 来源", "conditional"],
      ["operating-modes", "Standalone、Cloud-managed 与嵌入宿主权威", "base"],
    ],
  },
  {
    id: "compile",
    title: "构建运行时",
    subtitle: "build_runtime · 先验证后发布",
    owner: "Gateway / gateway::builders",
    source: "apps/gateway/src/gateway.rs",
    description:
      "build_runtime 编译 RouterTable、TCP SNI 表、Middleware Pipeline、RoutePlan、ServiceRegistry、InferenceAuthorizer 与相关运行组件。候选构建、校验和必要激活检查通过后才替换运行快照，失败保留上一有效状态。",
    features: [
      ["compiled-routing", "RouterTable / TCP routes / RoutePlan", "base"],
      ["bound-pipelines", "编译管线、服务和推理策略绑定", "base"],
      ["transactional-reload", "构建与激活成功后提交，拒绝不覆盖旧状态", "base"],
    ],
  },
  {
    id: "snapshot",
    title: "运行快照",
    subtitle: "ArcSwap<GatewayState>",
    owner: "GatewayRuntime::load / replace",
    source: "apps/gateway/src/entrypoint.rs",
    description:
      "请求从 GatewayRuntime::load 取得 Arc<GatewayState>。replace 在成功重载后切换 ArcSwap，并处理退役目标；已接纳请求持有自己的快照。它是本地状态引用，不是请求链中的远程服务。",
    features: [
      ["snapshot-read", "请求开始读取本地已提交快照", "base"],
      ["snapshot-replace", "原子替换与旧请求继续完成", "base"],
      ["drain-generations", "目标代际退役与排空协调", "conditional"],
    ],
  },
  {
    id: "management",
    title: "Node API 管理面",
    subtitle: "独立监听器 · 不截获业务流量",
    owner: "node_api",
    source: "apps/gateway/src/node_api.rs",
    description:
      "Node API 使用独立管理监听器提供有界机器契约。它不是第二个业务网关入口，也不承担用户部署控制台。管理认证、IP、TLS 与快照 Apply 规则在自己的边界执行。",
    features: [
      ["node-ops", "Health / Readiness / Metrics / Version", "conditional"],
      ["snapshot-apply", "完整托管快照 Apply 与状态", "conditional"],
      ["management-security", "管理令牌、IP 与 TLS/mTLS 约束", "conditional"],
    ],
  },
  {
    id: "local-ingress",
    title: "前置本地处理",
    subtitle: "ACME / Anonymous Chatbot",
    owner: "HTTP route match 之前的专用分支",
    source: "apps/gateway/src/entrypoint.rs",
    description:
      "HTTP-01 challenge 使用运行时 ChallengeStore 在普通路由之前处理。Anonymous Chatbot 也先拥有自己的路径；没有相应配置时拒绝，不能回落到 catch-all。它们是同一监听器内的分支，不是新网关入口。",
    features: [
      ["acme", "ACME HTTP-01 ChallengeStore", "conditional"],
      ["anonymous-chatbot", "专属匿名 Chatbot 路径及缺失时拒绝", "conditional"],
    ],
  },
  {
    id: "local-response",
    title: "本地响应分支",
    subtitle: "Static / Models / Platform",
    owner: "static_object / inference platform handlers",
    source: "apps/gateway/src/entrypoint.rs",
    description:
      "中间件后的部分路由在本地响应，不经过普通上游代理：静态 Bundle、授权模型目录以及 OCR / SystemOne 等平台处理。各分支有自己的准入契约，不把所有 Body 都变成聊天请求。",
    features: [
      ["static-bundles", "Digest / Manifest、SPA、Range、ETag 静态对象", "conditional"],
      ["model-listing", "按 Grant 返回授权模型目录", "conditional"],
      ["platform-handlers", "OCR、SystemOne、Rerank / 平台别名处理", "conditional"],
      ["specialized-tools", "按既有路由绑定的搜索/解析等专用服务", "conditional"],
    ],
  },
  {
    id: "demand",
    title: "激活与唤醒需求",
    subtitle: "Parking / Function / Agent Wake",
    owner: "hosted_function_activation / agent_wake",
    source: "apps/gateway/src/entrypoint.rs",
    description:
      "路由可记录 Function 激活或 Agent 唤醒需求。声明 Parking 的可持有请求有界等待新快照，不能持有时返回受限 Retry-After。匹配 Agent billing 绑定时还会执行准入拒绝或幂等调用分支；这些是执行服务契约，不是规划任务图或选择 Harness。",
    features: [
      ["parking", "可持有请求有界停放与新快照重匹配", "conditional"],
      ["function-demand", "Hosted Function 激活需求", "conditional"],
      ["agent-wake", "Agent 空闲策略、需求去重与有界发出", "conditional"],
      ["agent-binding", "Agent 执行绑定的准入拒绝与幂等调用", "conditional"],
    ],
  },
  {
    id: "transport",
    title: "TCP / UDP 分支",
    subtitle: "连接路由与协议中继",
    owner: "TCP listener / udp_listener",
    source: "apps/gateway/src/entrypoint/listener.rs",
    description:
      "TCP 用 HostSNI 或入口绑定规则确定服务，执行连接准入和服务 Backend 选择后 relay。UDP 使用自己的监听器和会话处理。它们共享已提交运行时，但不走 HTTP Body、HTTP 中间件或模型 JSON 准入。",
    features: [
      ["tcp-sni", "TCP IP/连接限制与 HostSNI", "conditional"],
      ["raw-service-selection", "使用运行时服务与健康选择 Backend", "base"],
      ["transport-relay", "TCP relay 与 UDP 独立中继实现", "base"],
    ],
  },
  {
    id: "model-control",
    title: "Studio ModelControl",
    subtitle: "仅 Studio 编译的本地模型路由",
    owner: "Studio 宿主 · 不是 Gateway 内置模块",
    source: "apps/studio/src-tauri/src/local_api_gateway_control.rs",
    description:
      "connect_model_control 把本地模型别名编译到宿主私有 HTTP 服务，转交客户端身份。外层目标指向 ModelControl，真实候选、重试和结算归现有宿主模型链；源码显式设定外层 Failover 默认值，避免再次重试。此路径与原生直连供应商部署不能混成双重业务链。",
    features: [
      ["embedded-gateway", "Gateway::new_embedded 接收宿主快照与键", "separate"],
      ["host-model-services", "别名 → 私有 ModelControl service 地址", "separate"],
      ["single-retry-owner", "实际模型候选、重试、计量由 ModelControl 负责", "separate"],
    ],
  },
  {
    id: "agent-cli",
    title: "Agent / Skill CLI",
    subtitle: "独立命令，非数据面路由",
    owner: "coding_agent_cli / agent",
    source: "apps/gateway/src/agent/mod.rs",
    description:
      "源码明确把此模块与网关数据面分离。main 识别 agent / skill 子命令后直接执行并返回，不启动网关监听。它提供原生 CLI Profile 与只读 Skill 发现，不是远程 Harness 路由或 Lead/Worker 编排 API。",
    features: [
      ["agent-profiles", "AgentRegistry 与原生命令契约", "separate"],
      ["skill-discovery", "SKILL.md 只读发现与命令构建", "separate"],
      ["no-harness-router", "Gateway 当前没有统一自动 Harness 选择与编排入口", "absent"],
    ],
  },
  {
    id: "native-cli",
    title: "原生 Agent 子进程",
    subtitle: "AgentRuntime::execute",
    owner: "本地进程操作",
    source: "apps/gateway/src/agent/runtime.rs",
    description:
      "AgentRuntime 用 tokio::process::Command 传入程序、参数、工作目录，继承终端流并等待退出。没有 shell 求值，也没有在此建立 Gateway HTTP 任务会话、检查点或子任务调度。",
    features: [
      ["native-execution", "程序 / 参数 / 工作目录显式传递", "separate"],
      ["terminal-inheritance", "继承 stdin/stdout/stderr，等待退出状态", "separate"],
    ],
  },
  {
    id: "cloud-usage",
    title: "Cloud 用量接收",
    subtitle: "后台投递 · 非同步请求跳转",
    owner: "Cloud / UsageCloudUploader",
    source: "apps/gateway/src/gateway/startup.rs",
    description:
      "启用用量上报时，Gateway 后台 uploader 从耐久 Spool 投递已记录批次并处理 ACK。Cloud 的长期账本不是每个请求执行一次的在线中间件。",
    features: [
      ["usage-uploader", "后台批次上传与接收确认", "conditional"],
      ["ledger-owner", "长期账本属于 Cloud", "separate"],
    ],
  },
];
