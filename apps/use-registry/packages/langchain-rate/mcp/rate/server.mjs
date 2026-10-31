import http from 'node:http'

import { tool } from '@langchain/core/tools'
import { z } from 'zod'

const usdCny = tool(
  async () => {
    const response = await fetch('https://api.frankfurter.app/latest?from=USD&to=CNY')
    if (!response.ok) {
      throw new Error(`frankfurter ${response.status}`)
    }
    const body = await response.json()
    const rate = body?.rates?.CNY
    if (typeof rate !== 'number') {
      throw new Error('frankfurter 没有返回 CNY')
    }
    return `1 USD = ${rate} CNY（${body.date}）`
  },
  {
    name: 'usd_cny',
    description: '读取法兰克福公开汇率，返回 1 美元兑人民币。没有参数。',
    schema: z.object({})
  }
)

const tools = [
  {
    name: 'usd_cny',
    description: '读取法兰克福公开汇率，返回 1 美元兑人民币。没有参数。',
    inputSchema: { type: 'object', properties: {} }
  }
]

const server = http.createServer(async (request, response) => {
  if (request.url !== '/mcp') {
    response.writeHead(404).end()
    return
  }
  if (request.method === 'GET') {
    response.writeHead(405).end()
    return
  }
  if (request.method !== 'POST') {
    response.writeHead(405).end()
    return
  }
  const chunks = []
  for await (const chunk of request) chunks.push(chunk)
  let message
  try {
    message = JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}')
  } catch {
    response.writeHead(400).end()
    return
  }
  if (message.id === undefined) {
    response.writeHead(202).end()
    return
  }
  const reply = await dispatch(message)
  response.writeHead(200, { 'content-type': 'application/json' })
  response.end(JSON.stringify(reply))
})

async function dispatch(message) {
  if (message.method === 'initialize') {
    return ok(message.id, {
      protocolVersion: message.params?.protocolVersion ?? '2024-11-05',
      capabilities: { tools: { listChanged: false } },
      serverInfo: { name: 'langchain-rate', version: '1.0.0' }
    })
  }
  if (message.method === 'tools/list') {
    return ok(message.id, { tools })
  }
  if (message.method === 'tools/call' && message.params?.name === 'usd_cny') {
    try {
      const text = await usdCny.invoke({})
      return ok(message.id, { content: [{ type: 'text', text: String(text) }], isError: false })
    } catch (error) {
      return ok(message.id, {
        content: [{ type: 'text', text: error instanceof Error ? error.message : 'rate failed' }],
        isError: true
      })
    }
  }
  return { jsonrpc: '2.0', id: message.id, error: { code: -32601, message: 'method not found' } }
}

function ok(id, result) {
  return { jsonrpc: '2.0', id, result }
}

if (process.env.ASTER_AGENT_PROTOCOL === '1') {
  const events = []
  const agent = http.createServer(async (request, response) => {
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
      let text
      try {
        text = String(await usdCny.invoke({}))
      } catch (error) {
        text = error instanceof Error ? error.message : '汇率查询失败'
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
  agent.listen(0, '127.0.0.1', () => {
    const address = agent.address()
    const port = address && typeof address === 'object' ? address.port : 0
    process.stdout.write(`port ${port}\n`)
  })
} else {
  server.listen(8761, '127.0.0.1')
}
