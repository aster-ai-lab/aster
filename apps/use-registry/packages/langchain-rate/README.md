# 汇率助手

操作系统插件目录里的智能体。启动时 API 会把它放进左侧「插件」。AsterDream 打开「智能体」，安装「汇率助手」。

工具进程要先开着。目录是本包的 `mcp/rate`。

```bash
cd apps/use-registry/packages/langchain-rate/mcp/rate
npm install
node server.mjs
```

进程听 `http://127.0.0.1:8761/mcp`。关掉之后，对话里的汇率工具会失败。
