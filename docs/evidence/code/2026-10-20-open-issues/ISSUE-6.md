# Issue 6：内部依赖本地解析与工作区守卫

Gateway 的 ACL、可选 Sentry、Updater 依赖改用已有本地 crate，保留版本约束、optional 和公开 API。Gateway、Sentry、Updater 同时设为 `publish=false`；实际 161 个工作区成员均不可发布。锁文件只去掉三个 registry A3S 副本及消除歧义所需的引用标签，没有更新第三方版本。

已保存修改前 Cargo.lock 的 hash 和其他线程的完整差异：[before](issue-6-before.json)、[原有锁差异](issue-6-cargo-lock-before.patch)。离线解析后，既有 aster 的六项新增依赖全部保留；对根 HEAD 的结构化 TOML 比较证明除此之外只有预期的三包去重变化：[锁释放收据](issue-6-lock-ready.json)、[语义保留核对](issue-6-lock-semantic-preservation.json)。

Work 的 `.git` 是合法 gitlink 对应的 gitfile，不能表示第二个 Cargo 工作区。旧断言改为从 Work host 的实际 manifest 查询 Cargo metadata：核对根 workspace 路径、准确 host manifest/name、成员 id、无独立 `[workspace]` 及无 Work 根/host 的额外锁文件。没有检查或删除用户 Git 元数据，原本地来源、publish 和锁文件 ratchet 原样保留。

## 验证

- 修改前两个定向守卫均失败：旧 `.git` 断言，以及首个 `publish=null`。另记录真实 metadata 的三条 registry 依赖及三个不可发布设置缺项。[失败日志一](issue-6-before-guard-1.log)、[失败日志二](issue-6-before-guard-2.log)
- `test_internal_dependencies_are_local_and_unpublishable` 修改后通过，覆盖所有成员、包括 optional 的 A3S 依赖和锁中所有 A3S 来源。[日志](issue-6-local-guard-after.log)
- `cargo check --locked --offline -p a3s-gateway --features wire` 退出 0，实际编译检查本地 ACL、Updater 和可选 Sentry。耗时 151.58 秒。[命令及结果](issue-6-gateway-wire-check.json)、[日志](issue-6-gateway-wire-check.log)
- 全量 Python **5/6 通过**；唯一失败为缺失 `apps/web/public/tenant/toyou/INFO.md`。[完整结果](issue-6-python-after.log)
- `cargo test --locked --offline -p aster --test workspace workspace_contains_local_rust_harness_and_both_product_binaries -- --nocapture` 退出 0，工作区本地来源守卫通过。耗时 361.43 秒，包含共享目标队列和 CLI 编译。[命令及结果](issue-6-rust-workspace-after.json)、[日志](issue-6-rust-workspace-after.log)

## 已有迁移矛盾与未声明范围

提交 `e07fd7c69307e33239053c7bb3c5cdb64d78b80d` 明确记录 “The science, web, and official content trees leave this checkout.”，并有意移除 Web 树。现有 Studio tenant loader 和 fixture 检查仍引用其中品牌资产，导致上述独立已有失败。没有删除该真实契约、造占位资产或恢复已经移出的 Web 树；本轮不扩展 Studio/tenant 迁移。

不声称整个 Python suite 通过。没有更改版本、服务部署、已发布 R4 包、标签或更新 feed；没有提交或推送。
