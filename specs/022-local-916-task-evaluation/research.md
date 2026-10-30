# Research: 复用依据与当前缺口

- Core 和 ACP manifest 已是 9.1.6，Studio 已有本地 Core 路径依赖；先验证，不增加平行适配层。
- `just build-studio` 同一 feature graph 构建 host/ACP/Office，`studio_product.py` 按内容暂存 companion，`launcher.rs` 的显式路径失败不回退。
- `just fork aster` 不停止主实例。`tenant_profile.rs` 给 Aster side instance 分配独立 profile 与 bundle 身份；不导入现有会话。
- `cua_guard.py` 已拒绝指针、坐标、前台投递、addressed hotkey；`SettingsDrive` 还明确拒绝截图和目录驱动的坐标例外。沿用这些保护，不增加新的 OS 输入方式。
- 现有 catalog 有 25 项，含 CAT-MAIL/COPY/CONFLICT；T01 和另两个代码任务不在该清单。本期验收集合是 28 项。
- A06/A12 的扫描 PDF 仍标为阻塞，另有七项未冻结评分器；不能直接把已有 runner 的成功状态当作本期完成。
- `common.prepare_job` 使用精确 allow/deny；调用时显式给出仓库内 target 作业目录，避免默认系统临时目录，不复制答案、测试或解法。
- `grade.py` 的既有片段检查可作必要条件，不能证明整行归属和数字关系正确。补独立关系/结构反例并人工核对语义结果，不扩大关键词列表制造通过。
- `model_route.py` 是旧 ToYou/gateway 路线，并手工读取 ACL。Aster 本期不使用该路线，直接消费可信宿主 provider 配置。
- 任务原件只读。Git 任务只在仓库内的准备夹具执行；原 Aster 仓库和同级 tasks 目录均不是其清理目标。
- 文档发布已完成：内容提交 `e011dc3ab`，验证提交 `0193fda51`。本期不复用发布分支的干净源码来冒充用户要求的最新本地 dirty 源码。
