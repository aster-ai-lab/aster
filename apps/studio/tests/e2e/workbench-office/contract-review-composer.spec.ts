import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { expect, test, type Page } from '@playwright/test'

import { installWorkbench, ipcCount, workAgent, type OfficeMessage } from './fixture'

const sourcePath = '/tmp/a3s-workbench-office/合同.docx'
const bytes = readFileSync(resolve('tests/e2e/reference-alignment/cases/review-basic/inputs/a.docx'))
const revision = createHash('sha256').update(bytes).digest('hex')
const editor = '[data-ui~="workbench.composer"] .composer-tiptap'

async function catalogue(page: Page) {
  await page.route('**/api/agents/agent-work', (route) =>
    route.request().method() === 'PATCH'
      ? route.fulfill({ json: { ...workAgent, configuration: { permission_mode: 'default' } } })
      : route.fallback()
  )
  await page.route('**/api/catalog-agents*', (route) => route.fulfill({ json: { items: [] } }))
  await page.route('**/api/capabilities?*', (route) =>
    route.fulfill({
      json: {
        items: [
          {
            slug: 'a3s-review',
            kind: 'plugin',
            name: '合同审查',
            category: 'other',
            installed: true,
            description: '通过输入框选择文件进行审查',
            version: '1.3.0',
            surfaces: ['skill', 'agent'],
            agentRuntime: {
              runtime: 'edge',
              harness: 'builtin',
              resources: { cpuMillis: 500, memoryBytes: 268435456 }
            },
            install: {
              type: 'plugin',
              packageType: 'aix',
              available: true,
              packageId: 'use/a3s/review',
              digest: `sha256:${'a'.repeat(64)}`,
              size: 1024
            }
          }
        ],
        total: 1,
        page: 1,
        pageSize: 100,
        categories: []
      }
    })
  )
}

test('selects @合同审查 and binds an explicit file path in the composer without opening a window', async ({ page }) => {
  await installWorkbench(page, {
    withAgent: true,
    collab: true,
    publishReview: true,
    invoke: 'record',
    preferences: { 'chat.default_model_id': 'a3s::gpt-work' },
    ui: { withAgent: true }
  })
  await catalogue(page)
  await page.goto('/windows/main/index.html')
  await expect(page.locator(editor)).toBeVisible()
  await page.locator(editor).fill('@合同审查')
  const item = page.getByTestId('quick-panel').locator('[data-id="workbench-agent:a3s-review"]')
  await expect(item).toBeVisible()
  await item.click()
  await expect(page.locator('[data-review-source-picker]')).toBeVisible()
  await page.getByRole('textbox', { name: 'Word 文件路径' }).fill(sourcePath)
  await page.getByRole('button', { name: '使用文件', exact: true }).click()
  await expect(page.locator('[data-review-source-picker]')).toHaveCount(0)
  await expect(page.getByTestId('quick-panel')).toBeHidden()
  await expect(page.locator(editor)).toContainText('合同.docx')
  const invoked = await page.evaluate(() => (globalThis as { __E2E_INVOKE__?: string[] }).__E2E_INVOKE__ ?? [])
  expect(invoked).not.toContain('open_workbench_applet_window')
  await page.screenshot({ path: test.info().outputPath('mention-file.png') })
  await page.locator(editor).press('End')
  await page.keyboard.insertText(' 检查错字和付款条款')
  await page.locator('[data-ui="workbench.composer"] [data-ui="chat.composer.action.send"]').click()
  try {
    await expect.poll(() => ipcCount(page, 'ai.stream.open')).toBe(1)
  } catch (error) {
    await test.info().attach('send-boundary', {
      body: JSON.stringify(
        await page.evaluate(() => ({
          ipc: (globalThis as { __E2E_IPC__?: unknown }).__E2E_IPC__,
          http: (globalThis as { __E2E_HTTP__?: unknown }).__E2E_HTTP__,
          notices: [...document.querySelectorAll('[role="alert"], [data-sonner-toast]')].map((node) => node.textContent)
        }))
      ),
      contentType: 'application/json'
    })
    throw error
  }
  const turn = await page.evaluate(() => {
    const host = globalThis as { __E2E_HTTP__?: { method: string; url: string; body: string }[] }
    const request = host.__E2E_HTTP__?.find(
      (item) => item.method === 'POST' && /\/agent-sessions\/[^/]+\/turns(?:\?|$)/u.test(item.url)
    )
    return request ? JSON.parse(request.body) : null
  })
  expect(turn).toMatchObject({ turnContext: { agentId: 'a3s-review' } })
  expect(JSON.stringify(turn)).toContain('检查错字和付款条款')
  const text = turn.userMessageParts
    .filter((part: { type: string }) => part.type === 'text')
    .map((part: { text: string }) => part.text)
    .join('\n')
  expect(text.split('\n')).toContain(sourcePath)
})

test('opens a single Word file chooser from a keyboard-selected agent and preserves the draft on cancel', async ({
  page
}) => {
  await installWorkbench(page, {
    withAgent: true,
    collab: true,
    publishReview: true,
    invoke: 'record',
    preferences: { 'chat.default_model_id': 'a3s::gpt-work' },
    ui: { withAgent: true }
  })
  await catalogue(page)
  await page.goto('/windows/main/index.html')
  await page.locator(editor).fill('@合同审查')
  await expect(page.getByTestId('quick-panel').locator('[data-id="workbench-agent:a3s-review"]')).toBeVisible()
  await page.keyboard.press('Enter')
  const pick = page.getByRole('button', { name: '选择 Word 文件' })
  await expect(pick).toBeVisible()
  const pending = page.waitForEvent('filechooser')
  await pick.click()
  const chooser = await pending
  expect(chooser.isMultiple()).toBe(false)
  expect(await chooser.element().getAttribute('accept')).toBe('.docx')
  await chooser.setFiles([])
  await expect(pick).toBeEnabled()
  await expect(page.locator(editor)).toContainText('合同审查')
  await expect(page.locator('[data-review-source-picker]')).toBeVisible()
  const nextChoice = page.waitForEvent('filechooser')
  await pick.click()
  await (await nextChoice).setFiles(resolve('tests/e2e/reference-alignment/cases/review-basic/inputs/a.docx'))
  await expect(page.locator('[data-review-source-picker]')).toHaveCount(0)
  await expect(page.locator(editor)).toContainText('a.docx')
  expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
  await page.locator('[data-ui="workbench.composer"] [data-ui="chat.composer.action.send"]').click()
  await expect.poll(() => ipcCount(page, 'ai.stream.open')).toBe(1)
})

test('opens the version-bound Word source with host attention and live comments in the task workspace', async ({
  page
}) => {
  page.on('pageerror', (error) => console.error(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') console.error(message.text().slice(0, 300))
  })
  let reads = 0
  await installWorkbench(page, {
    withAgent: true,
    collab: true,
    publishReview: true,
    workspaceFiles: true,
    sessions: [
      {
        id: 'review-task',
        name: '合同审查任务',
        messages: [
          { id: 'a-review-user', role: 'user', text: `使用智能体 合同审查：\n检查错字\n${sourcePath}` },
          {
            id: 'b-review-assistant',
            role: 'assistant',
            status: 'pending',
            text: '',
            parts: [
              { type: 'data-review-source', data: { kind: 'document', path: sourcePath, sha256: revision } },
              { type: 'data-review-progress', data: { index: 1, total: 2, quote: '我们以经完成测试' } },
              {
                type: 'text',
                text: '```a3s-review-finding\n{"id":"typo","title":"错字","evidenceQuote":"以经","summary":"用字错误","guidance":"已经"}\n```'
              }
            ]
          }
        ].reverse() as OfficeMessage[]
      }
    ]
  })
  await catalogue(page)
  await page.route('**/api/workspace-file/snapshot?*', (route) => {
    reads += 1
    return route.fulfill({
      body: bytes,
      headers: {
        'content-type': 'application/octet-stream',
        etag: `"${revision}"`,
        'content-length': String(bytes.length),
        'access-control-allow-origin': '*',
        'access-control-expose-headers': 'etag, content-length'
      }
    })
  })
  await page.addInitScript(() => {
    const original = globalThis.fetch.bind(globalThis)
    globalThis.fetch = (input, init) => {
      if (String(input).includes('/api/local-documents/changes?'))
        return Promise.resolve(
          new Response(
            new ReadableStream({
              start(controller) {
                controller.enqueue(new TextEncoder().encode('data: {"revision":1,"paths":[],"fullScan":false}\n\n'))
              }
            }),
            { headers: { 'content-type': 'text/event-stream' } }
          )
        )
      return original(input, init)
    }
  })
  await page.goto('/windows/main/index.html')
  await page.getByRole('button', { name: '合同审查任务', exact: true }).click()
  await expect.poll(() => reads).toBeGreaterThan(0)
  const document = page.locator('[data-review-office-document]')
  await expect(document).toBeVisible()
  await expect(document.getByRole('status').first()).toContainText('第 1 / 2 段')
  await expect(document.getByRole('status').first()).toContainText('已批注 1 条')
  await expect
    .poll(async () => (await document.locator('[data-review-attention]').allTextContents()).join(''))
    .toBe('我们以经完成测试')
  await expect(document.locator('.work-document-comments-panel')).toContainText('建议：已经')
  await expect(document.locator('.work-document-comments-panel')).toBeInViewport({ ratio: 0.8 })
  await expect(document.locator('.ProseMirror')).toHaveAttribute('contenteditable', 'false')
  await expect(document.locator('.ProseMirror')).toHaveText('我们以经完成测试，请按排验收。')
  await document.getByRole('button', { name: '关闭批注审阅' }).click()
  await expect(document.locator('.work-document-comments-panel')).toHaveCount(0)
  await document.getByRole('button', { name: '文件', exact: true }).click()
  await page.getByRole('menuitem', { name: '批注', exact: true }).click()
  await expect(document.locator('.work-document-comments-panel')).toBeInViewport({ ratio: 0.8 })
  await expect(document.getByRole('status').first()).toContainText('已批注 1 条')
  await expect(page.locator('[data-task-auto-review]')).toHaveCount(0)
  await expect(page.locator('[data-chat-review-findings]')).toHaveCount(0)
  await page.screenshot({ path: test.info().outputPath('document-review.png') })
})
