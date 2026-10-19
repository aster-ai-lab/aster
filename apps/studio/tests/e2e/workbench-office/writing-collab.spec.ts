import { expect, test, type Page } from '@playwright/test'

import { expectWorkspaceEditor, installWorkbench, ipcCount, openHome } from './fixture'

/**
 * 交给助手 opens one task, the workspace copy, and a live caret.
 * The opening draft types one character at a time. Later turns stay in the
 * conversation. A reply that drops the draft stays off the paper.
 */

const BRIEF = '请写明事项。'
const ORG = '市教育局'
const CLOSER = '特此通知。'
const FILL_1 = '周五下午开会。'

async function openNoticeDraft(page: Page) {
  await page.getByRole('button', { name: '去拟稿' }).click()
  await page.locator('[data-ui="workbench.writing.picker"] [data-official-type="notice"]').click()
  const dialog = page.locator('[data-writing-stage]')
  await expect(page.locator('[data-writing-stage="brief"]')).toBeVisible()
  await dialog.locator('[data-writing-field="org"]').fill(ORG)
  await dialog.getByRole('textbox', { name: '写作说明' }).fill(BRIEF)
  await expect(page.locator('[data-writing-stage="brief"]')).toBeVisible()
  await expect(page.locator('[data-ui="workbench.writing.live"]')).toContainText(CLOSER)
  await dialog.getByRole('button', { name: '交给助手' }).click()
  await expect(page.locator('[data-writing-caret]')).toBeVisible()
  await expectWorkspaceEditor(page, '通知.docx')
  await expect(page.locator('[data-ui="workbench.writing.document"]')).toContainText(CLOSER)
}

async function sendFollowUp(page: Page, text: string, opens: number) {
  const composer = page.locator('[data-ui="workbench.composer"]')
  await expect(composer).toBeVisible()
  const editor = composer.locator('.composer-tiptap')
  await expect.poll(async () => (await editor.innerText()).trim()).toBe('')
  await editor.click()
  await page.keyboard.press('Escape')
  await editor.pressSequentially(text, { delay: 12 })
  await page.keyboard.press('Escape')
  await expect(composer.locator('[data-ui="chat.composer.action.send"]')).toHaveAttribute('aria-disabled', 'false')
  await page.keyboard.press('Enter')
  await expect.poll(() => ipcCount(page, 'ai.stream.open')).toBe(opens)
}

async function publishTurn(page: Page, userText: string, assistantText: string) {
  const listeners = await page.evaluate(
    async ({ userText, assistantText }) => {
      const host = globalThis as typeof globalThis & {
        __E2E_SESSION_MESSAGES__?: Array<Record<string, unknown>>
        __E2E_DATA_LISTENERS__?: Array<(effects: unknown) => void>
      }
      const rows = host.__E2E_SESSION_MESSAGES__ ?? []
      host.__E2E_SESSION_MESSAGES__ = rows
      const row = (id: string, role: 'user' | 'assistant', text: string, second: number) => {
        const stamp = new Date(Date.UTC(2026, 9, 7, 0, 0, second)).toISOString()
        return {
          id,
          sessionId: 'session-writing',
          role,
          data: { parts: [{ type: 'text', text }] },
          status: 'success',
          modelId: null,
          messageSnapshot: null,
          stats: null,
          searchableText: text,
          runtimeResumeToken: null,
          createdAt: stamp,
          updatedAt: stamp
        }
      }
      const userSecond = rows.length
      rows.unshift(row(`user-${userSecond}`, 'user', userText, userSecond))
      const assistantSecond = rows.length
      rows.unshift(row(`assistant-${assistantSecond}`, 'assistant', assistantText, assistantSecond))
      const note = (globalThis as { __e2eNoteSessionMessages?: (rows: unknown[]) => Promise<void> })
        .__e2eNoteSessionMessages
      if (note) await note([...(host.__E2E_SESSION_MESSAGES__ ?? [])])
      const effects = [
        {
          endpoint: '/agent-sessions/:sessionId/messages',
          kind: 'membership'
        }
      ]
      for (const listener of host.__E2E_DATA_LISTENERS__ ?? []) listener(effects)
      return host.__E2E_DATA_LISTENERS__?.length ?? 0
    },
    { userText, assistantText }
  )
  expect(listeners).toBeGreaterThan(0)
}

test.describe('公文任务持续协作', () => {
  test('交给助手后创建一条任务并打开公文副本，后续回稿留在会话里', async ({ page }) => {
    test.setTimeout(180_000)
    await installWorkbench(page, { withAgent: true, collab: true, workspaceFiles: true })
    await openHome(page)
    await openNoticeDraft(page)
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(1)
    await expect(page.locator('[data-ui="workbench.composer"]')).toBeVisible()
    await expect.poll(() => ipcCount(page, 'ai.stream.open')).toBe(1)

    const firstAsk = '请把第一处待填写写成周五下午开会。'
    await sendFollowUp(page, firstAsk, 2)
    await publishTurn(page, firstAsk, FILL_1)
    await expect(page.getByText(firstAsk).first()).toBeVisible()
    await expect(page.getByText(FILL_1).first()).toBeVisible()
    await expect(page.locator('[data-ui="workbench.writing.document"]')).toContainText(CLOSER)
    await expect(page.locator('[data-ui="workbench.writing.document"]')).not.toContainText(FILL_1)
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(1)
    expect(await ipcCount(page, 'ai.stream.open')).toBe(2)
  })
})
