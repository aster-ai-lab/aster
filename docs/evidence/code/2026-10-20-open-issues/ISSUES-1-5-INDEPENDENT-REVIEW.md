# Issues 1、5 补充独立只读审查

范围限当前 diff 的 slice 进展归属、停滞/终态边界及 lazy memory 共享生命周期；没有编辑产品、启动应用或新增 Cargo 运行。

## #1：当前进展路径与历史边界

`goal_slice_made_progress` 从指定 SliceAdmitted 到下一条 SliceAdmitted 截取原始事实窗口；receipt fold 验证 prepared/result 配对和 background child/terminal 归属。helper 再检查每项 receipt 的 run/session 与一致 capability digest，未知、缺失、未结算及 incomplete 观察不能重置 stall。只认非 harness 路径的真实 kind、bytes 或 mode 身份变化；同字节 write、只读成功和 plan 文字不是新进展。

`fold_settle` 先验证原 slice、goal/revision/workspace、closeout 引用、命令 digest 及效果已结算，再使用该进展投影。reset 只影响停滞计数，不会把 rejected goal judgment 变成 Completed；达到无进展限额走既有 NeedsInput/失败结果出口。生产工具 preparation 的 admission 来自 Run/checkpoint 的冻结 capability binding；新 helper 没有另造一次工具授权。

已核对最终 typed 语义与双计数修复，没有未决源码发现。`ContinuationSource.progress_basis` 为私有 `Option<closed enum>`，缺字段为 legacy None，且 default + skip_serializing_if 使重序列化继续省略该字段，保留旧 RunBody/command digest 形状；未知语义版本拒绝。唯一生产 source 构造点 `goal_tick_source` 调用 `observed_mutations`，新记录显式写 observed_workspace_mutation_v1，没有让公开 ACL 或命令输入选择 legacy。

两计数均随每次有效 admission 递增，但 StepAccepted 的原 `!ledger.is_empty()` 含义只 reset legacy counter；SliceSettled 的完整已知身份变化只 reset actual counter。admit 按记录的 basis 选择对应计数，原 clock/deadline/总数/stall/replan 检查没有跳过。新机会只读 actual counter，旧 accepted-step reset 无法污染严格预算；旧记录仍按其原 legacy 语义验证。相同历史 tick 先由原身份匹配得到 Reconcile，不会以新 source 重执行。

已读维护回归及 [修复前失败日志](issue-1-replay-before.log)：真实首个 legacy-shaped Run 完成同字节 write 并 accepted，随后“历史上合法的下一条 admission”在原修复中被 Waiting 拒绝。最终回归同时覆盖 memory/file session store、原事实不改写、恢复及新严格机会 NeedsInput；第二条是 protocol-history admission fixture，不宣称它在旧发布二进制中执行或已运行第二个 Run。after 运行结论由实现代理/根代理收据记录，本审查未运行 Cargo。

## #5：共享与最后释放

当前 canonical path→Weak registry 保持首次 open/import 串行，成功 Arc 仅由各 lazy OnceCell 持有，外部调用只得到借用。初始化 closure 本身无 await，失败不进入 OnceCell，故可在 direct-owner 释放后重试。`LazyFileMemoryStore::Drop` 先 take 出 owned Arc，随后在同一 registry mutex 下 drop，包含最后一次 engine/lease 析构；upgrade/open 与最后释放因此不能交错。已知 strong=0、lease 尚未释放的窗口从机制上闭合。

全局 registry 不保活 collection，最后 lazy 释放后可重新打开；直接 VecMemoryStore 的独占打开、存储格式、legacy import 错误保留和跨进程锁路径均未改变。没有发现额外确定生命周期缺口。新增 16 轮 drop/open 并发用例是有界竞争测试，不是强制暂停在旧析构窗口的确定性注入；本审查未运行该用例，不将它冒称修复前失败证据。

最终源码身份已与实现代理 source-ready 收据的 11 个生产文件逐项 SHA-256 核对一致；附加调用/回归文件及具体检查见 [独立收据](issue-1-5-independent-review.json)。以上是有界源码审查，不替代实现代理的旧事实兼容回归、同进程多 ACP session 测试或根代理的最终同批验收。

最终规范调整再次只读核对：ProgressBasis 改用已有 `closed_set!` 宏，wire literal 仍为 `observed_workspace_mutation_v1`，补齐统一 parse/as_str 契约；Option 缺省/省略及双计数逻辑保持不变。仅该文件 SHA 更新，其余 10 个生产文件与上一独审一致，11 项均匹配 source-ready 新收据。没有额外运行 Cargo。
