import { readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

import { expect, test } from '@playwright/test'

import { nativeHarness } from './nativeHarness'

/**
 * F14 partial (PLAN.md section 4): after the real native host process exits
 * and relaunches, agent-plane business data must still be readable and backed
 * by on-disk files. This exercises a real process restart -- a renderer-level
 * reload or an in-memory fixture would not satisfy it.
 */
let host: Awaited<ReturnType<typeof nativeHarness>>
test.beforeAll(async () => {
  test.setTimeout(240000)
  host = await nativeHarness()
})
test.afterAll(async () => {
  try {
    await host?.close()
  } catch (error) {
    console.warn(`restart-e2e cleanup skipped: ${String(error)}`)
  }
})

test('agent-plane entities survive a real native-host process restart', async () => {
  const sessionsFile = join(host.profile, 'agent-plane', 'sessions.json')
  const agentsFile = join(host.profile, 'agent-plane', 'agents.json')
  const usageDb = join(host.profile, 'model-control', 'usage.sqlite')

  const before = JSON.stringify(await host.api('GET', '/api/agent-sessions'))
  expect(before).toContain(host.session.id)
  expect(readFileSync(sessionsFile, 'utf8')).toContain(host.session.id)
  expect(statSync(usageDb).size).toBeGreaterThan(0)

  // Stops the proxy and the actual native host binary, then relaunches both.
  await host.restart()

  const after = JSON.stringify(await host.api('GET', '/api/agent-sessions'))
  expect(after).toContain(host.session.id)
  expect(readFileSync(sessionsFile, 'utf8')).toContain(host.session.id)
  expect(readFileSync(agentsFile, 'utf8').length).toBeGreaterThan(0)
  expect(statSync(usageDb).size).toBeGreaterThan(0)
})
