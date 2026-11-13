Aster v0.2.3 · macOS Apple Silicon (arm64)

**Installation**: this DMG is signed with a Developer ID Application certificate, notarized by Apple and stapled. Gatekeeper accepts both the disk image and the app directly — no "System Settings → Privacy & Security → Open Anyway" override is needed.

> ⚠️ 发布前置：此段安装说明只在 `spctl --assess --type open --context context:primary-signature` 返回 `accepted / Notarized Developer ID` 之后才成立。公证未完成时**禁止**使用本文案创建 release。

## New Features

- None. This is a maintenance release.

## Improvements

- Locale week-start resolution now accepts both the finalized `Intl.Locale.prototype.getWeekInfo()` method and the `weekInfo` getter shipped by current V8 runtimes, with a documented Sunday fallback when neither is available.

## Bug Fixes

- Fixed the usage heatmap and distribution chart throwing `locale.getWeekInfo is not a function` on runtimes that expose `weekInfo` as a getter instead of the finalized method (verified on Node 22 / jsdom). The eight renderer assertions that failed for this reason now pass.

---

**安装说明**：本 DMG 已使用 Developer ID 签名、通过 Apple 公证并装订票据（stapled），macOS 可直接打开，无需到「系统设置 → 隐私与安全性」手动放行。仅支持 Apple Silicon (arm64)。

## 新功能

- 无。本次为维护版本。

## 改进

- 每周起始日的解析现在同时兼容最终规范的 `Intl.Locale.prototype.getWeekInfo()` 方法与当前 V8 运行时提供的 `weekInfo` 属性；两者都不可用时按周日回退，并已在代码中注明依据。

## 错误修复

- 修复在仅提供 `weekInfo` 属性、不提供 `getWeekInfo()` 方法的运行时（已在 Node 22 / jsdom 上确认）中，用量热力图与分布图抛出 `locale.getWeekInfo is not a function` 的问题。此前因此失败的 8 条 renderer 断言现已通过。
