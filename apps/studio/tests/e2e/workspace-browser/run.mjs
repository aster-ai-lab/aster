/**
 * Workspace browser end-to-end check.
 *
 * Chain:
 * 1. The address bar accepts an http address without an Electron guest.
 * 2. Keystrokes do not open a page. Enter asks launch_workspace_browser for the pane rectangle.
 * 3. A second address updates that same embed. The payload is not a 1280×860 window.
 * 4. A rejected address does not add a launch.
 * 5. Text that is not a web address shows Quark suggestions, then opens https://quark.sm.cn/s?q= inside the same pane.
 * 6. Collapsing the workspace sends visible:false and leaves the conversation clickable.
 *
 * Run from apps/studio after a ToWork renderer build:
 *   node tests/e2e/workspace-browser/run.mjs
 *
 * The preview at http://127.0.0.1:4318 must already be serving .studio-renderer.
 * This script does not restart ToWork or the preview.
 * A native child webview is not visible in this Chromium preview.
 */

import { readdirSync } from 'node:fs'
import { mkdir, writeFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)
const pnpm = fileURLToPath(new URL('../../../node_modules/.pnpm/', import.meta.url))
const playwrightDir = readdirSync(pnpm).find((entry) => entry.startsWith('playwright@'))
if (!playwrightDir) throw new Error('playwright is not installed')
const { chromium } = require(join(pnpm, playwrightDir, 'node_modules/playwright/index.js'))

const PREVIEW = 'http://127.0.0.1:4318/windows/main/index.html'
const OUT = join(tmpdir(), 'workspace-browser-e2e')

function fail(message) {
  throw new Error(message)
}

function startFixture() {
  const page = (marker) => `<!doctype html><meta charset="utf-8"><title>A3S workspace browser fixture</title>
<body style="margin:0;background:#f4f1ea;color:#171717;font:20px/1.4 sans-serif">
<main style="padding:48px"><h1>工作区浏览器端到端</h1><p id="marker">${marker}</p></main>
</body>`
  const server = createServer((request, response) => {
    const marker = request.url?.startsWith('/next') ? 'fixture-next' : 'fixture-ready'
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'cache-control': 'no-store' })
    response.end(page(marker))
  })
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      const address = server.address()
      resolve({ server, url: `http://127.0.0.1:${address.port}/` })
    })
  })
}

function separateWindow(payload) {
  return payload?.width === 1280 && payload?.height === 860
}

async function openBrowser(page) {
  const loader = page.locator('[data-ui="page.loader.wordmark"]')
  if (await loader.count())
    await loader
      .first()
      .waitFor({ state: 'hidden', timeout: 8000 })
      .catch(() => {})
  const rail = page.locator('[data-workspace-shortcut-rail-item="browser"]')
  const session = page.locator('[data-ui="workbench.session"][data-session-place="recent"]').first()
  await session.waitFor({ state: 'visible', timeout: 20000 })
  if (!(await rail.isVisible().catch(() => false))) {
    await session.locator('button').first().click()
    await rail.waitFor({ state: 'visible', timeout: 20000 })
  }
  await rail.click()
}

async function main() {
  await mkdir(OUT, { recursive: true })
  const fixture = await startFixture()
  const launches = []
  const browser = await chromium.launch({ headless: true })
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  try {
    await page.exposeBinding('__a3sE2eLaunch', async (_source, payload) => {
      const url = typeof payload?.url === 'string' ? payload.url : ''
      const loopback = url.startsWith('http://127.0.0.1:')
      const quarkSearch = url.startsWith('https://quark.sm.cn/s?')
      if (!loopback && !quarkSearch) fail(`refused to launch ${JSON.stringify(payload)}`)
      if (!payload?.sessionId) fail('launch is missing sessionId')
      if (typeof payload.x !== 'number' || typeof payload.y !== 'number') fail('launch is missing the pane origin')
      if (typeof payload.visible !== 'boolean') fail('launch is missing visible')
      if (separateWindow(payload)) fail(`launch uses a separate window size ${payload.width}x${payload.height}`)
      launches.push({ ...payload })
      return null
    })
    const stamp = new Date().toISOString()
    const session = {
      id: 'e2e-workspace-browser',
      agentId: null,
      name: '浏览器端到端',
      isNameManuallyEdited: true,
      workspaceId: 'e2e-workspace',
      workspace: {
        id: 'e2e-workspace',
        name: '收件箱',
        path: '/tmp/a3s-workspace-browser-e2e',
        type: 'system',
        orderKey: 'a0',
        createdAt: stamp,
        updatedAt: stamp
      },
      orderKey: 'a0',
      lastActivityAt: stamp,
      createdAt: stamp,
      updatedAt: stamp
    }
    await page.addInitScript((seededSession) => {
      const paths = []
      globalThis.__a3sE2ePaths = paths
      const wrapped = new WeakSet()
      const envelope = (request, data) => ({
        id: request?.id,
        status: 200,
        data,
        metadata: { timestamp: Date.now() }
      })
      const seed = (request) => {
        const method = String(request?.method || 'GET').toUpperCase()
        const path = String(request?.path || '')
        paths.push(`${method} ${path}`)
        if (method !== 'GET') return null
        const params = request?.params && typeof request.params === 'object' ? request.params : {}
        if (path === '/agent-sessions') {
          if (params.inTrash === true || params.inTrash === 'true') return null
          return envelope(request, { items: [seededSession], total: 1 })
        }
        if (path === `/agent-sessions/${seededSession.id}`) return envelope(request, seededSession)
        if (path === `/agent-sessions/${seededSession.id}/messages`) return envelope(request, { items: [] })
        return null
      }
      const wrap = (api) => {
        if (!api || typeof api !== 'object' || wrapped.has(api)) return
        const dataApi = api.dataApi
        if (!dataApi || typeof dataApi.request !== 'function') return
        wrapped.add(api)
        const original = dataApi.request.bind(dataApi)
        dataApi.request = (request) => {
          const seeded = seed(request)
          if (seeded) return Promise.resolve(seeded)
          return original(request)
        }
      }
      let current
      Object.defineProperty(globalThis, 'api', {
        configurable: true,
        enumerable: true,
        get() {
          return current
        },
        set(value) {
          current = value
          wrap(value)
        }
      })
      const invoke = (command, args) => {
        if (command === 'launch_workspace_browser') return globalThis.__a3sE2eLaunch(args)
        if (command === 'quark_search_suggestions') {
          return Promise.resolve(args?.query === '天气预报' ? ['天气预报', '天气预报15天'] : [])
        }
        return Promise.resolve(undefined)
      }
      globalThis.__TAURI_INTERNALS__ = { invoke }
    }, session)
    await page.goto(PREVIEW, { waitUntil: 'domcontentloaded' })
    await openBrowser(page)
    const address = page.getByRole('combobox', { name: '网页地址' }).or(page.getByRole('textbox', { name: '网页地址' }))
    await address.waitFor({ state: 'visible', timeout: 15000 })
    await page.waitForFunction(() => {
      const field = document.querySelector('[aria-label="网页地址"]')
      return field instanceof HTMLInputElement && !field.disabled
    })
    await address.click()
    await address.press('Meta+A')
    await address.pressSequentially(fixture.url, { delay: 8 })
    await page.waitForTimeout(350)
    if (launches.length !== 0) fail(`address keystrokes opened a page: ${JSON.stringify(launches)}`)
    await address.press('Enter')
    await page.locator('[data-workspace-browser="a3s-webview"]').waitFor({ state: 'visible', timeout: 10000 })
    await page.waitForFunction(
      () =>
        document
          .querySelector('[data-workspace-browser="a3s-webview"]')
          ?.getAttribute('data-workspace-browser-phase') === 'opened'
    )
    if (launches.length < 1) fail('Enter did not open the page')
    if (!launches.every((launch) => launch.visible === true && launch.url.startsWith(fixture.url))) {
      fail(`opened ${JSON.stringify(launches)}`)
    }
    const slot = await page.locator('[data-workspace-browser="a3s-webview"]').boundingBox()
    const placed = launches.at(-1)
    if (!slot || Math.abs(placed.x - slot.x) > 1 || Math.abs(placed.y - slot.y) > 1) {
      fail(`embed origin ${placed.x},${placed.y} is outside the pane ${JSON.stringify(slot)}`)
    }
    if (Math.abs(placed.width - slot.width) > 1 || Math.abs(placed.height - slot.height) > 1) {
      fail(`embed size ${placed.width}x${placed.height} does not match the pane ${slot.width}x${slot.height}`)
    }
    const openedCount = launches.length
    await page.screenshot({ path: join(OUT, '01-opened.png') })

    await address.click()
    await address.press('Meta+A')
    await address.pressSequentially('javascript:alert(1)', { delay: 8 })
    await address.press('Enter')
    await page.getByText('请输入有效的 HTTP、HTTPS 或文件地址').waitFor({ timeout: 5000 })
    if (launches.slice(openedCount).some((launch) => !String(launch.url).startsWith(fixture.url))) {
      fail('a rejected address added a launch')
    }

    const query = '天气预报'
    await address.click()
    await address.press('Meta+A')
    await address.pressSequentially(query, { delay: 8 })
    await page.waitForTimeout(350)
    if (launches.slice(openedCount).some((launch) => !String(launch.url).startsWith(fixture.url))) {
      fail('search keystrokes opened a page')
    }
    await page.getByRole('option', { name: '天气预报15天' }).waitFor({ timeout: 5000 })
    await address.press('Enter')
    const searchStarted = Date.now()
    while (
      !launches.some((launch) => launch.url.startsWith('https://quark.sm.cn/s?')) &&
      Date.now() - searchStarted < 5000
    ) {
      await page.waitForTimeout(100)
    }
    const searchLaunch = launches.findLast((launch) => launch.url.startsWith('https://quark.sm.cn/s?'))
    if (!searchLaunch?.visible) fail(`search did not open Quark: ${JSON.stringify(launches)}`)
    if (new URL(searchLaunch.url).searchParams.get('q') !== query) fail(`Quark query was ${searchLaunch.url}`)
    if (separateWindow(searchLaunch)) fail('Quark search requested a separate window')
    await page.screenshot({ path: join(OUT, '04-quark-search.png') })

    const nextUrl = new URL('/next', fixture.url).toString()
    await address.click()
    await address.press('Meta+A')
    await address.pressSequentially(nextUrl, { delay: 8 })
    await address.press('Enter')
    const started = Date.now()
    while (!launches.some((launch) => launch.url.startsWith(nextUrl)) && Date.now() - started < 5000) {
      await page.waitForTimeout(100)
    }
    const nextLaunch = launches.findLast((launch) => launch.url.startsWith(nextUrl))
    if (!nextLaunch?.visible) fail(`the next address did not update the embed: ${JSON.stringify(launches)}`)
    if (separateWindow(nextLaunch)) fail('the next address requested a separate window')
    await page.screenshot({ path: join(OUT, '03-next-address.png') })

    await page.locator('[data-workspace-chrome-action="collapse"]').click()
    await page.waitForFunction(() => {
      const pane = document.querySelector('[data-right-pane]')
      return pane?.getAttribute('data-right-pane-phase') === 'closed'
    })
    const cover = await page.evaluate(() => {
      const status = document.querySelector('[data-workspace-browser="a3s-webview"]')
      const pane = document.querySelector('[data-right-pane]')
      const field = document.querySelector('[contenteditable="true"], textarea')
      const rect = field?.getBoundingClientRect()
      const hit = rect ? document.elementFromPoint(rect.left + 12, rect.top + 12) : null
      return {
        fixed: status ? getComputedStyle(status).position === 'fixed' : false,
        outsidePane: Boolean(status && pane && !pane.contains(status)),
        blocksConversation: Boolean(status && hit && (hit === status || status.contains(hit))),
        mode: pane?.getAttribute('data-right-pane-mode') ?? ''
      }
    })
    const hiddenAt = Date.now()
    while (!launches.some((launch) => launch.visible === false) && Date.now() - hiddenAt < 5000) {
      await page.waitForTimeout(50)
    }
    if (!launches.some((launch) => launch.visible === false)) {
      fail(`collapsed workspace did not hide the embed: ${JSON.stringify(launches)}`)
    }
    if (cover.fixed || cover.outsidePane || cover.blocksConversation || cover.mode !== 'closed') {
      fail(`collapsed workspace still covers the conversation: ${JSON.stringify(cover)}`)
    }
    await page.screenshot({ path: join(OUT, '02-collapsed.png') })
    await writeFile(
      join(OUT, 'result.json'),
      JSON.stringify({ fixture: fixture.url, launches, cover, out: OUT }, null, 2)
    )
    console.log(`PASS ${OUT}`)
  } catch (error) {
    await page.screenshot({ path: join(OUT, 'failure.png') }).catch(() => {})
    const detail = await page
      .evaluate(() => ({
        paths: globalThis.__a3sE2ePaths || [],
        text: document.body?.innerText?.slice(0, 1800) || ''
      }))
      .catch(() => ({ paths: [], text: '' }))
    console.error(error)
    console.error(JSON.stringify(detail, null, 2))
    process.exitCode = 1
  } finally {
    await browser.close()
    await new Promise((resolve) => fixture.server.close(resolve))
  }
}

await main()
