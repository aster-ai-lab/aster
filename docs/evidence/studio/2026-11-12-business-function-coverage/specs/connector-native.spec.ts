import { readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { expect, test, type Page } from '@playwright/test'

import { nativeHarness } from './nativeHarness'

let host: Awaited<ReturnType<typeof nativeHarness>>
test.beforeAll(async () => {
  test.setTimeout(240000)
  host = await nativeHarness()
})
test.afterAll(async () => {
  // Cleanup best-effort: a sandbox bulk-delete guard may refuse to remove the
  // isolated profile. That must not turn a passed functional assertion into a
  // failed test; the leftover profile is reported in the evidence instead.
  try {
    await host?.close()
  } catch (error) {
    console.warn(`connector-e2e cleanup skipped: ${String(error)}`)
  }
})

async function dismissBrowserBootstrap(page: Page) {
  // The onboarding dialog's accessible name changes with its state and the
  // close button is only rendered once required components are prepared
  // (canDismiss). Use the stable data-ui hook and tolerate both absences.
  const dialog = page.locator('[data-ui="runtime-components.first-run"]')
  if (!(await dialog.isVisible().catch(() => false))) return
  const close = dialog.getByRole('button', { name: /关闭|继续|后台继续|组件已就绪/ })
  if (await close.count()) {
    await close.first().click({ timeout: 10000 })
    await expect(dialog).toBeHidden({ timeout: 10000 }).catch(() => {})
  }
}

test('real authority persists installations and real ACP invokes stdio, SSE and HTTP connectors', async ({
  page
}, info) => {
  const script = join(host.root, 'tests/e2e/capability_mcp.py')
  const log = join(host.stage, 'mcp-process.jsonl')
  const executable = host.python
  const catalogue = {
    id: 'e2e/fixture',
    chinese_name: '端到端安装连接器',
    description: 'Synthetic read-only connector',
    categories: ['developer-tools'],
    author: 'E2E',
    logo_url: ''
  }
  await page.route('**/api/modelscope/openapi/**', async (route) => {
    const url = new URL(route.request().url())
    if (url.pathname.endsWith('/mcp/servers'))
      return route.fulfill({
        json: { success: true, data: { mcp_server_list: [catalogue], total_count: 1, page_number: 1, page_size: 24 } }
      })
    if (url.pathname.endsWith('/mcp/servers/e2e/fixture'))
      return route.fulfill({
        json: {
          success: true,
          data: {
            ...catalogue,
            server_config: [
              { mcpServers: { fixture: { command: executable, args: [script], env: { ASTER_TEST_MCP_LOG: log } } } }
            ]
          }
        }
      })
    return route.fulfill({ status: 404, json: { error: 'unexpected_fixture_path' } })
  })
  await page.addInitScript(() => {
    Object.assign(window, {
      __ASTER_PREFERENCE_DEFAULTS__: {
        'app.language': 'zh-CN',
        'app.onboarding.permissions_dismissed': true,
        'app.onboarding.provider_setup.status': 'skipped',
        'app.privacy.policy_version': '20260820'
      }
    })
  })
  await page.goto('/windows/main/index.html')
  await dismissBrowserBootstrap(page)
  await page.locator('[data-ui="workbench.nav.connectors"]').click()
  await page.locator('[data-agentation-portal]').evaluateAll((nodes) => nodes.forEach((node) => node.remove()))
  const card = page.locator('[data-ui="capabilities.card"]').filter({ hasText: catalogue.chinese_name })
  await expect(card).toBeVisible()
  await card.getByRole('button', { name: '安装', exact: true }).click()
  await expect(card.getByRole('button', { name: '卸载', exact: true })).toBeVisible()
  const installed = await host.api('GET', '/api/capabilities/custom')
  expect(installed.connectors).toHaveLength(1)
  expect(installed.connectors[0]).toMatchObject({ command: executable, args: [script] })
  await page.reload()
  await dismissBrowserBootstrap(page)
  await page.locator('[data-ui="workbench.nav.connectors"]').click()
  await expect(card.getByRole('button', { name: '卸载', exact: true })).toBeVisible()
  await card.getByRole('button', { name: '卸载', exact: true }).click()
  await page.getByRole('button', { name: '卸载', exact: true }).last().click()
  await expect.poll(async () => (await host.api('GET', '/api/capabilities/custom')).connectors.length).toBe(0)

  async function configure(transport: 'stdio' | 'sse' | 'streamable_http') {
    await page.locator('[data-ui="capabilities.create"]').click()
    const dialog = page.getByRole('dialog', { name: '自定义连接器' })
    await dialog.getByRole('tab', { name: 'JSON', exact: true }).click()
    const spec =
      transport === 'stdio'
        ? { command: executable, args: [script], env: { ASTER_TEST_MCP_LOG: log } }
        : {
            type: transport === 'sse' ? 'sse' : 'http',
            url: host.mcp.origin + (transport === 'sse' ? '/sse' : '/mcp'),
            headers: { Authorization: 'Bearer connector-fixture' }
          }
    await dialog
      .locator('[data-ui="capabilities.connector.custom.json-input"]')
      .fill(JSON.stringify({ mcpServers: { fixture: spec } }))
    await dialog.getByRole('button', { name: '应用', exact: true }).click()
    await dialog.getByRole('button', { name: '保存', exact: true }).click()
    await expect(dialog.getByRole('button', { name: 'fixture', exact: true })).toBeVisible()
    expect((await host.api('GET', '/api/capabilities/custom')).connectors).toHaveLength(1)
    await expect(dialog.getByRole('button', { name: '保存', exact: true })).toBeEnabled()
    await page.keyboard.press('Escape')
    await expect(dialog).toHaveCount(0)
  }
  async function run(expected: string) {
    const path = `/api/agent-sessions/${host.session.id}`
    const before = new Set((await host.api('GET', path + '/messages')).items.map((row: { id: string }) => row.id))
    await host.api('POST', path + '/turns', { userMessageParts: [{ type: 'text', text: 'CAPABILITY-PROBE:mcp' }] })
    let result:
      | {
          status: string
          searchableText: string
          data: { parts: Array<{ type: string; toolName?: string; state: string; output: unknown }> }
        }
      | undefined
    await expect
      .poll(
        async () => {
          result = (await host.api('GET', path + '/messages')).items.find(
            (row: { id: string; role: string }) => row.role === 'assistant' && !before.has(row.id)
          )
          return result?.status === 'success' ? result.searchableText : ''
        },
        { timeout: 60000 }
      )
      .toContain(expected)
    return result!
  }
  await configure('stdio')
  const stdio = await run('CAPABILITY-RESULT:mcp:PASS')
  const stdioReceipt = join(host.stage, 'stdio-turn.json')
  writeFileSync(stdioReceipt, JSON.stringify(stdio))
  await info.attach('stdio-turn', { path: stdioReceipt, contentType: 'application/json' })
  expect(
    stdio.data.parts.some(
      (part) =>
        part.type === 'dynamic-tool' &&
        part.toolName === 'mcp__fixture__lookup' &&
        part.state === 'output-available' &&
        JSON.stringify(part.output).includes('fixture')
    )
  ).toBe(true)
  expect(readFileSync(log, 'utf8')).toContain('"event": "called"')
  await host.restart()
  expect((await host.api('GET', '/api/capabilities/custom')).connectors[0].name).toBe('fixture')
  await page.reload()
  await dismissBrowserBootstrap(page)
  await page.locator('[data-ui="workbench.nav.connectors"]').click()
  for (const transport of ['sse', 'streamable_http'] as const) {
    await configure(transport)
    const result = await run('CAPABILITY-RESULT:mcp:PASS')
    expect(
      result.data.parts.some(
        (part) =>
          part.state === 'output-available' && part.type === 'dynamic-tool' && part.toolName === 'mcp__fixture__lookup'
      )
    ).toBe(true)
    expect(host.mcp.calls).toContain(transport)
  }
  await page.screenshot({ path: info.outputPath('native-connectors.png') })
  const count = host.mcp.calls.length
  await host.api('DELETE', '/api/capabilities/custom/fixture')
  await run('CAPABILITY-MISSING:mcp')
  expect(host.mcp.calls).toHaveLength(count)
  expect((await host.api('GET', '/api/capabilities/custom')).connectors).toEqual([])
  const runtime = readFileSync(join(host.profile, 'capabilities/mcp.json'), 'utf8')
  expect(JSON.parse(runtime).mcpServers.fixture).toBeUndefined()
})
