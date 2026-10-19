import { expect, test, type Page } from '@playwright/test'

import {
  HELD_ID,
  HISTORY_ID,
  heldParts,
  installTurnState,
  openWorkbenchHome,
  pushTopicStatus
} from './fixture'

async function expectCollapsed(page: Page) {
  const bar = page.locator('#workbench-sidebar')
  await expect(bar).toHaveAttribute('aria-hidden', 'true')
  await expect.poll(async () => bar.evaluate((node) => getComputedStyle(node).left)).toBe('-260px')
}

async function expectExpanded(page: Page) {
  const bar = page.locator('#workbench-sidebar')
  await expect(bar).not.toHaveAttribute('aria-hidden', 'true')
  await expect.poll(async () => bar.evaluate((node) => getComputedStyle(node).left)).not.toBe('-260px')
}

async function expandSidebar(page: Page) {
  const toggle = page.locator('[data-ui="workbench.sidebar.toggle"]').last()
  if ((await page.locator('#workbench-sidebar').getAttribute('aria-hidden')) === 'true') {
    await toggle.click()
  }
  await expectExpanded(page)
}

async function sendHome(page: Page, text: string) {
  const editor = page.locator('[data-ui="workbench.composer"] .composer-tiptap')
  await expect(editor).toBeVisible()
  await editor.click()
  await page.keyboard.press('Escape')
  await editor.pressSequentially(text, { delay: 8 })
  await page.keyboard.press('Escape')
  const send = page.locator('[data-ui="workbench.composer"] [data-ui="chat.composer.action.send"]')
  await expect(send).toHaveAttribute('aria-disabled', 'false')
  await send.click()
}

async function openHeldSession(page: Page) {
  await installTurnState(page, {
    sessions: [{ id: HELD_ID, name: '进行中任务', place: 'inbox', updatedAt: '2026-10-18T00:00:00.000Z' }],
    officeSessions: [
      {
        id: HELD_ID,
        name: '进行中任务',
        messages: [
          { id: 'held-user', role: 'user', text: '保持进行中' },
          {
            id: 'held-assistant',
            role: 'assistant',
            text: '环境就绪，开始构建工作簿。',
            parts: heldParts(),
            status: 'success'
          }
        ]
      }
    ]
  })
  await openWorkbenchHome(page)
  await page.locator(`#workbench-sidebar [data-session-id="${HELD_ID}"]`).locator('button').first().click()
  await expect(page.getByText('环境就绪，开始构建工作簿。').last()).toBeVisible({ timeout: 30000 })
  await pushTopicStatus(page, HELD_ID, 'streaming', 'held-assistant')
  await expect(page.getByTestId('task-pending')).toBeVisible({ timeout: 20000 })
}

test('TT-01 TT-03 home send collapses the work sidebar and keeps it after reload', async ({ page }) => {
  test.setTimeout(180000)
  await installTurnState(page, { heldStream: true })
  await openWorkbenchHome(page)
  await expectExpanded(page)
  await sendHome(page, '首页发送收侧栏')
  await expectCollapsed(page)
  console.log('TT-01 首页发送后侧栏折叠')
  await page.reload({ waitUntil: 'domcontentloaded' })
  await openWorkbenchHome(page)
  await expectCollapsed(page)
  console.log('TT-03 刷新后仍折叠')
})

test('TT-02 opening a history session does not collapse an expanded sidebar', async ({ page }) => {
  test.setTimeout(180000)
  await installTurnState(page)
  await openWorkbenchHome(page)
  await expectExpanded(page)
  await page.locator(`#workbench-sidebar [data-session-id="${HISTORY_ID}"]`).locator('button').first().click()
  await expectExpanded(page)
  console.log('TT-02 点历史会话侧栏保持展开')
})

test('TT-04 TT-05 TT-06 TT-07 TT-09 held turn shows live status then clears', async ({ page }) => {
  test.setTimeout(180000)
  await openHeldSession(page)
  const pending = page.getByTestId('task-pending')
  await expect(pending).toHaveAttribute('aria-label', '正在处理')
  await expect(page.getByTestId('task-pending-verb')).toBeVisible()
  console.log('TT-09 live 时有稳定 aria-label 与 verb 节点')

  const editor = page.locator('[data-ui="workbench.composer"] .composer-tiptap, [data-ui~="part:composer-input"] textarea').last()
  await expect(editor).toBeVisible()
  const placeholder =
    (await editor.getAttribute('placeholder')) ||
    (await page.locator('[data-ui="workbench.composer"]').innerText())
  expect(String(placeholder)).toMatch(/排入下一条|queues the next message/)
  expect(String(placeholder)).not.toContain('Agent 运行中')
  await expect(page.locator('[data-ui~="chat.composer.action.pause"]').last()).toBeVisible()
  console.log('TT-04 Composer 短占位且 pause 可见')

  const turn = page.locator('[data-task-conversation-turn]').last()
  await expect(turn).toHaveAttribute('data-task-turn-state', 'live')
  await expect(turn.locator('[data-task-tool-name="bash"]').first()).toBeVisible()
  await expect(turn).toContainText('已完成')
  await expect(pending).toBeVisible()
  console.log('TT-05 工具已完成仍 live + task-pending')

  await expect(turn).toContainText('环境就绪，开始构建工作簿')
  await expect(pending).toBeVisible()
  console.log('TT-06 prose 看似写完仍 live')

  await pushTopicStatus(page, HELD_ID, 'done')
  await expect(page.getByTestId('task-pending')).toHaveCount(0, { timeout: 20000 })
  await expect(page.locator('[data-task-conversation-turn]').last()).toHaveAttribute('data-task-turn-state', 'idle')
  console.log('TT-07 done 后 pending 消失')
})

test('TT-08 approval pending tool hides the working status', async ({ page }) => {
  test.setTimeout(180000)
  await installTurnState(page, {
    sessions: [{ id: HELD_ID, name: '审批任务', place: 'inbox', updatedAt: '2026-10-18T00:00:00.000Z' }],
    officeSessions: [
      {
        id: HELD_ID,
        name: '审批任务',
        messages: [
          { id: 'held-user', role: 'user', text: '保持审批' },
          {
            id: 'held-assistant',
            role: 'assistant',
            text: '',
            parts: heldParts(true),
            status: 'success'
          }
        ]
      }
    ]
  })
  await openWorkbenchHome(page)
  await page.locator(`#workbench-sidebar [data-session-id="${HELD_ID}"]`).locator('button').first().click()
  await expect(page.locator('[data-task-tool-name="bash"]').first()).toBeVisible({ timeout: 30000 })
  await pushTopicStatus(page, HELD_ID, 'streaming', 'held-assistant')
  await expect(page.locator('[data-ui~="chat.composer.action.pause"]').last()).toBeVisible({ timeout: 20000 })
  await expect(page.getByTestId('task-pending')).toHaveCount(0)
  console.log('TT-08 审批等待时无 task-pending')
})

test('TT-10 only the latest assistant row may show awaiting status', async ({ page }) => {
  test.setTimeout(180000)
  await installTurnState(page, {
    sessions: [{ id: HELD_ID, name: '多轮任务', place: 'inbox', updatedAt: '2026-10-18T00:00:00.000Z' }],
    officeSessions: [
      {
        id: HELD_ID,
        name: '多轮任务',
        messages: [
          { id: 'old-user', role: 'user', text: '先完成一轮' },
          {
            id: 'old-assistant',
            role: 'assistant',
            text: '上一轮已经说完。',
            status: 'success'
          },
          { id: 'held-user', role: 'user', text: '保持进行中' },
          {
            id: 'held-assistant',
            role: 'assistant',
            text: '环境就绪，开始构建工作簿。',
            parts: heldParts(),
            status: 'success'
          }
        ]
      }
    ]
  })
  await openWorkbenchHome(page)
  await page.locator(`#workbench-sidebar [data-session-id="${HELD_ID}"]`).locator('button').first().click()
  await expect(page.getByText('上一轮已经说完。').last()).toBeVisible({ timeout: 30000 })
  await pushTopicStatus(page, HELD_ID, 'streaming', 'held-assistant')
  await expect(page.getByTestId('task-pending')).toHaveCount(1, { timeout: 20000 })
  await expect(page.locator('[data-task-turn-state="live"]')).toHaveCount(1)
  console.log('TT-10 仅最新 turn 为 live')
})
