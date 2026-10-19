# ACP 云端智能体

这个包是 Console 运营侧「智能体」页的案例。`agent/acp-agent.mjs` 在标准输入和标准输出上实现 ACP `protocolVersion` 1。

安装并使用：

```sh
node client/install-and-use.mjs
```

客户端会启动智能体，建立同一个 `sessionId`，连续发送两轮 `session/prompt`，并在每轮 `stopReason` 之前打印流式 `session/update`。
