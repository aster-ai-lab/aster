# R4 构建后源码与回归收据复核

**结论：通过本次只读源码复核。** [完整收据](source-freeze-r4-postbuild-source-review.json) 核对原始树与隔离树 **33/33** 个文件均仍等于 R4 冻结 SHA256；R3 的 31 个文件未变，R13 仅增加两个路径。隔离 Work 的 25 个 tracked、6 个 untracked 路径精确符合允许清单；完整隔离补丁等于原树限定补丁，SHA256 为 `b62d9db899d7d0624e7daacaed5ab72e7ac3a81e44c9d67dda5622b879725e91`。

原始 Work 的额外 ACP `acp.rs`、`turns.rs`、`acp_reconciliation_tests.rs` 及根树其他线程的 Rust/Cargo 修改均未进入隔离树；其中未跟踪文件逐项确认不存在，已跟踪文件确认保持隔离基线。根/Work HEAD 与冻结基线一致。

已有 R1–R13 回归日志、源文件 SHA 和基线对照仍可使用：R1–R4 为八文件 384，通过；R11 为五文件 33，通过；R13 为五文件 58，通过。它们是各自范围，不相加。原先 guard 的 14 个 renderer 文件、R11 类型修正后的测试与 R13 两文件均和各自收据相符。安装器 7、Linux launcher 12、真实客户端协议 8，以及 R12 精确重复投递负向/双显示正向的既有记录已核对。

完整 Web TypeScript 仍为相同的 **278 条既有诊断、零新增**；workspace 仍为 **4 通过、2 既有失败**。没有把这两个门禁写成全面通过。

本次未重跑测试、未修改产品或测试、未访问应用/容器、未构建打包。此收据不替代最终安装包或 GUI 验收；最终 GUI、清理和 OUTCOME 结论由主代理补齐后另审。
