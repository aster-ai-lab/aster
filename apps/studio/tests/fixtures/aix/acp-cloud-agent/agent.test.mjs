import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createInterface } from 'node:readline'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

function start() {
  const child = spawn(process.execPath, [fileURLToPath(new URL('./agent.mjs', import.meta.url))], {
    stdio: ['pipe', 'pipe', 'inherit']
  })
  const lines = createInterface({ input: child.stdout })
  const queue = []
  const waiters = []
  lines.on('line', (line) => {
    const message = JSON.parse(line)
    const waiter = waiters.shift()
    if (waiter) waiter(message)
    else queue.push(message)
  })
  const next = () =>
    queue.length > 0 ? Promise.resolve(queue.shift()) : new Promise((resolve) => waiters.push(resolve))
  let sequence = 0
  return {
    async request(method, params) {
      const id = ++sequence
      const updates = []
      child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`)
      for (;;) {
        const message = await next()
        if (message.id === id) return { response: message, updates }
        if (message.method === 'session/update') updates.push(message)
      }
    },
    close() {
      child.stdin.end()
      child.kill()
    }
  }
}

test('ACP cloud agent streams two turns on one session', async () => {
  const client = start()
  try {
    const initialized = await client.request('initialize', { protocolVersion: 1 })
    assert.equal(initialized.response.result.protocolVersion, 1)
    assert.equal(initialized.response.result.agentCapabilities.loadSession, true)
    assert.deepEqual(initialized.response.result.authMethods, [])

    const opened = await client.request('session/new', { cwd: '/workspace', mcpServers: [] })
    const sessionId = opened.response.result.sessionId
    assert.equal(typeof sessionId, 'string')

    const first = await client.request('session/prompt', {
      sessionId,
      prompt: [{ type: 'text', text: '第一句' }]
    })
    assert.equal(first.response.result.stopReason, 'end_turn')
    assert.ok(first.updates.length > 0)
    assert.equal(first.updates.map((update) => update.params.update.content.text).join(''), '收到：第一句')
    assert.ok(first.updates.every((update) => update.id === undefined))

    const second = await client.request('session/prompt', {
      sessionId,
      prompt: [{ type: 'text', text: '第二句' }]
    })
    assert.equal(second.response.result.stopReason, 'end_turn')
    assert.equal(
      second.updates.map((update) => update.params.update.content.text).join(''),
      '上一轮是第一句。这一轮是第二句'
    )

    const loaded = await client.request('session/load', { sessionId, cwd: '/workspace', mcpServers: [] })
    assert.equal(loaded.response.result.sessionId, sessionId)
    assert.match(loaded.updates.map((update) => update.params.update.content.text).join(''), /第一句/)
  } finally {
    client.close()
  }
})
