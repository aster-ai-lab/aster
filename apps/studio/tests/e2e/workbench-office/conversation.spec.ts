import { expect, test } from '@playwright/test'

import { installWorkbench, QUOTE, type OfficeMessage, type OfficeSession } from './fixture'

const FENCE = [
  '```a3s-review-finding',
  '{"id":"f1","title":"付款期限","evidenceQuote":"甲方应在三日内付款。","summary":"期限过短","guidance":"改为书面通知"}',
  '```'
].join('\n')

const OPEN_FENCE = ['```a3s-review-finding', '{"id":"f1","title":"付款期限"'].join('\n')

function session(id: string, name: string, messages: OfficeMessage[]): OfficeSession {
  return { id, name, messages }
}

async function openSession(page: Parameters<typeof installWorkbench>[0], item: OfficeSession, publishReview: boolean) {
  await installWorkbench(page, { sessions: [item], publishReview, withAgent: true })
  await page.goto('/windows/main/index.html', { waitUntil: 'domcontentloaded' })
  await page.getByRole('button', { name: item.name }).click()
}

test.describe('审查会话', () => {
  test('R13 闭合围栏显示发现卡，点开后见证据', async ({ page }) => {
    const item = session('session-review-closed', '审查样本', [
      {
        id: 'msg-user',
        role: 'user',
        text: `/review 审查这个文件\n/tmp/a3s-studio-review/notice.docx`
      },
      { id: 'msg-assistant', role: 'assistant', text: FENCE }
    ])
    await openSession(page, item, true)
    const card = page.locator('[data-chat-review-findings="true"]')
    await expect(card).toBeVisible()
    await expect(card).toHaveAttribute('data-chat-review-findings-count', '1')
    await expect(card).toContainText('风险')
    await expect(card).toContainText('1 条审查发现')
    await page.locator('[data-chat-review-finding-toggle]').click()
    await expect(page.locator('[data-chat-review-finding-open="true"]')).toBeVisible()
    await expect(card).toContainText('证据')
    await expect(card).toContainText(QUOTE)
  })

  test('R14 未闭合围栏保持原文，不变成发现卡', async ({ page }) => {
    const item = session('session-review-open', '审查整理', [
      { id: 'msg-assistant', role: 'assistant', text: OPEN_FENCE, status: 'pending' }
    ])
    await openSession(page, item, true)
    const source = page.locator('[data-task-fence="source"]')
    await expect(source).toContainText('付款期限')
    await expect(page.locator('[data-chat-review-findings]')).toHaveCount(0)
  })

  test('R15 普通助手文字没有审查卡', async ({ page }) => {
    const item = session('session-plain', '普通会话', [
      { id: 'msg-assistant', role: 'assistant', text: '这是一段普通回复。' }
    ])
    await openSession(page, item, true)
    await expect(page.getByText('这是一段普通回复。')).toBeVisible()
    await expect(page.locator('[data-chat-review-findings]')).toHaveCount(0)
  })

  test('R16 R18 发现卡展开；有原文标记时才画连线', async ({ page }) => {
    const item = session('session-review-link', '审查连线', [
      {
        id: 'msg-user',
        role: 'user',
        text: `/review 审查这个文件\n/tmp/a3s-studio-review/notice.docx`
      },
      { id: 'msg-assistant', role: 'assistant', text: FENCE }
    ])
    await openSession(page, item, true)
    await page.locator('[data-chat-review-finding-toggle]').click()
    await expect(page.locator('[data-chat-review-finding-open="true"]')).toBeVisible()
    const mark = page.locator('[data-review-office-document] [data-comment-id="f1"]')
    if ((await mark.count()) === 0) {
      await expect(page.locator('[data-review-annotation-links]')).toHaveCount(0)
      test.skip(true, '预览打不开壳层里的 Word，原文连线留在车道 B')
    }
    await expect(page.locator('[data-review-annotation-link="f1"]')).toBeVisible()
    await expect(page.locator('[data-review-annotation-occluder]').first()).toBeAttached()
  })

  test('R17 审查技能未发布时不画连线', async ({ page }) => {
    const item = session('session-review-off', '审查未发布', [{ id: 'msg-assistant', role: 'assistant', text: FENCE }])
    await installWorkbench(page, { sessions: [item], publishReview: false, withAgent: true })
    await page.goto('/windows/main/index.html', { waitUntil: 'domcontentloaded' })
    await page.getByRole('button', { name: item.name }).click()
    await expect(page.getByText('付款期限')).toBeVisible()
    await expect(page.locator('[data-review-annotation-links]')).toHaveCount(0)
    await expect(page.locator('[data-chat-review-findings="true"]')).toHaveCount(0)
  })
})
