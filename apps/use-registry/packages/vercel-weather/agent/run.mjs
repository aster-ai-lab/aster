import http from 'node:http'

import { streamText, tool } from 'ai'
import { createOpenAICompatible } from '@ai-sdk/openai-compatible'
import { z } from 'zod'

import { readCall } from './call.mjs'
import { currentWeather } from './weather.mjs'

if (process.env.ASTER_AGENT_PROTOCOL === '1') {
  const events = []
  const server = http.createServer(async (request, response) => {
    const chunks = []
    for await (const chunk of request) chunks.push(chunk)
    let body
    try {
      body = JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}')
    } catch {
      response.writeHead(400).end()
      return
    }
    response.setHeader('content-type', 'application/json')
    if (request.url === '/v1/agent/commands') {
      const city = typeof body.parameters?.city === 'string' ? body.parameters.city : ''
      let text
      try {
        text = await currentWeather(city)
      } catch (error) {
        text = error instanceof Error ? error.message : '天气查询失败'
      }
      events.push({ sequence: 1, type: 'output', text })
      events.push({ sequence: 2, type: 'completed' })
      response.end(JSON.stringify({ accepted: true }))
      return
    }
    if (request.url === '/v1/agent/events:page') {
      const after = Number(body.after) || 0
      response.end(JSON.stringify({ events: events.filter((event) => event.sequence > after) }))
      return
    }
    response.writeHead(404).end()
  })
  server.listen(0, '127.0.0.1', () => {
    const address = server.address()
    const port = address && typeof address === 'object' ? address.port : 0
    process.stdout.write(`port ${port}\n`)
  })
} else {

const weather = tool({
  description: '按城市名读取 Open-Meteo 的当前气温。',
  inputSchema: z.object({
    city: z.string().min(1).max(80)
  }),
  execute: async ({ city }) => currentWeather(city)
})

const chunks = []
for await (const chunk of process.stdin) chunks.push(chunk)
const call = readCall(Buffer.concat(chunks).toString('utf8'))

const baseURL = process.env.ASTER_MODEL_BASE_URL?.trim()
if (!baseURL) {
  const text = await weather.execute({ city: call.city }, { toolCallId: 'turn', messages: [] })
  process.stdout.write(String(text))
} else {
  const model = createOpenAICompatible({
    name: 'aster',
    baseURL,
    apiKey: process.env.ASTER_MODEL_API_KEY ?? ''
  }).chatModel(process.env.ASTER_MODEL_ID?.trim() || 'default')
  const result = streamText({
    model,
    tools: { current_weather: weather },
    prompt: call.text
  })
  for await (const part of result.textStream) process.stdout.write(part)
}
}
