/** Transport probe only; it cannot certify independent review or artifact provenance. */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { homeTurnContext } from '../../../src/renderer/pages/agents/workbenchHomeCapabilities'

const [runtimeFile, destination] = process.argv.slice(2)
if (!runtimeFile || !destination)
  throw new Error('Usage: live-plane-probe.ts <existing-runtime-file> <evidence-directory>')
const runtime = JSON.parse(readFileSync(runtimeFile, 'utf8')) as { apiBaseUrl: string; token: string }
if (typeof runtime.token !== 'string' || !runtime.token || typeof runtime.apiBaseUrl !== 'string')
  throw new Error('Runtime descriptor is incomplete')

function loopback(value: string) {
  const url = new URL(value)
  if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || url.username || url.password)
    throw new Error('Runtime URL must be an authenticated loopback endpoint')
  return url.origin
}

interface ProbeMessage {
  role: string
  status: string
  data?: { parts?: { type: string; text?: string }[] }
}
const output = resolve(destination)
mkdirSync(output, { recursive: true })
const owned: string[] = []
const rows: Record<string, unknown>[] = []
let base = loopback(runtime.apiBaseUrl)

async function request<T>(path: string, method = 'GET', body?: unknown): Promise<T> {
  const response = await fetch(base + path, {
    method,
    signal: AbortSignal.timeout(15_000),
    headers: {
      authorization: `Bearer ${runtime.token}`,
      ...(body === undefined ? {} : { 'content-type': 'application/json' })
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) })
  })
  if (!response.ok) throw new Error(`${method} ${path}: HTTP ${response.status}`)
  return response.json()
}

async function main() {
  try {
    const plane = await request<{ baseUrl: string }>('/api/agent-plane')
    base = loopback(plane.baseUrl)
    const list = await request<{ items: { id: string; type: string; model?: string }[] }>('/api/agents')
    const agent = list.items?.find((row) => row.type === 'a3s-code' && typeof row.model === 'string')
    if (!agent) throw new Error('No configured a3s-code agent; no user configuration was changed')

    for (const [reviewMode, artifactTrace] of [
      [false, false],
      [true, false],
      [false, true],
      [true, true]
    ] as const) {
      const cell = `${Number(reviewMode)}${Number(artifactTrace)}`
      const marker = `COMPOSER_AUDIT_${Date.now()}_${cell}`
      const session = await request<{ id: string }>('/api/agent-sessions', 'POST', {
        agentId: agent.id,
        name: `Composer policy audit ${cell} ${Date.now()}`,
        workspace: { type: 'system' }
      })
      if (typeof session.id !== 'string') throw new Error('Session create did not return an identity')
      owned.push(session.id)
      const row: Record<string, unknown> = { cell, sessionId: session.id, reviewMode, artifactTrace }
      rows.push(row)
      const ack = await request<{ mode?: string }>(`/api/agent-sessions/${session.id}/turns`, 'POST', {
        userMessageParts: [
          {
            type: 'text',
            text: `This is an isolated product test. Do not use tools, inspect files, run commands, or change anything. Reply with exactly this marker: ${marker}`
          }
        ],
        turnContext: homeTurnContext({ reviewMode, artifactTrace, researchMode: false })
      })
      row.ackMode = ack.mode
      const deadline = Date.now() + 60_000
      let assistant: ProbeMessage | undefined
      while (Date.now() < deadline) {
        const messages = await request<{ items: ProbeMessage[] }>(`/api/agent-sessions/${session.id}/messages`)
        assistant = messages.items?.find((message) => message.role === 'assistant')
        if (assistant && assistant.status !== 'pending' && assistant.status !== 'streaming') break
        await new Promise((resolve) => setTimeout(resolve, 500))
      }
      row.assistantStatus = assistant?.status ?? 'missing'
      const parts = assistant?.data?.parts ?? []
      row.partTypes = parts.map((part) => part.type)
      row.markerObserved = parts.some((part) => part.type === 'text' && part.text?.includes(marker))
      // No files are requested; absence of a file receipt is not a provenance failure.
      row.status =
        assistant?.status !== 'success' || !row.markerObserved ? 'blocked-model-or-runtime' : 'passed-transport-only'
      process.stdout.write(`${cell} ${row.status} assistant=${row.assistantStatus}\n`)
      if (row.status === 'blocked-model-or-runtime') break
    }
  } catch (error) {
    rows.push({ status: 'infrastructure-error', error: String(error).replaceAll(runtime.token, '[redacted]') })
  } finally {
    if (owned.length) {
      for (const id of owned) await request(`/api/agent-sessions/${id}/turn/abort`, 'POST', {}).catch(() => undefined)
      try {
        await request('/api/agent-sessions/archive', 'POST', { sessionIds: owned })
        rows.push({ status: 'cleanup', archivedOwnSessions: owned.length })
      } catch (error) {
        rows.push({
          status: 'cleanup-error',
          error: String(error).replaceAll(runtime.token, '[redacted]'),
          ownedSessionIds: owned
        })
      }
    }
    writeFileSync(
      resolve(output, 'live-plane-results.json'),
      JSON.stringify(
        {
          boundary: 'HTTP transport probe only; no native UI, independent review or file-provenance certification',
          rows
        },
        null,
        2
      ) + '\n'
    )
  }
  process.exitCode = rows.some((row) => !['passed-transport-only', 'cleanup'].includes(String(row.status))) ? 1 : 0
}
void main()
