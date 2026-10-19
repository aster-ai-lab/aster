import { expect, test, type Page } from '@playwright/test'

import { installWorkbench } from '../workbench-office/fixture'

// Real renderer/menu/serialization; clipboard, native dialogs and external services are controlled boundaries.
const answer = 'Toolbar answer **bold**.'
const reply = (page: Page) => page.locator('[data-message-id="toolbar-answer"]')

interface ToolbarState {
  saved: { name: string; text: string }[]
  clipboard: string[]
  ipc: { route: string; input: unknown }[]
  images: { width: number; height: number }[]
  cancel: boolean
}

async function start(page: Page, withHome = true) {
  await installWorkbench(page, {
    withAgent: true,
    collab: true,
    workspaceFiles: true,
    ...(withHome
      ? {
          ui: {
            withAgent: true,
            sessions: [{ id: 'session-writing', name: '消息工具栏验收', place: 'inbox' as const }]
          }
        }
      : {}),
    createdMessages: [
      { id: 'toolbar-user', role: 'user', text: 'Toolbar question' },
      {
        id: 'toolbar-answer',
        role: 'assistant',
        text: answer,
        parts: [
          { type: 'reasoning', text: 'Reasoning only on explicit export.' },
          { type: 'text', text: answer }
        ]
      }
    ],
    sessions: [{ id: 'session-writing', name: '消息工具栏验收', messages: [] }]
  })
  await page.addInitScript(() => {
    const host = globalThis as typeof globalThis & { __A3S_PREFERENCE_DEFAULTS__?: Record<string, unknown> }
    let defaults = host.__A3S_PREFERENCE_DEFAULTS__ ?? {}
    const options = Object.fromEntries(
      [
        'image',
        'markdown',
        'markdown_reason',
        'notion',
        'yuque',
        'joplin',
        'obsidian',
        'siyuan',
        'docx',
        'plain_text'
      ].map((key) => [`data.export.menus.${key}`, true])
    )
    Object.defineProperty(host, '__A3S_PREFERENCE_DEFAULTS__', {
      configurable: true,
      get: () => ({ ...defaults, ...options, 'chat.message.confirm_delete': true }),
      set: (value) => {
        defaults = value ?? {}
      }
    })
  })
  await page.goto('/windows/main/index.html', { waitUntil: 'domcontentloaded' })
  await page.getByRole('button', { name: '消息工具栏验收', exact: true }).click()
  await expect(reply(page)).toContainText('Toolbar answer')
  await page.evaluate(() => {
    const host = globalThis as typeof globalThis & { __TOOLBAR_TEST__?: ToolbarState }
    const state: ToolbarState = { saved: [], clipboard: [], ipc: [], images: [], cancel: false }
    host.__TOOLBAR_TEST__ = state
    window.api.file.save = async (name: string, text: string) => {
      state.saved.push({ name, text })
      return state.cancel ? null : `/tmp/${name}`
    }
    window.api.file.saveImage = async (_name: string, dataUrl: string) => {
      const encoded = dataUrl.slice(dataUrl.indexOf(',') + 1)
      const image = await createImageBitmap(
        new Blob([Uint8Array.from(atob(encoded), (char) => char.charCodeAt(0))], { type: 'image/png' })
      )
      state.images.push({ width: image.width, height: image.height })
      image.close()
      return true
    }
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          state.clipboard.push(text)
        },
        write: async (items: ClipboardItem[]) => {
          for (const item of items) {
            if (item.types.includes('text/plain')) state.clipboard.push(await (await item.getType('text/plain')).text())
            if (item.types.includes('image/png')) {
              const image = await createImageBitmap(await item.getType('image/png'))
              state.images.push({ width: image.width, height: image.height })
              image.close()
            }
          }
        }
      }
    })
    const request = window.api.ipcApi.request.bind(window.api.ipcApi)
    window.api.ipcApi.request = async (route, input) => {
      state.ipc.push({ route, input })
      if (route === 'export.word.from_markdown') return { ok: true, data: true }
      return request(route, input)
    }
  })
}

test('MA-07 missing workspace refuses note saving and leaves the source available', async ({ page }) => {
  await start(page, false)
  await reply(page).getByRole('button', { name: '保存到笔记', exact: true }).click()
  await expect(page.getByText('导出笔记失败', { exact: true })).toBeVisible()
  await expect(reply(page)).toContainText('Toolbar answer')
  await expect(reply(page).getByRole('button', { name: '保存到笔记', exact: true })).toBeEnabled()
})

async function state(page: Page) {
  return page.evaluate(() => (globalThis as typeof globalThis & { __TOOLBAR_TEST__: ToolbarState }).__TOOLBAR_TEST__)
}

async function more(page: Page) {
  await reply(page).getByRole('button', { name: '更多操作', exact: true }).click()
  await expect(page.getByRole('menuitem', { name: '多选', exact: true })).toBeVisible()
}

async function expectSubmenuHitTarget(page: Page, label: string) {
  const item = page.getByRole('menuitem', { name: label, exact: true })
  await expect(item).toBeVisible()
  await expect
    .poll(() =>
      item.evaluate((element) => {
        const rect = element.getBoundingClientRect()
        const hit = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2)
        return hit !== null && element.contains(hit)
      })
    )
    .toBe(true)
}

test('MA-15 mouse-opened submenus paint outside the parent clip and accept real pointer clicks', async ({ page }) => {
  await start(page)
  await page.setViewportSize({ width: 960, height: 640 })
  await more(page)
  await page.getByRole('menuitem', { name: '保存', exact: true }).hover()
  await expectSubmenuHitTarget(page, '保存到笔记')
  await page.screenshot({ path: test.info().outputPath('save-submenu-hit-target.png') })
  await page.getByRole('menuitem', { name: '保存到本地文件', exact: true }).click()
  await expect.poll(async () => (await state(page)).saved.length).toBe(1)
  await more(page)
  await page.getByRole('menuitem', { name: '导出', exact: true }).hover()
  await expectSubmenuHitTarget(page, '导出为 Word')
  await expectSubmenuHitTarget(page, '复制为纯文本（去除 Markdown）')
  await page.screenshot({ path: test.info().outputPath('export-submenu-hit-target.png') })
  await page.getByRole('menuitem', { name: '复制为纯文本（去除 Markdown）', exact: true }).click()
  await expect.poll(async () => (await state(page)).clipboard.length).toBe(1)
})

test('MA-16 message menus stay opaque when background animation playback is suspended', async ({ page }) => {
  await start(page)
  await page.addStyleTag({ content: '.message-action-menu { animation-play-state: paused !important; }' })
  await more(page)
  await expect(page.getByRole('menu')).toHaveCSS('opacity', '1')
  await page.getByRole('menuitem', { name: '导出', exact: true }).hover()
  await expect(page.getByRole('menu').last()).toHaveCSS('opacity', '1')
  await expectSubmenuHitTarget(page, '导出为 Word')
})

test('MA-17 a closed menu dispatches its export even when animation frames stop', async ({ page }) => {
  await start(page)
  await more(page)
  await page.getByRole('menuitem', { name: '导出', exact: true }).hover()
  await expectSubmenuHitTarget(page, '导出为 Markdown')
  await page.evaluate(() => {
    window.requestAnimationFrame = () => 0
  })
  await page.getByRole('menuitem', { name: '导出为 Markdown', exact: true }).click()
  await expect.poll(async () => (await state(page)).saved.length).toBe(1)
  await expect(page.locator('[data-slot="dropdown-menu-content"]')).toHaveCount(0)
  await expect(page.locator('[data-slot="dropdown-menu-sub-content"]')).toHaveCount(0)
})

test('MA-18 multiselect export writes only the selection when background animations stop', async ({ page }) => {
  await start(page)
  await more(page)
  await page.getByRole('menuitem', { name: '多选', exact: true }).click()
  await page.getByRole('checkbox', { name: '全选', exact: true }).check()
  await page.addStyleTag({ content: '.message-action-menu { animation-play-state: paused !important; }' })
  await page.getByRole('button', { name: '导出', exact: true }).click()
  await expect(page.getByRole('menu')).toHaveCSS('opacity', '1')
  await expectSubmenuHitTarget(page, '导出为 Markdown')
  await page.evaluate(() => {
    window.requestAnimationFrame = () => 0
  })
  await page.getByRole('menuitem', { name: '导出为 Markdown', exact: true }).click()
  await expect.poll(async () => (await state(page)).saved.length).toBe(1)
  const exported = (await state(page)).saved[0].text
  expect(exported).toContain('Toolbar question')
  expect(exported).toContain(answer)
  expect(exported).not.toContain('Reasoning only')
  await expect(page.getByRole('checkbox', { name: '全选', exact: true })).toHaveCount(0)
})

test('MA-01 toolbar copy and every export/save submenu are reachable with a keyboard', async ({ page }) => {
  await start(page)
  await reply(page).getByRole('button', { name: '复制', exact: true }).click()
  await expect.poll(async () => (await state(page)).clipboard).toEqual([answer])
  await more(page)
  const save = page.getByRole('menuitem', { name: '保存', exact: true })
  await save.focus()
  await page.keyboard.press('ArrowRight')
  await expectSubmenuHitTarget(page, '保存到笔记')
  await page.keyboard.press('ArrowLeft')
  await expect(save).toBeFocused()
  await page.keyboard.press('ArrowDown')
  await expect(page.getByRole('menuitem', { name: '导出', exact: true })).toBeFocused()
  await page.keyboard.press('ArrowRight')
  await expectSubmenuHitTarget(page, '导出为 Word')
  const items = await page.getByRole('menu').last().getByRole('menuitem').allTextContents()
  expect(items.join('|')).toMatch(/Markdown/)
  expect(items.join('|')).toMatch(/Word/)
  for (const name of ['Notion', '语雀', 'Obsidian', 'Joplin', '思源']) expect(items.join('|')).toContain(name)
  await page.screenshot({ path: test.info().outputPath('export-submenu.png') })
  await page.keyboard.press('Escape')
  await expect(page.getByRole('menu')).toHaveCount(0)
  await expect(reply(page).getByRole('button', { name: '更多操作', exact: true })).toBeFocused()
  expect((await state(page)).saved).toEqual([])
})

test('MA-02 save file writes the original message once and cancellation permits retry', async ({ page }) => {
  await start(page)
  for (const cancel of [true, false]) {
    await page.evaluate((cancel) => {
      ;(globalThis as typeof globalThis & { __TOOLBAR_TEST__: { cancel: boolean } }).__TOOLBAR_TEST__.cancel = cancel
    }, cancel)
    await more(page)
    await page.getByRole('menuitem', { name: '保存', exact: true }).hover()
    await page.getByRole('menuitem', { name: /保存到文件|保存为文件|文件$/, exact: false }).click()
    await expect.poll(async () => (await state(page)).saved.length).toBe(cancel ? 1 : 2)
  }
  for (const saved of (await state(page)).saved) {
    expect(saved.name).toMatch(/\.md$/)
    expect(saved.text).toBe(answer)
  }
})

test('MA-03 multiselect has named controls, starts empty and copies exactly the selection', async ({ page }) => {
  await start(page)
  await more(page)
  await page.getByRole('menuitem', { name: '多选', exact: true }).click()
  await expect(page.getByRole('button', { name: '导出', exact: true })).toBeDisabled()
  await page.getByRole('checkbox', { name: '全选', exact: true }).check()
  await expect(page.getByText('选中 2 条消息', { exact: true })).toBeVisible()
  const copy = page.getByRole('button', { name: '复制', exact: true }).filter({ visible: true })
  await copy.last().click()
  await expect.poll(async () => (await state(page)).clipboard.length).toBe(1)
  const copied = (await state(page)).clipboard[0]
  expect(copied).toContain('Toolbar question')
  expect(copied).toContain('Toolbar answer')
  expect(copied).not.toContain('Reasoning only')
  await expect(page.getByRole('checkbox', { name: '全选', exact: true })).toHaveCount(0)
})

for (const label of ['导出为 Markdown', '导出为 Markdown (包含思考)', '导出为 Word']) {
  test(`MA-05 ${label} exports the selected message with the requested reasoning policy`, async ({ page }) => {
    await start(page)
    await more(page)
    await page.getByRole('menuitem', { name: '导出', exact: true }).hover()
    await page.getByRole('menuitem', { name: label, exact: true }).click()
    if (label === '导出为 Word') {
      await expect
        .poll(async () => (await state(page)).ipc.filter((item) => item.route === 'export.word.from_markdown').length)
        .toBe(1)
      const payload = (await state(page)).ipc.find((item) => item.route === 'export.word.from_markdown')!.input as {
        markdown: string
        fileName: string
      }
      expect(payload.markdown).toContain(answer)
      expect(payload.markdown).not.toContain('Reasoning only')
      expect(payload.fileName).not.toBe('')
    } else {
      await expect.poll(async () => (await state(page)).saved.length).toBe(1)
      const exported = (await state(page)).saved[0]
      expect(exported.name).toMatch(/\.md$/)
      expect(exported.text).toContain(answer)
      expect(exported.text.includes('Reasoning only')).toBe(label.includes('包含思考'))
    }
    await expect(page.getByRole('menu')).toHaveCount(0)
  })
}

for (const entry of ['toolbar', 'submenu']) {
  test(`MA-06 notes ${entry} creates the real note body through the document boundary`, async ({ page }) => {
    await start(page)
    const writes: { path: string; text: string }[] = []
    await page.route('**/api/local-documents/entries**', async (route) => {
      if (route.request().method() !== 'POST') return route.fulfill({ json: { entries: [] } })
      const body = route.request().postDataJSON() as { parent: string; name: string; kind: string }
      return route.fulfill({
        json: {
          name: body.name,
          path: `${body.parent}/${body.name}`,
          kind: body.kind === 'folder' ? 'directory' : 'file',
          sizeBytes: 0,
          modifiedMs: 0
        }
      })
    })
    await page.route('**/api/workspace-file/**', async (route) => {
      if (route.request().method() !== 'PUT') return route.fallback()
      writes.push({
        path: new URL(route.request().url()).searchParams.get('path') ?? '',
        text: route.request().postDataBuffer()!.toString('utf8')
      })
      return route.fulfill({ status: 204 })
    })
    if (entry === 'toolbar') await reply(page).getByRole('button', { name: '保存到笔记', exact: true }).click()
    else {
      await more(page)
      await page.getByRole('menuitem', { name: '保存', exact: true }).hover()
      await page.getByRole('menuitem', { name: '保存到笔记', exact: true }).click()
    }
    await expect.poll(() => writes.length).toBe(1)
    expect(writes[0].path).toMatch(/\/wiki\/local\/[^/]+\.md$/)
    expect(writes[0].text).toContain(answer)
    expect(writes[0].text).not.toContain('Reasoning only')
  })
}

test('MA-04 delete confirmation can be cancelled without deleting a message', async ({ page }) => {
  await start(page)
  await reply(page).getByRole('button', { name: '删除', exact: true }).click()
  await expect(page.getByRole('dialog')).toBeVisible()
  await page.getByRole('button', { name: '取消', exact: true }).click()
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await expect(reply(page)).toContainText('Toolbar answer')
})

test('MA-08 copy plain text removes Markdown formatting', async ({ page }) => {
  await start(page)
  await more(page)
  await page.getByRole('menuitem', { name: '导出', exact: true }).hover()
  await page.getByRole('menuitem', { name: '复制为纯文本（去除 Markdown）', exact: true }).click()
  await expect.poll(async () => (await state(page)).clipboard.length).toBe(1)
  expect((await state(page)).clipboard[0]).toContain('Toolbar answer bold.')
  expect((await state(page)).clipboard[0]).not.toContain('**')
})

for (const label of ['导出为图片', '复制为图片']) {
  test(`MA-09 ${label} produces a decodable image from the actual message DOM`, async ({ page }) => {
    await start(page)
    await more(page)
    await page.getByRole('menuitem', { name: '导出', exact: true }).hover()
    await page.getByRole('menuitem', { name: label, exact: true }).click()
    await expect.poll(async () => (await state(page)).images.length).toBe(1)
    const image = (await state(page)).images[0]
    expect(image.width).toBeGreaterThan(100)
    expect(image.height).toBeGreaterThan(20)
    await expect(reply(page)).toContainText('Toolbar answer')
    await expect(reply(page).getByRole('button', { name: '复制', exact: true })).toBeEnabled()
  })
}

for (const [label, error] of [
  ['导出到 Notion', /Notion.*API|API.*Notion/],
  ['导出到语雀', /配置.*语雀|语雀.*配置/],
  ['导出到 Joplin', /配置.*Joplin|Joplin.*配置/],
  ['导出到思源笔记', /配置.*思源|思源.*配置/]
] as const) {
  test(`MA-10 unconfigured ${label} gives actionable feedback`, async ({ page }) => {
    await start(page)
    await more(page)
    await page.getByRole('menuitem', { name: '导出', exact: true }).hover()
    await page.getByRole('menuitem', { name: label, exact: true }).click()
    await expect(page.getByText(error)).toBeVisible()
    await expect(page.getByRole('menu')).toHaveCount(0)
    await expect(reply(page)).toContainText('Toolbar answer')
  })
}

test('MA-11 Obsidian opens its configuration dialog and cancellation keeps the source', async ({ page }) => {
  await start(page)
  await more(page)
  await page.getByRole('menuitem', { name: '导出', exact: true }).hover()
  await page.getByRole('menuitem', { name: '导出到 Obsidian', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('配置笔记属性')
  await dialog.getByRole('button', { name: '取消', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(reply(page)).toContainText('Toolbar answer')
})

test('MA-12 fork uses the selected message identity and failure allows retry', async ({ page }) => {
  await start(page)
  await page.evaluate(() => {
    const request = window.api.ipcApi.request.bind(window.api.ipcApi)
    window.api.ipcApi.request = async (route, input) => {
      if (route === 'ai.agent.session.fork') {
        ;(globalThis as typeof globalThis & { __TOOLBAR_TEST__: ToolbarState }).__TOOLBAR_TEST__.ipc.push({
          route,
          input
        })
        throw new Error('Owned fork probe failed')
      }
      return request(route, input)
    }
  })
  for (let count = 1; count <= 2; count += 1) {
    await more(page)
    await page.getByRole('menuitem', { name: '分叉', exact: true }).click()
    await expect
      .poll(async () => (await state(page)).ipc.filter((item) => item.route === 'ai.agent.session.fork').length)
      .toBe(count)
    await expect(page.getByText(/Owned fork probe failed/).first()).toBeVisible()
  }
  expect((await state(page)).ipc.find((item) => item.route === 'ai.agent.session.fork')!.input).toEqual({
    sourceSessionId: 'session-writing',
    messageId: 'toolbar-answer'
  })
  await expect(reply(page)).toContainText('Toolbar answer')
})

test('MA-13 confirmed deletion removes only the selected fixture message', async ({ page }) => {
  await start(page)
  let deleted = 0
  await page.route('**/api/agent-sessions/session-writing/messages/toolbar-answer', async (route) => {
    if (route.request().method() !== 'DELETE') return route.fallback()
    deleted += 1
    return route.fulfill({ status: 204 })
  })
  await reply(page).getByRole('button', { name: '删除', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: '删除', exact: true }).click()
  await expect.poll(() => deleted).toBe(1)
  await expect(reply(page)).toHaveCount(0)
  await expect(page.locator('[data-message-id="toolbar-user"]')).toContainText('Toolbar question')
})

test('MA-14 repeated menu navigation stays inside the minimum native window and causes no writes', async ({ page }) => {
  await start(page)
  await page.setViewportSize({ width: 960, height: 640 })
  const durations: number[] = []
  for (let cycle = 0; cycle < 5; cycle += 1) {
    const started = Date.now()
    await more(page)
    await page.getByRole('menuitem', { name: '导出', exact: true }).hover()
    const submenu = page.getByRole('menu').last()
    await expect(submenu.getByRole('menuitem', { name: '导出为 Word', exact: true })).toBeVisible()
    const bounds = await submenu.boundingBox()
    expect(bounds).not.toBeNull()
    expect(bounds!.x).toBeGreaterThanOrEqual(0)
    expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(960)
    expect(bounds!.y).toBeGreaterThanOrEqual(0)
    expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(640)
    durations.push(Date.now() - started)
    await page.keyboard.press('Escape')
    await page.keyboard.press('Escape')
  }
  await test.info().attach('menu-navigation-ms', { body: JSON.stringify(durations), contentType: 'application/json' })
  expect((await state(page)).saved).toEqual([])
  expect((await state(page)).images).toEqual([])
  expect(
    (await state(page)).ipc.filter((item) => item.route.startsWith('export.') || item.route === 'ai.agent.session.fork')
  ).toEqual([])
})
