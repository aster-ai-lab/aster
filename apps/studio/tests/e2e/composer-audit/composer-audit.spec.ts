import { expect, test, type Page } from '@playwright/test'

import { makeWorkbenchProof } from '../../../src/shared/types/__tests__/workbenchProof.fixture'
import type { WorkbenchProof } from '../../../src/shared/types/workbenchProof'
import { composerText, installWorkbench, type OfficeMessage } from '../workbench-office/fixture'

// Production renderer with controlled service boundaries. This is not native or live-model proof.
const OPTIONS = 'workbench.composer.options.v1'
const ARMS = 'workbench.turn-arms.v1'
const root = '[data-ui~="workbench.composer"]'
const editor = `${root} .composer-tiptap`

function findingProof() {
  return makeWorkbenchProof({
    review: {
      status: 'open_queue',
      reviewer: 'independent-model',
      findings: [
        {
          id: 'finding-1',
          severity: 'warning',
          category: 'change',
          claim: 'The creation claim needs evidence',
          evidenceQuote: 'report',
          guidance: 'Verify actual file bytes',
          status: 'pending'
        }
      ]
    }
  })
}

async function start(page: Page, fallback = false, withSession = false) {
  await installWorkbench(page, {
    withAgent: true,
    collab: true,
    workspaceFiles: true,
    ...(withSession ? { sessions: [{ id: 'session-writing', name: '输入框测试任务', messages: [] }] } : {}),
    ui: {
      withAgent: true,
      holdComposer: fallback,
      ...(withSession ? { sessions: [{ id: 'session-writing', name: '输入框测试任务', place: 'inbox' as const }] } : {})
    }
  })
  if (withSession) {
    await page.addInitScript(() => {
      let defaults =
        (globalThis as { __A3S_PREFERENCE_DEFAULTS__?: Record<string, unknown> }).__A3S_PREFERENCE_DEFAULTS__ ?? {}
      Object.defineProperty(globalThis, '__A3S_PREFERENCE_DEFAULTS__', {
        configurable: true,
        get: () => ({ ...defaults, 'chat.default_model_id': 'a3s::gpt-work' }),
        set: (value) => {
          defaults = value ?? {}
        }
      })
    })
  }
  await page.goto('/windows/main/index.html', { waitUntil: 'domcontentloaded' })
  await expect(page.locator(fallback ? '[data-ui~="workbench.composer.fallback"]' : editor)).toBeVisible()
  if (withSession) {
    await page.getByRole('button', { name: '输入框测试任务', exact: true }).click()
    await expect(page.locator('[data-ui~="workbench.canvas"]')).toHaveCount(0)
    await page.locator('[data-new-task-trigger="true"]').click()
    await expect(page.locator(editor)).toBeVisible()
  }
}

async function choose(page: Page, reviewMode: boolean, artifactTrace: boolean, fallback = false) {
  const host = page.locator(fallback ? '[data-ui~="workbench.composer.fallback"]' : root)
  await host.getByRole('button', { name: '更多选项', exact: true }).click()
  const menu = page.locator('[data-workbench-menu="options"]')
  await menu.getByRole('switch', { name: '自动审查', exact: true }).setChecked(reviewMode)
  await menu.getByRole('switch', { name: '产物追溯', exact: true }).setChecked(artifactTrace)
  await page.keyboard.press('Escape')
}

async function capture(page: Page, name: string) {
  await page.screenshot({ path: test.info().outputPath(`${name}.png`), fullPage: true })
}

async function streamInputs(page: Page) {
  return page.evaluate(() => {
    const host = globalThis as typeof globalThis & { __E2E_HTTP__?: { method: string; url: string; body: string }[] }
    return (host.__E2E_HTTP__ ?? [])
      .filter((row) => row.method === 'POST' && new URL(row.url, location.href).pathname.endsWith('/turns'))
      .map((row) => JSON.parse(row.body))
  })
}

test.afterEach(async ({ page }) => {
  if (test.info().status === test.info().expectedStatus || page.isClosed()) return
  const diagnostics = await page
    .evaluate(() => {
      const host = globalThis as typeof globalThis & {
        __E2E_IPC__?: { route: string; input: unknown }[]
        __E2E_HTTP__?: { method: string; url: string }[]
      }
      return {
        ipcRoutes: (host.__E2E_IPC__ ?? []).map((row) => row.route),
        httpRoutes: (host.__E2E_HTTP__ ?? []).map((row) => ({
          method: row.method,
          path: new URL(row.url, location.href).pathname
        }))
      }
    })
    .catch(() => ({ unavailable: true }))
  await test
    .info()
    .attach('boundary-diagnostics', { body: JSON.stringify(diagnostics), contentType: 'application/json' })
})

test('UI-01 empty input, default policies, menu names and no accidental send', async ({ page }) => {
  await start(page)
  await expect(page.locator(root).getByRole('button', { name: '发送', exact: true })).toBeDisabled()
  await page.locator(root).getByRole('button', { name: '更多选项', exact: true }).click()
  await expect(page.getByRole('switch', { name: '自动审查', exact: true })).not.toBeChecked()
  await expect(page.getByRole('switch', { name: '产物追溯', exact: true })).not.toBeChecked()
  await page.keyboard.press('Escape')
  await expect.poll(() => composerText(page)).toBe('')
  await expect(page.locator('[data-task-auto-review]')).toHaveCount(0)
  await expect(page.locator('[data-task-lineage-open]')).toHaveCount(0)
  await capture(page, 'empty-home')
})

test('UI-02 keyboard switches, Escape focus restoration and draft retention', async ({ page }) => {
  await start(page)
  await page.locator(editor).fill('输入框草稿：保留中文、English 和标点。')
  const trigger = page.locator(root).getByRole('button', { name: '更多选项', exact: true })
  await trigger.focus()
  await page.keyboard.press('Enter')
  const review = page.getByRole('switch', { name: '自动审查', exact: true })
  await review.focus()
  await page.keyboard.press('Space')
  await expect(review).toBeChecked()
  await page.keyboard.press('Tab')
  await expect(page.getByRole('switch', { name: '产物追溯', exact: true })).toBeFocused()
  await page.keyboard.press('Space')
  await expect(page.getByRole('switch', { name: '产物追溯', exact: true })).toBeChecked()
  await page.keyboard.press('Escape')
  await expect(trigger).toBeFocused()
  await expect.poll(() => composerText(page)).toContain('输入框草稿：保留中文、English 和标点。')
  await capture(page, 'keyboard-draft')
})

for (const review of [false, true])
  for (const trace of [false, true]) {
    test(`UI-03-${Number(review)}${Number(trace)} independent policies survive reload`, async ({ page }) => {
      await start(page)
      await choose(page, review, trace)
      await expect
        .poll(() =>
          page.evaluate(
            (key) => ({
              reviewMode: false,
              artifactTrace: false,
              researchMode: false,
              ...JSON.parse(localStorage.getItem(key) ?? '{}')
            }),
            OPTIONS
          )
        )
        .toEqual({
          reviewMode: review,
          artifactTrace: trace,
          researchMode: false
        })
      await page.reload({ waitUntil: 'domcontentloaded' })
      await expect(page.locator(editor)).toBeVisible()
      await page.locator(root).getByRole('button', { name: '更多选项', exact: true }).click()
      await expect(page.getByRole('switch', { name: '自动审查', exact: true })).toBeChecked({ checked: review })
      await expect(page.getByRole('switch', { name: '产物追溯', exact: true })).toBeChecked({ checked: trace })
      await capture(page, `policies-${Number(review)}${Number(trace)}`)
    })
  }

test('UI-04 Chinese composition does not send and Shift+Enter adds a line', async ({ page }) => {
  await start(page, false, true)
  const field = page.locator(editor)
  await field.fill('中文候选')
  const sent: string[] = []
  page.on('request', (request) => {
    if (request.method() === 'POST' && request.url().endsWith('/turns')) sent.push(request.url())
  })
  await field.dispatchEvent('compositionstart', { data: '' })
  await field.dispatchEvent('keydown', { key: 'Enter', code: 'Enter', keyCode: 229, isComposing: true })
  await expect.poll(() => composerText(page)).toContain('中文候选')
  expect(sent).toEqual([])
  await field.dispatchEvent('compositionend', { data: '中文候选' })
  await field.focus()
  await page.keyboard.press('Shift+Enter')
  await page.keyboard.insertText('第二行')
  await expect.poll(() => composerText(page)).toContain('第二行')
  expect(sent).toEqual([])
  await page.keyboard.press('Enter')
  await expect.poll(async () => (await streamInputs(page)).length).toBe(1)
})

test('UI-05 slash selection, closing the panel, and undo preserve the editable draft', async ({ page }) => {
  await start(page)
  await page.locator(editor).fill('/')
  const panel = page.getByTestId('quick-panel')
  await expect(panel).toBeVisible()
  await panel.locator('[data-id="workbench-slash:/goal"]').click()
  await expect.poll(() => composerText(page)).toContain('/goal')
  await page.locator(editor).press('End')
  await page.keyboard.insertText(' 保存这份目标')
  await page.keyboard.press('Meta+z')
  await expect(page.locator(editor)).toBeVisible()
  await expect(page.locator('[data-ui~="workbench.canvas"]')).toBeVisible()
})

test('UI-06 options remain reachable inside a narrow viewport', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 700 })
  await start(page)
  await page.locator(root).getByRole('button', { name: '更多选项', exact: true }).click()
  const menu = page.locator('[data-workbench-menu="options"]')
  const box = await menu.boundingBox()
  expect(box).not.toBeNull()
  expect(box!.x).toBeGreaterThanOrEqual(0)
  expect(box!.x + box!.width).toBeLessThanOrEqual(391)
  expect(box!.y).toBeGreaterThanOrEqual(0)
  expect(box!.y + box!.height).toBeLessThanOrEqual(701)
  await expect(menu.getByRole('switch', { name: '自动审查', exact: true })).toBeVisible()
  await capture(page, 'narrow-options')
})

test('UI-07 fallback draft and policies survive the live-composer takeover', async ({ page }) => {
  await start(page, true)
  const fallback = page.locator('[data-ui~="workbench.composer.fallback"]')
  await fallback.locator('textarea').fill('接管前输入的正文不应丢失')
  await choose(page, true, true, true)
  await page.evaluate(() => {
    const host = globalThis as typeof globalThis & { __E2E_HOLD_COMPOSER__?: boolean; __E2E_HELD_COMPOSER__?: Node[] }
    host.__E2E_HOLD_COMPOSER__ = false
    const composer = document.querySelector('[data-ui~="workbench.composer"]')
    for (const node of host.__E2E_HELD_COMPOSER__ ?? [])
      if (composer && node.parentNode !== composer) composer.appendChild(node)
    if (host.__E2E_HELD_COMPOSER__) host.__E2E_HELD_COMPOSER__.length = 0
  })
  await expect(page.locator(editor)).toBeVisible()
  await capture(page, 'fallback-takeover')
  await expect.poll(() => composerText(page)).toContain('接管前输入的正文不应丢失')
  await page.locator(root).getByRole('button', { name: '更多选项', exact: true }).click()
  await expect(page.getByRole('switch', { name: '自动审查', exact: true })).toBeChecked({ timeout: 2000 })
  await expect(page.getByRole('switch', { name: '产物追溯', exact: true })).toBeChecked({ timeout: 2000 })
})

for (const review of [false, true])
  for (const trace of [false, true]) {
    test(`UI-08-${Number(review)}${Number(trace)} selected policies reach the submitted turn`, async ({ page }) => {
      await start(page, false, true)
      await choose(page, review, trace)
      await page.locator(editor).fill(`策略传输验证 ${Number(review)}${Number(trace)}`)
      await page.locator(root).getByRole('button', { name: '发送', exact: true }).click()
      await expect.poll(async () => (await streamInputs(page)).length).toBe(1)
      const outgoing = (await streamInputs(page))[0] as {
        turnContext?: { directives?: string[]; workbench?: { reviewMode: boolean; artifactTrace: boolean } }
      }
      expect(outgoing.turnContext?.workbench).toEqual({ reviewMode: review, artifactTrace: trace })
      const directives: string[] = outgoing.turnContext?.directives ?? []
      expect(directives.some((line) => line.startsWith('自动审查：'))).toBe(false)
      expect(directives.some((line) => line.startsWith('产物追溯：'))).toBe(false)
      await test.info().attach('submitted-policy', {
        body: JSON.stringify({ review, trace, directives }),
        contentType: 'application/json'
      })
    })
  }

async function savedTurn(
  page: Page,
  message: OfficeMessage,
  reviewMode: boolean,
  artifactTrace: boolean,
  hostProof?: WorkbenchProof,
  followingMessages: OfficeMessage[] = []
) {
  const proof = hostProof
    ? {
        ...hostProof,
        sessionId: 'audit-session',
        messageId: message.id,
        sourceText: message.text,
        policy: { reviewMode, artifactTrace }
      }
    : null
  if (proof)
    message = {
      ...message,
      parts: [
        ...(message.parts ?? [{ type: 'text', text: message.text }]),
        { type: 'data-workbench-proof', id: 'workbench-proof', data: proof }
      ]
    }
  await installWorkbench(page, {
    withAgent: true,
    collab: true,
    workspaceFiles: Boolean(proof),
    sessions: [{ id: 'audit-session', name: '审查追溯专项任务', messages: [message, ...followingMessages] }]
  })
  if (proof)
    await page.route('**/api/agent-sessions/audit-session/**', async (route) => {
      const path = new URL(route.request().url()).pathname
      if (path.endsWith('/proofs')) return route.fulfill({ json: [proof] })
      if (path.includes('/proof-artifacts/'))
        return route.fulfill({
          json: {
            id: proof.artifacts[0].id,
            name: 'report.md',
            mediaType: 'text/markdown',
            contentDigest: proof.artifacts[0].contentDigest,
            data: Buffer.from('hello').toString('base64')
          }
        })
      if (path.endsWith('/proof')) {
        const action = route.request().postDataJSON()
        const finding = proof.review.findings.find((item) => item.id === action.findingId)
        if (finding) finding.status = action.action === 'accept' ? 'accepted' : 'waived'
        return route.fulfill({ json: proof })
      }
      return route.fallback()
    })
  await page.addInitScript(
    ({ key, id, reviewMode, artifactTrace }) => {
      localStorage.setItem(key, JSON.stringify({ pending: [], bound: { [id]: { reviewMode, artifactTrace } } }))
    },
    { key: ARMS, id: message.id, reviewMode, artifactTrace }
  )
  await page.goto('/windows/main/index.html', { waitUntil: 'domcontentloaded' })
  await page.getByRole('button', { name: '审查追溯专项任务', exact: true }).click()
  await expect(page.locator('[data-task-conversation-turn]').first()).toBeVisible()
}

test('UI-09 negated creation is not presented as an unperformed change', async ({ page }) => {
  await savedTurn(page, { id: 'audit-negative', role: 'assistant', text: 'I have not created any files.' }, true, false)
  await capture(page, 'negated-claim')
  await expect(page.locator('[data-task-auto-review-kind="change"]')).toHaveCount(0, { timeout: 2000 })
})

test('UI-13 composer has a named textbox for assistive technology', async ({ page }) => {
  await start(page)
  await capture(page, 'composer-accessibility')
  await expect(page.locator(root).getByRole('textbox')).toHaveAccessibleName(/.+/, { timeout: 1500 })
})

test('UI-14 review findings expose an actionable remediation path', async ({ page }) => {
  await savedTurn(
    page,
    { id: 'audit-actionable', role: 'assistant', text: '已创建报告。' },
    true,
    false,
    findingProof()
  )
  const review = page.locator('[data-task-auto-review]')
  await expect(review).not.toHaveAttribute('data-task-auto-review-count', '0')
  await capture(page, 'finding-actions')
  expect(await review.getByRole('button').count()).toBeGreaterThan(0)
})

test('UI-15 review on with trace off does not arm provenance UI', async ({ page }) => {
  await savedTurn(
    page,
    { id: 'audit-review-only', role: 'assistant', text: 'I have not created any files.' },
    true,
    false
  )
  await expect(page.locator('[data-task-auto-review]')).toHaveCount(0)
  await expect(page.locator('[data-task-lineage-open]')).toHaveCount(0)
})

test('UI-16 trace on with review off does not create a review certificate', async ({ page }) => {
  await savedTurn(
    page,
    {
      id: 'audit-trace-only',
      role: 'assistant',
      text: '完成。',
      parts: [
        {
          type: 'tool-write',
          toolCallId: 'audit-trace-only',
          state: 'output-available',
          input: { file_path: '/workspace/report.md', content: 'hello' },
          output: { path: '/workspace/report.md' }
        }
      ]
    },
    false,
    true,
    makeWorkbenchProof()
  )
  await expect(page.locator('[data-task-lineage-open]')).toBeVisible()
  await expect(page.locator('[data-task-auto-review]')).toHaveCount(0)
})

test('UI-17 historical source citations are not reported as unwritten artifacts', async ({ page }) => {
  await savedTurn(
    page,
    { id: 'audit-source', role: 'assistant', text: '原始资料在 /workspace/source.csv 。本轮只做阅读。' },
    true,
    false
  )
  await capture(page, 'source-citation')
  await expect(page.locator('[data-task-auto-review-kind="citation"]')).toHaveCount(0, { timeout: 1500 })
})

test('UI-18 older messages do not consume a newly armed review policy', async ({ page }) => {
  await installWorkbench(page, {
    withAgent: true,
    collab: true,
    sessions: [
      {
        id: 'audit-session',
        name: '审查追溯专项任务',
        messages: [{ id: 'audit-history', role: 'assistant', text: '一条早于发送时间的历史回复。' }]
      }
    ]
  })
  await page.addInitScript((key) => {
    localStorage.setItem(
      key,
      JSON.stringify({
        pending: [{ at: Date.parse('2026-10-07T00:00:00.500Z'), reviewMode: true, artifactTrace: true }],
        bound: {}
      })
    )
  }, ARMS)
  await page.goto('/windows/main/index.html', { waitUntil: 'domcontentloaded' })
  await page.getByRole('button', { name: '审查追溯专项任务', exact: true }).click()
  await expect(page.locator('[data-task-conversation-turn]')).toBeVisible()
  await capture(page, 'history-policy-theft')
  await expect(page.locator('[data-task-auto-review]')).toHaveCount(0, { timeout: 1500 })
})

test('UI-19 permission menu offers all postures and preserves text', async ({ page }) => {
  await start(page)
  await page.locator(editor).fill('打开授权菜单时保留的正文')
  await page.locator(root).locator('[data-workbench-chip="authorize"]').click()
  const menu = page.locator('[data-workbench-menu="authorize"]')
  await expect(menu.getByRole('menuitemradio')).toHaveCount(4)
  await expect(menu.locator('[data-posture="always-approve"]')).toContainText('高风险')
  await page.keyboard.press('Escape')
  await expect.poll(() => composerText(page)).toBe('打开授权菜单时保留的正文')
})

test('UI-20 model picker can search, close and return to the draft', async ({ page }) => {
  await start(page)
  await page.locator(editor).fill('模型菜单旁的草稿')
  await page.locator(root).getByRole('button', { name: '自动', exact: true }).click()
  const picker = page.locator('[data-workbench-model-picker="true"]')
  await expect(picker).toBeVisible()
  await picker.getByRole('textbox').fill('无此模型-audit')
  await expect(picker.getByRole('option')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect.poll(() => composerText(page)).toBe('模型菜单旁的草稿')
})

test('UI-21 the native minimum window keeps input and send inside the viewport', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await start(page)
  for (const target of [page.locator(editor), page.locator(root).getByRole('button', { name: '发送', exact: true })]) {
    const box = await target.boundingBox()
    expect(box).not.toBeNull()
    expect(box!.x).toBeGreaterThanOrEqual(0)
    expect(box!.x + box!.width).toBeLessThanOrEqual(960)
    expect(box!.y + box!.height).toBeLessThanOrEqual(640)
  }
  await capture(page, 'native-minimum-window')
})

test('UI-22 long multilingual input reaches the request without truncation', async ({ page }) => {
  await start(page, false, true)
  const draft = `${'长文本 English 👩🏽‍💻 <tag> & 符号。\n'.repeat(650)}结尾标记 AUDIT_END`
  await page.locator(editor).fill(draft)
  await page.locator(root).getByRole('button', { name: '发送', exact: true }).click()
  await expect.poll(async () => (await streamInputs(page)).length).toBe(1)
  const sent = (await streamInputs(page))[0] as { userMessageParts: { type: string; text?: string }[] }
  expect(
    sent.userMessageParts
      .filter((part) => part.type === 'text')
      .map((part) => part.text)
      .join('')
  ).toBe(draft)
})

test('UI-23 pasting HTML preserves safe text without executing it', async ({ page }) => {
  await start(page)
  await page.locator(editor).focus()
  await page.locator(editor).evaluate((node) => {
    const data = new DataTransfer()
    data.setData('text/plain', '粘贴正文 https://example.test/ <script>literal</script>')
    data.setData(
      'text/html',
      '<p>粘贴正文 <a href="https://example.test/">链接</a><img src="invalid-audit-image" onerror="globalThis.__COMPOSER_PASTE_EXECUTED__=true"><script>globalThis.__COMPOSER_PASTE_EXECUTED__=true</script></p>'
    )
    node.dispatchEvent(new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: data }))
  })
  await expect.poll(() => composerText(page)).toContain('粘贴正文')
  await expect(page.locator(editor).locator('script,[onerror]')).toHaveCount(0)
  expect(
    await page.evaluate(
      () => (globalThis as typeof globalThis & { __COMPOSER_PASTE_EXECUTED__?: boolean }).__COMPOSER_PASTE_EXECUTED__
    )
  ).not.toBe(true)
})

test('UI-24 whitespace and clearing the draft cannot submit an empty turn', async ({ page }) => {
  await start(page, false, true)
  const send = page.locator(root).getByRole('button', { name: '发送', exact: true })
  await page.locator(editor).fill('  \n\t  ')
  await expect(send).toBeDisabled()
  await page.locator(editor).fill('随后删除的正文')
  await expect(send).toBeEnabled()
  await page.locator(editor).press('Meta+a')
  await page.keyboard.press('Backspace')
  await expect(send).toBeDisabled()
  expect(await streamInputs(page)).toEqual([])
})

test('UI-10 an unsupported creation claim does not receive a clean-review certificate', async ({ page }) => {
  await savedTurn(
    page,
    {
      id: 'audit-false-clean',
      role: 'assistant',
      text: 'The final report now exists on disk and is ready for download.'
    },
    true,
    false,
    findingProof()
  )
  await capture(page, 'false-clean')
  await expect(page.locator('[data-task-auto-review]')).not.toHaveAttribute('data-task-auto-review-count', '0', {
    timeout: 2000
  })
})

test('UI-11 a path-only successful tool does not certify verified provenance', async ({ page }) => {
  await savedTurn(
    page,
    {
      id: 'audit-no-receipt',
      role: 'assistant',
      text: '文件准备完成。',
      parts: [
        { type: 'text', text: '文件准备完成。' },
        {
          type: 'tool-write',
          toolCallId: 'audit-write',
          state: 'output-available',
          input: { file_path: '/workspace/report.md', content: 'hello' },
          output: { path: '/workspace/report.md' }
        }
      ]
    },
    false,
    true
  )
  await capture(page, 'path-only-lineage')
  await expect(page.locator('[data-task-lineage-open]')).toHaveCount(0)
  await expect(page.locator('[data-task-lineage]')).toHaveCount(0)
})

test('UI-12 provenance inspector exposes versioned byte evidence and execution authority', async ({ page }) => {
  await savedTurn(
    page,
    {
      id: 'audit-inspector',
      role: 'assistant',
      text: '文件准备完成。',
      parts: [
        { type: 'text', text: '文件准备完成。' },
        {
          type: 'tool-write',
          toolCallId: 'audit-inspect-write',
          state: 'output-available',
          input: { file_path: '/workspace/report.md', content: 'hello' },
          output: { path: '/workspace/report.md' }
        }
      ]
    },
    true,
    true,
    makeWorkbenchProof()
  )
  await page.locator('[data-task-lineage-open]').click()
  // UI parity includes the task-workspace host and all seven inspection tabs.
  const workspace = page.locator('section[aria-label="任务工作区"], aside[aria-label="任务工作区"]')
  await expect.soft(workspace).toBeVisible({ timeout: 1000 })
  for (const name of ['产物', '证据', '对话', '执行', '代码', '环境', '审查']) {
    await expect.soft(workspace.getByRole('tab', { name, exact: true })).toBeVisible({ timeout: 500 })
  }
  const inspector = page.locator('[data-task-lineage]')
  await expect.soft(inspector).toContainText(/SHA-?256|digest|摘要/iu, { timeout: 1500 })
  await expect.soft(inspector).toContainText(/version|版本/iu, { timeout: 1500 })
  await expect.soft(inspector).toContainText(/execution|执行|回执|receipt/iu, { timeout: 1500 })
  const panel = workspace.getByRole('tabpanel')
  await expect(panel).toContainText('hello')
  const pane = page.locator('[data-right-pane]').filter({ has: workspace })
  await expect(pane).toHaveAttribute('data-right-pane-phase', 'docked')
  await expect(pane).toHaveCSS('opacity', '1')
  await expect(pane).toBeInViewport()
  await capture(page, 'inspector-artifact')
  await workspace.getByRole('tab', { name: '证据', exact: true }).click()
  await expect(panel).toContainText(makeWorkbenchProof().artifacts[0].contentDigest)
  await capture(page, 'inspector-evidence')
  await workspace.getByRole('tab', { name: '对话', exact: true }).click()
  await expect(panel).toContainText('Write a report')
  await expect(panel).toContainText('文件准备完成。')
  await workspace.getByRole('tab', { name: '执行', exact: true }).click()
  await expect(panel).toContainText('run-1')
  await workspace.getByRole('tab', { name: '代码', exact: true }).click()
  await expect(panel).toContainText('file_path')
  await workspace.getByRole('tab', { name: '环境', exact: true }).click()
  await expect(panel).toContainText('fixture')
  await workspace.getByRole('tab', { name: '审查', exact: true }).click()
  await expect(panel).not.toBeEmpty()
  await workspace.getByRole('tab', { name: '审查', exact: true }).press('ArrowLeft')
  await expect(workspace.getByRole('tab', { name: '环境', exact: true })).toBeFocused()
  await capture(page, 'inspector-scope')
})

test('UI-25 review without an independent result does not add a clean-success card', async ({ page }) => {
  await savedTurn(page, { id: 'audit-quiet-review', role: 'assistant', text: '2 + 2 = 4。' }, true, false)
  await capture(page, 'unexpected-clean-review-card')
  await expect(page.locator('[data-task-auto-review]')).toHaveCount(0, { timeout: 1500 })
})

test('UI-26 answer scan remains visible throughout a live review', async ({ page }) => {
  await savedTurn(
    page,
    { id: 'audit-scanning', role: 'assistant', text: 'The result is ready for independent review.' },
    true,
    false,
    makeWorkbenchProof({ review: { status: 'running', reviewer: 'independent-model', findings: [] } })
  )
  const prose = page.locator('[data-review-scanning] [data-task-prose]')
  await expect(prose).toBeVisible()
  await expect
    .poll(() =>
      prose.evaluate((node) => {
        const style = getComputedStyle(node, '::after')
        return [style.animationName, style.animationIterationCount, style.height]
      })
    )
    .toEqual(['studio-review-scan', 'infinite', '2px'])
  await page.waitForTimeout(1600)
  await expect(page.locator('[data-review-scanning]')).toHaveCount(1)
  await capture(page, 'live-review-column-scan')
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await expect.poll(() => prose.evaluate((node) => getComputedStyle(node, '::after').animationName)).toBe('none')
  await expect(page.getByRole('button', { name: '取消', exact: true })).toBeEnabled()
})

test('UI-27 quote annotations use a connected rail and survive collapse without replaying a scan', async ({ page }) => {
  await page.setViewportSize({ width: 1800, height: 1000 })
  await savedTurn(
    page,
    { id: 'audit-annotation', role: 'assistant', text: 'I created report. This statement requires verification.' },
    true,
    false,
    findingProof(),
    [{ id: 'after-annotation', role: 'assistant', text: 'Following message stays in place.' }]
  )
  const review = page.locator('[data-task-auto-review]')
  await expect(page.locator('[data-task-conversation-turn]')).toHaveCount(2)
  await expect(review).toHaveAttribute('data-task-review-layout', 'rail')
  await expect(page.locator('[data-task-review-connector="finding-1"]')).toHaveCount(1)
  const annotation = page.locator('[data-task-review-finding="finding-1"]')
  await annotation.focus()
  await expect(annotation).toHaveAttribute('data-finding-focused', '')
  await expect
    .poll(() =>
      page.evaluate(() =>
        Array.from(CSS.highlights.entries())
          .filter(([name]) => name.endsWith('-focus'))
          .flatMap(([, highlight]) => Array.from(highlight, (range) => range.toString()))
      )
    )
    .toEqual(['report'])
  const rail = await review.boundingBox()
  expect(rail).not.toBeNull()
  expect(rail!.x + rail!.width).toBeLessThanOrEqual(1800)
  await capture(page, 'review-annotation-rail')
  const messageGeometry = () =>
    page.locator('[data-task-conversation-turn]').evaluateAll((nodes) =>
      nodes.map((node) => ({
        top: node.getBoundingClientRect().top,
        height: node.getBoundingClientRect().height,
        scrollHeight: node.scrollHeight,
        scrollTop: node.scrollTop
      }))
    )
  const geometry = await messageGeometry()
  expect(await review.evaluate((node) => node.closest('[data-task-conversation-turn]'))).toBeNull()
  expect(
    await page
      .locator('[data-task-review-floating]')
      .evaluate((node) => node.parentElement === document.body && getComputedStyle(node).position === 'fixed')
  ).toBe(true)
  await review.locator('button[aria-expanded]').click()
  await expect(annotation).toHaveCount(0)
  await expect
    .poll(() =>
      page.evaluate(() =>
        Array.from(CSS.highlights.entries())
          .filter(([name]) => name.endsWith('-warning'))
          .flatMap(([, highlight]) => Array.from(highlight, (range) => range.toString()))
      )
    )
    .toEqual(['report'])
  await expect(page.locator('[data-review-scanning]')).toHaveCount(0)
  await expect.poll(messageGeometry).toEqual(geometry)
  await review.locator('button[aria-expanded]').click()
  await expect.poll(messageGeometry).toEqual(geometry)
  await page.setViewportSize({ width: 960, height: 640 })
  await expect(review).toHaveAttribute('data-task-review-layout', 'overlay')
  await expect(annotation).toBeVisible()
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  await capture(page, 'review-annotation-narrow')
  const narrowGeometry = await messageGeometry()
  await review.locator('button[aria-expanded]').click()
  await expect.poll(messageGeometry).toEqual(narrowGeometry)
})
