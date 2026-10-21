# AsterDream

桌面应用的默认租户是 `aster`，使用星标与蓝银主题。`legacy` 租户保留通友产品品牌。
两者共用本仓库的 Rust 宿主、ACP、Office 和前端，应用标识、数据目录与更新资格分别核验。

```sh
cd apps/desktop
pnpm install --frozen-lockfile
cd ../..
just build-desktop
just dev
# 通友租户
just dev
```

`just dev` 和 `just work` 也使用默认租户。开发端口为 `4318`。
macOS 打包：`just build-desktop-release`；显式租户：
`A3S_TENANT_SLUG=legacy just build-desktop-release`。

新默认租户使用独立数据目录。本次功能和品牌合并不导入旧版的会话、项目、偏好或凭据。
原有许可证保留在 [LICENSE](LICENSE)，上游说明保留为
[历史来源](../../docs/provenance/desktop-upstream-readme.md)。
