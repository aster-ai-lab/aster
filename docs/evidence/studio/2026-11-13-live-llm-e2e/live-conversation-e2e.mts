#!/usr/bin/env -S npx tsx
/**
 * Real-LLM conversation E2E: real Studio native host + real ACP turn +
 * a real external provider, with Playwright renderer checks.
 *
 * Reuses connectors/nativeHarness.ts orchestration WITHOUT modifying the
 * submodule: after the harness is up, a second provider pointing at a real
 * endpoint is registered and a fresh session is driven through a live turn.
 *
 * Gate: ASTER_E2E_LIVE_MODEL=1. Credentials come from ~/.aster/config.acl,
 * are held in memory only, and are never logged or written to disk.
 * One turn per run, bounded by LIVE_TURN_TIMEOUT_MS (default 120000).
 */
import { randomUUID } from 'node:crypto'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'
import { createRequire } from 'node:module'
import { homedir } from 'node:os'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

const STUDIO_ROOT = resolve('/Users/roylin/code/aster/apps/studio')
const HARNESS = STUDIO_ROOT + '/tests/e2e/connectors/nativeHarness.ts'
const requireFromStudio = createRequire(STUDIO_ROOT + '/package.json')

const PROMPT = 'Reply with exactly the token LIVELLMREADY and nothing else.'
const TOKEN = 'LIVELLMREADY'
const TURN_TIMEOUT = Number(process.env.LIVE_TURN_TIMEOUT_MS ?? '120000')
const RECEIPT = resolve('/Users/roylin/code/aster/docs/evidence/studio/2026-11-13-live-llm-e2e/last-turn.json')
const FIXTURE = resolve('/Users/roylin/code/aster/docs/evidence/studio/2026-11-13-live-llm-e2e/fixtures/live-turn-parts.json')

type ProviderConfig = { name: string; baseUrl: string; apiKey: string; modelId: string }

function readRealProvider(): ProviderConfig {
  const text = readFileSync(homedir() + '/.aster/config.acl', 'utf8')
  for (const block of text.split(/\n\s*providers\s+"/).slice(1)) {
    if (block.split('"')[0] !== 'deepseek') continue
    const baseUrl = /base_url\s*=\s*"([^"]+)"/.exec(block)?.[1]
    const apiKey = /api_key\s*=\s*"([^"]+)"/.exec(block)?.[1]
    // The block key (e.g. "deepseek-flash") is the wire model id; the inner
    // `name` is only a display label and is rejected by the models endpoint.
    const modelId = /models\s+"([^"]+)"/.exec(block)?.[1]
    if (baseUrl && apiKey && modelId) return { name: 'deepseek', baseUrl, apiKey, modelId }
  }
  throw new Error('No usable real provider found in ~/.aster/config.acl')
}

function log(...parts: unknown[]) {
  console.log('[live-llm]', ...parts)
}

async function main(): Promise<number> {
  if (process.env.ASTER_E2E_LIVE_MODEL !== '1') {
    console.log('LIVE_LLM_E2E: SKIPPED (set ASTER_E2E_LIVE_MODEL=1 for a real model turn)')
    return 0
  }

  const provider = readRealProvider()
  const endpoint = new URL(provider.baseUrl)
  log('provider =', provider.name, 'model =', provider.modelId, 'host =', endpoint.hostname)

  const { nativeHarness } = await import(pathToFileURL(HARNESS).href)
  const host = await nativeHarness()
  let browser: { close: () => Promise<void> } | undefined
  const receipt: Record<string, unknown> = {
    gate: 'ASTER_E2E_LIVE_MODEL=1',
    provider: provider.name,
    model: provider.modelId,
    endpointHost: endpoint.hostname,
    prompt: PROMPT,
    startedAt: new Date().toISOString()
  }

  try {
    const providerId = randomUUID()
    const base = provider.baseUrl.replace(/\/$/, '')
    await host.api('POST', `/api/providers/${providerId}`, {
      name: '真实模型 E2E',
      enabled: true,
      presetProviderId: 'openai',
      adapterFamily: 'openai-compatible',
      defaultChatEndpoint: 'openai-chat-completions',
      apiKey: provider.apiKey,
      baseUrl: base.endsWith('/v1') ? base : base + '/v1'
    })
    await host.api('POST', `/api/providers/${providerId}/models`, {
      id: provider.modelId,
      name: provider.modelId,
      enabled: true,
      supportsStreaming: true,
      capabilities: ['function-call']
    })
    log('real provider registered')

    const workspace = host.stage + '/live-model-workspace'
    mkdirSync(workspace, { recursive: true })
    await host.api('POST', '/api/agent-workspaces/initialize', { projects: [] })
    const project = await host.api('POST', '/api/agent-workspaces', { path: workspace, name: '真实模型 E2E' })
    const agent = await host.api('POST', '/api/agents', {
      type: 'aster',
      name: '真实模型对话流',
      model: `${providerId}::${provider.modelId}`,
      configuration: { permission_mode: 'bypassPermissions' }
    })
    const session = await host.api('POST', '/api/agent-sessions', {
      agentId: agent.id,
      name: '真实模型对话流',
      workspace: { type: 'user', workspace: project }
    })
    const sessionPath = `/api/agent-sessions/${session.id}`
    const before = new Set(
      (await host.api('GET', sessionPath + '/messages')).items.map((row: { id: string }) => row.id)
    )
    log('session ready, sending real turn')

    const started = Date.now()
    await host.api('POST', sessionPath + '/turns', {
      userMessageParts: [{ type: 'text', text: PROMPT }]
    })

    type Assistant = {
      id: string
      status: string
      searchableText: string
      data?: { parts?: Array<{ type: string; state?: string }> }
    }
    let assistant: Assistant | undefined
    const deadline = Date.now() + TURN_TIMEOUT
    while (Date.now() < deadline) {
      const items = (await host.api('GET', sessionPath + '/messages')).items as Array<
        Assistant & { role: string }
      >
      assistant = items.find((row) => row.role === 'assistant' && !before.has(row.id))
      if (assistant && (assistant.status === 'success' || assistant.status === 'error')) break
      await new Promise((r) => setTimeout(r, 2000))
    }
    receipt.elapsedMs = Date.now() - started
    receipt.status = assistant?.status ?? 'no-assistant-message'
    receipt.searchableTextLength = assistant?.searchableText?.length ?? 0
    receipt.tokenPresent = (assistant?.searchableText ?? '').includes(TOKEN)
    const liveParts = (assistant?.data?.parts ?? []) as unknown as Array<Record<string, unknown>>
    receipt.partTypes = liveParts.map((p) => String(p.type))
    log('turn result:', receipt.status, 'elapsed', receipt.elapsedMs, 'ms',
        'parts', JSON.stringify(receipt.partTypes))

    // Persist the REAL turn parts as a rendering fixture. The turn uses a
    // fixed synthetic prompt, so the payload contains no credentials or
    // personal data; provider keys are never included.
    if (liveParts.length > 0) {
      mkdirSync(dirname(FIXTURE), { recursive: true })
      writeFileSync(FIXTURE, JSON.stringify({
        source: 'live-llm-turn',
        provider: provider.name,
        model: provider.modelId,
        capturedAt: new Date().toISOString(),
        status: assistant?.status,
        partTypes: receipt.partTypes,
        parts: liveParts
      }, null, 2))
      log('fixture written:', FIXTURE, 'parts =', liveParts.length)
    }

    const apiOk = assistant?.status === 'success'
      && (assistant.searchableText ?? '').length > 0
      && (assistant.data?.parts ?? []).some((p) => p.type === 'text')

    let renderOk = false
    let renderDetail = 'not attempted (API layer did not succeed)'
    if (apiOk) {
      try {
        const { chromium } = requireFromStudio('playwright') as typeof import('playwright')
        browser = await chromium.launch({ headless: true })
        const page = await browser.newPage({ locale: 'zh-CN' })
        const consoleErrors: string[] = []
        page.on('console', (msg) => {
          if (msg.type() === 'error') consoleErrors.push(msg.text().slice(0, 160))
        })
        page.on('pageerror', (err) => consoleErrors.push('pageerror: ' + String(err).slice(0, 160)))
        const response = await page.goto('http://127.0.0.1:4598/windows/main/index.html', {
          waitUntil: 'domcontentloaded', timeout: 30000
        })
        await page.waitForTimeout(6000)
        const html = await page.content().catch(() => '')
        const bodyText = await page.locator('body').innerText().catch(() => '')
        const messageNodes = await page.locator('[data-ui*="message"], [role="article"]').count().catch(() => 0)
        renderOk = bodyText.includes(TOKEN)
        renderDetail = `http=${response?.status()} title=${await page.title().catch(() => '?')} ` +
          `htmlLength=${html.length} bodyLength=${bodyText.length} messageNodes=${messageNodes} ` +
          `tokenVisible=${bodyText.includes(TOKEN)} consoleErrors=${JSON.stringify(consoleErrors.slice(0, 3))}`
        receipt.render = { ok: renderOk, detail: renderDetail }
        log('renderer:', renderDetail)
      } catch (error) {
        renderDetail = String(error)
        receipt.render = { ok: false, detail: renderDetail }
        log('renderer error:', renderDetail)
      }
    }

    receipt.finishedAt = new Date().toISOString()
    writeFileSync(RECEIPT, JSON.stringify(receipt, null, 2))
    if (apiOk && renderOk) {
      console.log('\nLIVE_LLM_E2E: PASS')
      console.log('  real model reply reached the assistant message AND painted in the renderer')
      return 0
    }
    console.log('\nLIVE_LLM_E2E: FAIL')
    console.log('  apiOk=' + apiOk + ' renderOk=' + renderOk)
    console.log('  ' + JSON.stringify(receipt))
    return 1
  } catch (error) {
    console.error('[live-llm] run error:', error)
    throw error
  } finally {
    if (browser) await browser.close().catch(() => {})
    // The WorkBuddy sandbox blocks recursive rm of the isolated profile
    // (bulk-delete guard). Cleanup is best-effort; never let it mask the
    // real error, and never weaken a test gate because of it.
    try {
      await host.close()
    } catch (cleanupError) {
      log('cleanup skipped (sandbox guard):', String(cleanupError).slice(0, 160))
    }
  }
}

main().then((code) => process.exit(code)).catch((error) => {
  console.error('LIVE_LLM_E2E: ERROR -', error)
  process.exit(1)
})
