import { expect, test } from '@playwright/test'

import { installWorkbench, ipcCount, workAgent } from './fixture'

for (const viewport of [
  { width: 897, height: 691 },
  { width: 520, height: 640 }
]) {
  test(`keeps presentation settings usable and preserves the topic at ${viewport.width}px`, async ({ page }) => {
    await page.setViewportSize(viewport)
    await installWorkbench(page, {
      withAgent: true,
      collab: true,
      publishReview: true,
      preferences: { 'chat.default_model_id': 'a3s::gpt-work' },
      ui: { withAgent: true }
    })
    await page.route('**/api/agents/agent-work', (route) => route.fulfill({ json: workAgent }))
    await page.route('**/api/aix/views', (route) =>
      route.fulfill({
        headers: {
          'access-control-allow-origin': '*',
          'access-control-allow-headers': 'authorization, content-type',
          'access-control-allow-methods': 'GET, OPTIONS'
        },
        json: {
          protocolVersion: 1,
          revision: '',
          packages: [
            {
              slug: 'a3s-presenton',
              packageId: 'use/a3s/presenton',
              generation: 1,
              revision: `sha256:${'a'.repeat(64)}`,
              containers: [],
              composerModes: [],
              views: [
                {
                  id: 'workbench',
                  title: '演示文稿',
                  description: '',
                  icon: 'panels',
                  order: 20,
                  container: null,
                  point: 'studio.editor',
                  presentation: 'webview'
                }
              ]
            }
          ]
        }
      })
    )
    await page.goto('/windows/main/index.html')
    if (viewport.width < 640) await page.getByRole('button', { name: '收起侧边栏', exact: true }).click()
    await page.getByRole('radio', { name: '演示文稿', exact: true }).click()
    const editor = page.locator('[data-ui~="workbench.composer"] .composer-tiptap')
    const topic = '季度业务回顾：面向管理层，重点说明下季度计划。'
    await editor.fill(topic)
    await page.locator('[data-ui="chat.composer.action.send"]').click()
    const dialog = page.getByRole('dialog', { name: '演示文稿', exact: true })
    await expect(dialog).toBeInViewport({ ratio: 0.95 })
    expect((await dialog.boundingBox())?.x).toBeGreaterThanOrEqual(60)
    await expect(dialog.locator('[data-presentation-topic]')).toHaveText(topic)
    const notes = dialog.getByRole('textbox', { name: '补充说明' })
    await notes.fill('保留风险与下一步行动。')
    await expect(notes).toBeInViewport({ ratio: 0.95 })
    await expect(dialog.getByRole('button', { name: '生成', exact: true })).toBeInViewport()
    expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(0)
    await dialog.getByRole('textbox', { name: '页数', exact: true }).fill('99')
    await dialog.getByRole('button', { name: '生成', exact: true }).click()
    await expect(dialog.getByRole('alert')).toContainText('1')
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(0)
    await dialog.getByRole('button', { name: '取消', exact: true }).click()
    await expect(dialog).toHaveCount(0)
    await expect(editor).toHaveText(topic)
    await page.locator('[data-ui="chat.composer.action.send"]').click()
    await expect(dialog.locator('[data-presentation-topic]')).toHaveText(topic)
    await dialog.getByRole('textbox', { name: '页数', exact: true }).fill('2')
    await expect(notes).toHaveValue('保留风险与下一步行动。')
    await dialog.getByRole('button', { name: '语言', exact: true }).click()
    await expect(page.getByRole('option', { name: '简体中文', exact: true })).toBeVisible()
    await page.getByRole('option', { name: '简体中文', exact: true }).click()
    await page.screenshot({ path: test.info().outputPath('presentation-settings.png') })
    await dialog.getByRole('button', { name: '生成', exact: true }).click()
    await expect(dialog).toHaveCount(0)
    await expect.poll(() => ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(1)
    // Presenton owns model generation in the task editor; the chat must not submit a second model turn.
    expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
    await expect(page.locator('[data-workspace-presentation-canvas]')).toBeVisible()
    await expect(page.locator('[data-ui~="workbench.composer"] .composer-tiptap')).toHaveText('')
  })
}
