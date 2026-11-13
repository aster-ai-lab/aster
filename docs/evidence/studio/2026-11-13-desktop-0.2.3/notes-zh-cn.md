## 新功能

- 无。本次为维护版本。

## 改进

- 每周起始日的解析现在同时兼容最终规范的 `Intl.Locale.prototype.getWeekInfo()` 方法与当前 V8 运行时提供的 `weekInfo` 属性；两者都不可用时按周日回退，并已在代码中注明依据。

## 错误修复

- 修复在仅提供 `weekInfo` 属性、不提供 `getWeekInfo()` 方法的运行时（已在 Node 22 / jsdom 上确认）中，用量热力图与分布图抛出 `locale.getWeekInfo is not a function` 的问题。此前因此失败的 8 条 renderer 断言现已通过。
