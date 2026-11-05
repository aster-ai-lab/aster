# Tasks: 记忆融合保留原始相关度

- [x] T001 固定原输入/model/grader/source identity、primary linear 等权策略；先完成 read-only analyze。见 `baseline-identity.json`、`ANALYZE.md`。
- [x] T002 在唯一 fusion owner 接入 native score；扩展 typed policy/binding/profile/exports/context，保留旧字节与来源/准入。无另一套累计／索引／repository。
- [x] T003 sibling tests 覆盖新排序、零分/重复、非法边界、旧序列化、profile mismatch/generation identity；与 T002 同批。kernel 6 项通过；新增 memory 5 项通过，完整 memory target 的既有 context ID 金标失败保留。
- [x] T004 原 eval/probe 和原 frozen 10 memory owner queries 实际执行，复用原 metrics；保留所有失败，不改 gold。原 versioned eval 1 PASS；新 profile 反例 PASS，旧 RRF 严格 Top-1 FAIL 原样保留；10-query 两 profile 均过原 gates，各 query 质量指标相同，不宣称整体提升。见 `native-memory/`、`memory-profile-comparison.json`。
- [x] T005 scoped fmt/diff、owning Clippy、minimal、paired build，复开调用者与产物；回写 033/OUTCOME。两轮 Clippy／paired build PASS，最终 build ID 一致；最后配套二进制回放 8 PASS／1 已知 RRF FAIL。独立保留 shared-checkout 导出编译错误及一行修正、context ID FAIL、root fmt 781 个非本次路径与未测平台。见 `OUTCOME.md`、`delivery-manifest.json`。

## Final verification gate

- [x] T001–T005 有证据；旧 RRF compatibility 与新策略不混报。
- [x] 无新增重复 ranker、配置语言、依赖、primary mirror、权限扩大或发布。
- [x] 未完成 owner/整体质量/性能/平台如实标注，完整目标保持 active。
