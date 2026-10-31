# 天气助手

可安装的 AIX 智能体。`harness = "package"`，`runtime = "cloud"`。一次调用启动 `agent/run.mjs`，读完标准输入后退出。

安装文件是 `apps/use-registry/dist/vercel-weather-1.0.0.aix`。这个目录是它的源。

标准输入是一个 JSON 对象：

```json
{"text":"帮我看一下","parameters":{"city":"上海"}}
```

`parameters.schema.json` 要求 `city`。进程用 Vercel AI SDK 的 `tool` 向 Open-Meteo 查当前气温。设置了 `ASTER_MODEL_BASE_URL` 时，再用 `streamText` 把工具结果流到标准输出。没有这个地址时，直接把气温写到标准输出，然后退出。

客机里安装依赖：

```bash
cd apps/use-registry/packages/vercel-weather/agent
npm install
```
