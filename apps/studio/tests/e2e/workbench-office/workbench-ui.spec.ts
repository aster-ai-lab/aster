import { expect, test, type Locator, type Page } from '@playwright/test'

import { composerText, installWorkbench, openHome, type WorkbenchInstall } from './fixture'
import { UI_PROJECT, type WorkbenchUiSession } from './uiFixture'

const HOME = '/windows/main'
const SLASH_IDS = [
  'workbench-skill:skill-creator',
  'workbench-skill:schedule-task',
  'workbench-skill:stc-answer',
  'workbench-slash:/goal',
  'workbench-slash:/help',
  'workbench-slash:/tools',
  'workbench-slash:/mcp'
]
const OMITTED_SLASH = ['/compact', '/clear', '/cost', '/history', '/model']
const PERMISSIONS = ['逐次确认', '仅规划', '智能授权', '完全访问']
const INVALID_PROJECT = '名称不能包含 \\ / : * ? " < > | 等字符，也不能以点或空格结尾'
const DAY = Date.parse('2026-10-10T12:00:00.000Z')

function stamp(minutesAgo: number) {
  return new Date(DAY - minutesAgo * 60_000).toISOString()
}

function sidebarSessions(): WorkbenchUiSession[] {
  const recent = Array.from({ length: 49 }, (_, index) => {
    const number = String(index + 1).padStart(2, '0')
    return {
      id: `recent-${number}`,
      name: `最近任务 ${number}`,
      place: 'inbox' as const,
      updatedAt: stamp(3 + index)
    }
  })
  return [
    { id: 'pinned-task', name: '置顶任务', pinned: true, place: 'inbox', updatedAt: stamp(1) },
    {
      id: 'project-task',
      name: '项目任务',
      place: 'project',
      projectId: UI_PROJECT.id,
      updatedAt: stamp(2)
    },
    { id: 'drag-task', name: '拖拽任务', place: 'inbox', updatedAt: stamp(0) },
    ...recent,
    { id: 'overflow-task', name: '落选任务', place: 'inbox', updatedAt: stamp(400) }
  ]
}

async function start(page: Page, ui: WorkbenchInstall['ui']) {
  await installWorkbench(page, { ui })
  await openHome(page)
  await expect(page.locator('[data-ui="workbench.home"]')).toBeVisible()
  await expect(page).toHaveURL(new RegExp(HOME))
}

async function httpCalls(page: Page) {
  return page.evaluate(() => {
    const host = globalThis as typeof globalThis & {
      __E2E_HTTP__?: { method: string; url: string; body: string }[]
    }
    return host.__E2E_HTTP__ ?? []
  })
}

async function taskCreates(page: Page) {
  const calls = await httpCalls(page)
  return calls.flatMap((call) => {
    if (call.method !== 'POST' || !/\/api\/agent-tasks(?:\?|$)/.test(call.url)) return []
    try {
      const body = JSON.parse(call.body) as { agentId?: string; name?: string; prompt?: string }
      return [body]
    } catch {
      return [{}]
    }
  })
}

async function releaseHeldComposer(page: Page) {
  await page.evaluate(() => {
    const host = globalThis as typeof globalThis & {
      __E2E_HOLD_COMPOSER__?: boolean
      __E2E_HELD_COMPOSER__?: Node[]
    }
    host.__E2E_HOLD_COMPOSER__ = false
    const composer = document.querySelector('[data-ui="workbench.composer"]')
    const held = host.__E2E_HELD_COMPOSER__ ?? []
    if (composer) {
      for (const node of held) {
        if (node instanceof Node && node.parentNode !== composer) composer.appendChild(node)
      }
    }
    held.length = 0
  })
}

async function dragSession(page: Page, from: Locator, to: Locator) {
  await from.scrollIntoViewIfNeeded()
  await to.scrollIntoViewIfNeeded()
  const start = await from.evaluate((node) => {
    const rect = node.getBoundingClientRect()
    return { x: rect.x + Math.min(80, rect.width / 2), y: rect.y + rect.height / 2 }
  })
  const end = await to.evaluate((node) => {
    const rect = node.getBoundingClientRect()
    return { x: rect.x + Math.min(48, rect.width / 2), y: rect.y + Math.min(16, rect.height / 2) }
  })
  await page.mouse.move(start.x, start.y)
  await page.mouse.down()
  await page.mouse.move(start.x + 24, start.y, { steps: 8 })
  await page.mouse.move(end.x, end.y, { steps: 16 })
  await page.mouse.up()
}

async function expectParkedSidebar(page: Page) {
  const sidebar = page.locator('#workbench-sidebar')
  await expect(sidebar).toHaveAttribute('aria-hidden', 'true')
  await expect.poll(async () => sidebar.evaluate((node) => getComputedStyle(node).left)).toBe('-260px')
}

test('composer home input, slash menu, permission postures, and task draft', async ({ page }) => {
  await start(page, {
    sessions: [{ id: 'session-open', name: '打开任务', place: 'inbox', updatedAt: stamp(0) }]
  })
  const composer = page.locator('[data-ui="workbench.composer"] .composer-tiptap')
  await expect(composer).toBeVisible()
  await expect(page.locator('[data-ui="workbench.composer"] [aria-label="语音输入"]')).toHaveCount(0)
  await expect(page.getByText('深度研究')).toHaveCount(0)

  await page.locator('[data-feature="write"] [data-writing-type="notice"]').click()
  const writing = page.getByRole('dialog')
  await expect(writing).toHaveAttribute('data-writing-stage', 'brief')
  await expect(writing.getByRole('textbox', { name: '写作说明' })).toHaveValue('发布需要周知或执行的事项。')
  await expect.poll(() => composerText(page)).toBe('')
  await writing.getByRole('button', { name: '关闭' }).click()
  await expect(writing).toHaveCount(0)

  await composer.click()
  await page.keyboard.insertText('季度草稿')
  await expect.poll(() => composerText(page)).toContain('季度草稿')
  await page.locator('[data-new-task-trigger="true"]').click()
  await expect.poll(() => composerText(page)).toBe('')
  await expect(page.locator('[data-ui="workbench.canvas"]')).toBeVisible()
  await expect(page).toHaveURL(new RegExp(HOME))

  await composer.click()
  await page.keyboard.press('/')
  const panel = page.getByTestId('quick-panel')
  await expect(panel).toBeVisible()
  await expect
    .poll(async () =>
      panel.locator('[data-id]').evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-id')))
    )
    .toEqual(SLASH_IDS)
  for (const command of OMITTED_SLASH) {
    await expect(panel.locator(`[data-id="workbench-slash:${command}"]`)).toHaveCount(0)
  }
  await page.keyboard.press('Escape')

  await page.locator('[data-ui="workbench.composer"] [data-workbench-chip="options"]').click()
  const options = page.locator('[data-workbench-menu="options"]')
  const review = options.getByRole('switch', { name: '自动审查' })
  const trace = options.getByRole('switch', { name: '产物追溯' })
  const reviewWas = await review.isChecked()
  const traceWas = await trace.isChecked()
  await review.click()
  await trace.click()
  await expect(review).toBeChecked({ checked: !reviewWas })
  await expect(trace).toBeChecked({ checked: !traceWas })
  await page.keyboard.press('Escape')

  await page.locator('[data-ui="workbench.composer"] [data-workbench-chip="authorize"]').click()
  const permissions = page.locator('[data-workbench-menu="authorize"]')
  await expect(permissions.getByRole('menuitemradio')).toHaveCount(4)
  for (const label of PERMISSIONS) await expect(permissions).toContainText(label)
  await expect(permissions.locator('[data-posture="always-approve"]')).toContainText('高风险')
  await page.keyboard.press('Escape')

  await page
    .locator('[data-ui="workbench.tasks"] [data-session-id="session-open"]')
    .locator('button')
    .filter({ hasText: '打开任务' })
    .click()
  await expect(page.locator('[data-ui="workbench.canvas"]')).toHaveCount(0)
  await expect(page.locator('[data-ui="workbench.composer.anchor"]')).toHaveCount(0)
  await expect(page.locator('[data-ui="workbench.home"]')).toBeVisible()
  await expect(page).toHaveURL(new RegExp(HOME))
})

test('composer file drop marks the fallback and stages the file', async ({ page }) => {
  await start(page, { holdComposer: true, sessions: [] })
  const fallback = page.locator('[data-ui="workbench.composer.fallback"]')
  await expect(fallback).toBeVisible()
  const dropped = await page.evaluate(() => {
    const shell = document.querySelector('[data-ui="workbench.composer.fallback"]')
    if (!shell) return { ok: false, reason: 'fallback input is missing' }
    const transfer = new DataTransfer()
    transfer.items.add(new File(['hello'], 'note.txt', { type: 'text/plain' }))
    const files = transfer.files.length
    const types = Array.from(transfer.types)
    if (files === 0 && !types.includes('Files')) {
      return { ok: false, reason: 'browser DataTransfer accepted no file', files, types }
    }
    const host = globalThis as typeof globalThis & { __E2E_FILE_DROP__?: DataTransfer }
    host.__E2E_FILE_DROP__ = transfer
    for (const type of ['dragenter', 'dragover']) {
      shell.dispatchEvent(new DragEvent(type, { bubbles: true, cancelable: true, dataTransfer: transfer }))
    }
    return { ok: true, files, types }
  })
  if (!dropped.ok || dropped.files === 0) {
    throw new Error(`browser could not inject a file drop: ${JSON.stringify(dropped)}`)
  }
  await expect(fallback).toHaveAttribute('data-drag-over', 'true')
  await expect(fallback.locator('[data-ui="composer.file-drop"]')).toContainText('松开以添加文件')
  await page.evaluate(() => {
    const shell = document.querySelector('[data-ui="workbench.composer.fallback"]')
    const host = globalThis as typeof globalThis & { __E2E_FILE_DROP__?: DataTransfer }
    const transfer = host.__E2E_FILE_DROP__
    if (!shell || !transfer) throw new Error('file drop was not prepared')
    shell.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: transfer }))
  })
  await releaseHeldComposer(page)
  await expect(page.locator('[data-ui="workbench.composer"]')).toContainText('note')
  await expect
    .poll(async () => {
      const calls = await httpCalls(page)
      return calls.some((call) => call.method === 'POST' && call.url.includes('/api/composer-files/note.txt'))
    })
    .toBe(true)
})

test('connector and skill catalogs keep new task and park the narrow sidebar', async ({ page }) => {
  await start(page, { sessions: [], skills: [] })
  await expect(page.locator('[data-new-task-trigger="true"]')).toBeVisible()

  await page.locator('[data-ui="workbench.nav.connectors"]').click()
  await expect(page.locator('[data-ui="workbench.nav.connectors"]')).toHaveAttribute('aria-current', 'page')
  await expect(page.locator('[data-ui="workbench.nav.skills"]')).not.toHaveAttribute('aria-current', 'page')
  await expect(page.locator('[data-ui="workbench.nav.experts"]')).toHaveCount(0)
  await expect(page.locator('[data-ui="capabilities.page"] h1')).toHaveText('连接器')
  await expect(page.locator('[data-ui="workbench.composer.anchor"]')).toHaveCount(0)
  await expect(page.locator('[data-new-task-trigger="true"]')).toBeVisible()
  await expect(page).toHaveURL(new RegExp(HOME))

  await page.locator('[data-ui="capabilities.create"]').click()
  const connectorDialog = page.getByRole('dialog', { name: '自定义连接器' })
  await expect(connectorDialog).toBeVisible()
  const save = connectorDialog.getByRole('button', { name: '保存' })
  await expect(save).toBeDisabled()
  await save.locator('xpath=..').click()
  await expect(connectorDialog.getByText('名称需要是小写字母、数字和连字符')).toBeVisible()
  await expect
    .poll(async () => {
      const calls = await httpCalls(page)
      return calls.filter((call) => call.method === 'PUT' && call.url.includes('/api/capabilities/custom')).length
    })
    .toBe(0)
  await connectorDialog.getByRole('button', { name: '关闭' }).click()

  await page.setViewportSize({ width: 760, height: 800 })
  await expectParkedSidebar(page)
  await expect(page.locator('[data-ui="capabilities.page"] h1')).toHaveText('连接器')
  await page.setViewportSize({ width: 1280, height: 800 })
  await expect(page.locator('#workbench-sidebar')).not.toHaveAttribute('aria-hidden', 'true')

  await page.locator('[data-ui="workbench.nav.skills"]').click()
  await expect(page.locator('[data-ui="workbench.nav.skills"]')).toHaveAttribute('aria-current', 'page')
  await expect(page.locator('[data-ui="workbench.nav.connectors"]')).not.toHaveAttribute('aria-current', 'page')
  await expect(page.locator('[data-ui="capabilities.page"] h1')).toHaveText('技能')
  await expect(page.getByText('还没有技能')).toBeVisible()
  await expect(page.locator('[data-ui="capabilities.card"]')).toHaveCount(0)
  await expect(page.getByText('邮箱')).toHaveCount(0)
  await expect(page.locator('[data-ui="workbench.composer.anchor"]')).toHaveCount(0)
  await expect(page.locator('[data-new-task-trigger="true"]')).toBeVisible()

  await page.setViewportSize({ width: 760, height: 800 })
  await expectParkedSidebar(page)
  await expect(page.locator('[data-ui="capabilities.page"] h1')).toHaveText('技能')
  await expect(page.getByText('还没有技能')).toBeVisible()
})

test('project create and task-list cap, drag, search, rename, and archive', async ({ page }) => {
  await start(page, { sessions: sidebarSessions() })
  const recent = page.locator('[data-ui="workbench.tasks"] [data-session-place="recent"]')
  await expect(recent).toHaveCount(50)
  await expect(page.locator('[data-ui="workbench.tasks"]').getByText('落选任务')).toHaveCount(0)
  await expect(page.locator('[data-ui="workbench.tasks"]').getByText('项目任务')).toHaveCount(0)
  await expect(page.locator('[data-session-id="project-task"][data-session-place="project"]')).toBeVisible()

  await page.locator('[data-ui="workbench.tasks.more"]').scrollIntoViewIfNeeded()
  await page.locator('[data-ui="workbench.tasks.more"]').click()
  const history = page.locator('[data-ui="workbench.tasks.history"]')
  await expect(history.locator('[data-session-place="history"]')).toHaveCount(51)
  await expect(history.getByText('落选任务')).toHaveCount(0)
  await history.getByRole('button', { name: '关闭' }).click()
  await expect(history).toHaveCount(0)

  const renameRow = page.locator('[data-ui="workbench.tasks"] [data-session-id="recent-01"]')
  await renameRow.scrollIntoViewIfNeeded()
  await renameRow.hover()
  await renameRow.getByRole('button', { name: '重命名' }).click()
  const renameField = renameRow.getByRole('textbox', { name: '重命名' })
  await renameField.fill('临时名称')
  await renameField.press('Escape')
  await expect(renameRow).toContainText('最近任务 01')
  await renameRow.hover()
  await renameRow.getByRole('button', { name: '重命名' }).click()
  await renameRow.getByRole('textbox', { name: '重命名' }).fill('周报任务')
  await renameRow.getByRole('textbox', { name: '重命名' }).press('Enter')
  await expect(renameRow).toContainText('周报任务')

  await page.locator('[data-project-action="create"]').click()
  const create = page.locator('[data-ui="workbench.projects.create"]')
  const name = create.getByRole('textbox', { name: '项目名称' })
  await name.fill('  季度汇报  ')
  await create.getByRole('button', { name: '创建' }).click()
  await expect(create).toHaveCount(0)
  await expect(page.locator('[data-ui="workbench.project"]').filter({ hasText: '季度汇报' })).toBeVisible()

  await page.locator('[data-project-action="create"]').click()
  await name.fill('季度/汇报')
  await expect(create.getByRole('alert')).toHaveText(INVALID_PROJECT)
  await expect(create.getByRole('button', { name: '创建' })).toBeDisabled()
  await name.fill('a3s')
  await expect(create.getByRole('alert')).toHaveText('已有同名项目，请换一个名称')
  await expect(create.getByRole('button', { name: '创建' })).toBeDisabled()
  await name.fill('草稿项目')
  await create.getByRole('button', { name: '取消' }).click()
  await expect(create).toHaveCount(0)
  await page.locator('[data-project-action="create"]').click()
  await expect(name).toHaveValue('')
  await create.getByRole('button', { name: '取消' }).click()
  await expect(page.locator('[data-slot="dialog-overlay"]')).toHaveCount(0)

  const project = page.locator(`[data-ui="workbench.project"][data-project-id="${UI_PROJECT.id}"]`)
  await dragSession(page, page.locator('[data-ui="workbench.tasks"] [data-session-id="drag-task"]'), project)
  const toggle = project.locator('[data-project-action="toggle"]')
  if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click()
  const moved = page.locator('[data-session-id="drag-task"][data-session-place="project"]')
  await expect(moved).toBeVisible()
  await dragSession(page, moved, page.locator('[data-ui="workbench.tasks"] header h3'))
  await expect(
    page.locator('[data-ui="workbench.tasks"] [data-session-id="drag-task"][data-session-place="recent"]')
  ).toBeVisible()

  const projectTask = page.locator('[data-session-id="project-task"][data-session-place="project"]')
  await projectTask.hover()
  await projectTask.getByRole('button', { name: '归档' }).click()
  await expect(page.getByText('归档这个任务？')).toBeVisible()
  await page.getByRole('button', { name: '取消', exact: true }).click()
  await expect(projectTask).toBeVisible()
  await expect
    .poll(async () => {
      const calls = await httpCalls(page)
      return calls.filter((call) => call.method === 'POST' && call.url.includes('/api/agent-sessions/archive')).length
    })
    .toBe(0)

  await page.locator('[data-ui="workbench.search.trigger"]').click()
  const search = page.locator('[data-ui="workbench.search"]')
  await search.locator('input').fill('最近任务 07')
  await expect(search.locator('[data-ui="workbench.search.result"]')).toHaveCount(1)
  await expect(recent).toHaveCount(50)
  await search.locator('input').press('Enter')
  await expect(page.locator('[data-session-id="recent-07"] [aria-current="page"]')).toBeVisible()
  await expect(page.locator('[data-ui="workbench.canvas"]')).toHaveCount(0)
  await expect(page).toHaveURL(new RegExp(HOME))
})

test('automation with an agent opens detail and records the saved prompt', async ({ page }) => {
  await start(page, {
    sessions: [],
    tasks: [{ id: 'task-weekly', name: '周报任务', prompt: '汇总本周进展' }]
  })
  await page.locator('[data-ui="workbench.nav.scheduled"]').click()
  const automation = page.locator('[data-ui="workbench.automation"]')
  await expect(automation).toBeVisible()
  await expect(automation.locator('h1').first()).toHaveText('自动化')
  await expect(page.locator('[data-ui="workbench.nav.scheduled"]')).toHaveAttribute('aria-current', 'page')
  await expect(page.locator('[data-ui="workbench.composer.anchor"]')).toHaveCount(0)
  await expect(page.locator('[data-new-task-trigger="true"]')).toBeVisible()
  await expect(page).toHaveURL(new RegExp(HOME))

  await automation.getByRole('button', { name: '周报任务' }).click()
  const detail = page.locator('[data-ui="workbench.automation.detail"]')
  await expect(detail).toBeVisible()
  await expect(detail).toContainText('汇总本周进展')
  await detail.getByRole('button', { name: '返回' }).click()
  await expect(detail).toHaveCount(0)
  await expect(automation).toBeVisible()

  await automation.getByRole('button', { name: '周报任务' }).click()
  await detail.locator('h1').click()
  await page.keyboard.press('Escape')
  await expect(detail).toHaveCount(0)
  await expect(automation).toBeVisible()
  await expect(page).toHaveURL(new RegExp(HOME))

  await automation.getByRole('button', { name: '新建' }).click()
  const manual = page.getByRole('menuitem', { name: '手动创建' })
  await expect(manual).toBeEnabled()
  await manual.click()
  const dialog = page.getByRole('dialog', { name: '新建定时任务' })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByText('按 Agent 筛选')).toHaveCount(0)
  await expect(dialog.getByText('绑定智能体')).toHaveCount(0)
  await dialog.locator('#task-form-name').fill('周报提醒')
  await dialog.locator('.composer-tiptap').click()
  await page.keyboard.insertText('周五前提交周报')
  await dialog.getByRole('button', { name: '保存' }).click()
  await expect
    .poll(async () => taskCreates(page))
    .toEqual([expect.objectContaining({ agentId: 'agent-work', name: '周报提醒', prompt: '周五前提交周报' })])
  await expect(detail).toBeVisible()
  await expect(detail).toContainText('周五前提交周报')
  await detail.getByRole('button', { name: '返回' }).click()
  await expect(detail).toHaveCount(0)
  await automation.getByRole('button', { name: '新建' }).click()
  await page.getByRole('menuitem', { name: '通过 Agent 创建' }).click()
  await expect(page.locator('[data-ui="workbench.canvas"]')).toBeVisible()
  await expect(page.locator('[data-ui="workbench.composer.anchor"]')).toBeVisible()
  await expect(page.locator('[data-ui="workbench.nav.scheduled"]')).not.toHaveAttribute('aria-current', 'page')
  await expect(page.locator('[data-ui="workbench.nav.experts"]')).toHaveCount(0)
  await expect(page).toHaveURL(new RegExp(HOME))
})

test('automation without an agent disables manual create', async ({ page }) => {
  await start(page, { withAgent: false, sessions: [], tasks: [] })
  await page.locator('[data-ui="workbench.nav.scheduled"]').click()
  const automation = page.locator('[data-ui="workbench.automation"]')
  await expect(automation.locator('h1').first()).toHaveText('自动化')
  await automation.getByRole('button', { name: '新建' }).click()
  await expect(page.getByRole('menuitem', { name: '手动创建' })).toBeDisabled()
  await page.evaluate(() => {
    const host = globalThis as typeof globalThis & { __E2E_ROUTES__?: string[] }
    host.__E2E_ROUTES__ = []
    window.addEventListener('cherry:open-main-route', (event) => {
      const path = (event as CustomEvent<{ path?: string }>).detail?.path
      if (typeof path === 'string') host.__E2E_ROUTES__?.push(path)
    })
  })
  await page.getByRole('menuitem', { name: '通过 Agent 创建' }).click()
  await expect
    .poll(async () =>
      page.evaluate(() => (globalThis as typeof globalThis & { __E2E_ROUTES__?: string[] }).__E2E_ROUTES__ ?? [])
    )
    .toEqual(['/app/agents'])
  await expect(page.locator('[data-ui="workbench.automation"]')).toHaveCount(0)
  await expect(page.locator('[data-ui="workbench.canvas"]')).toBeVisible()
  await expect(page.locator('[data-ui="workbench.composer.anchor"]')).toBeVisible()
  await expect(page.locator('[data-ui="workbench.nav.scheduled"]')).not.toHaveAttribute('aria-current', 'page')
  await expect(page.locator('[data-ui="workbench.nav.experts"]')).toHaveCount(0)
  await expect(page.locator('[data-ui="capabilities.page"]')).toHaveCount(0)
  await expect(page).toHaveURL(new RegExp(HOME))
})
