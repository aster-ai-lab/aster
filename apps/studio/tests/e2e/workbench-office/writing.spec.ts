import { expect, test, type Locator, type Page } from '@playwright/test'

import { composerText, expectWorkspaceEditor, installWorkbench, ipcCount, openHome } from './fixture'

const NOTICE_BRIEF = '请教育局通知各校周五开会'

async function openTile(page: Page, typeId: string) {
  await page.locator(`[data-feature="write"] [data-writing-type="${typeId}"]`).click()
  await expect(page.getByRole('dialog')).toHaveAttribute('data-writing-stage', 'brief')
}

async function openCatalog(page: Page) {
  await page.getByRole('button', { name: '去拟稿' }).click()
  await expect(page.locator('[data-ui="workbench.writing.picker"]')).toBeVisible()
}

async function openCatalogType(page: Page, typeId: string) {
  await openCatalog(page)
  await page.locator(`[data-ui="workbench.writing.picker"] [data-official-type="${typeId}"]`).click()
  await expect(page.getByRole('dialog')).toHaveAttribute('data-writing-stage', 'brief')
}

function briefBox(page: Page): Locator {
  return page.getByRole('textbox', { name: '写作说明' })
}

function orgField(page: Page): Locator {
  return page.locator('[data-writing-field="org"]')
}

function livePaper(page: Page): Locator {
  return page.locator('[data-ui="workbench.writing.live"]')
}

test.describe('公文写作', () => {
  test('W1 首页四块磁贴是通知、报告、请示、纪要', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    const card = page.locator('[data-feature="write"]')
    await expect(card.locator('[data-writing-type="notice"]')).toContainText('通知')
    await expect(card.locator('[data-writing-type="report"]')).toContainText('报告')
    await expect(card.locator('[data-writing-type="request"]')).toContainText('请示')
    await expect(card.locator('[data-writing-type="minutes"]')).toContainText('纪要')
  })

  test('W2 去拟稿列出 11 个文种，输入框仍空', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openCatalog(page)
    const picker = page.locator('[data-ui="workbench.writing.picker"]')
    await expect(picker.locator('[data-official-type]')).toHaveCount(11)
    await expect(picker.locator('[data-official-type="notice"]')).toHaveAttribute('data-writing-banner', 'true')
    await expect(picker.locator('[data-official-type="report"]')).toHaveAttribute('data-writing-banner', 'false')
    await expect(page.getByRole('textbox', { name: '写作说明' })).toHaveCount(0)
    await expect.poll(() => composerText(page)).toBe('')
  })

  test('W3 目录里的通知在空说明时不能交给助手', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openCatalogType(page, 'notice')
    const banner = page.getByRole('switch', { name: '使用红头' })
    await expect(banner).toHaveAttribute('aria-checked', 'true')
    await expect(briefBox(page)).toHaveValue('')
    const commit = page.getByRole('button', { name: '交给助手' })
    await expect(commit).toBeDisabled()
    await commit.click({ force: true })
    await expect(page.getByRole('dialog')).toHaveAttribute('data-writing-stage', 'brief')
    await expect(page.locator('[data-ui="workbench.writing.draft"]')).toHaveCount(0)
  })

  test('W3 通知磁贴带着说明种子，红头默认打开', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openTile(page, 'notice')
    await expect(page.getByRole('switch', { name: '使用红头' })).toHaveAttribute('aria-checked', 'true')
    await expect(briefBox(page)).toHaveValue('发布需要周知或执行的事项。')
    await expect(page.getByRole('button', { name: '交给助手' })).toBeEnabled()
  })

  test('W4 机关名称写进实时预览，关掉红头后红头行消失', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openCatalogType(page, 'notice')
    const live = page.locator('[data-ui="workbench.writing.live"]')
    await expect(live).toBeVisible()
    await orgField(page).fill('市教育局')
    await expect(live).toContainText('市教育局文件')
    await page.getByRole('switch', { name: '使用红头' }).click()
    await expect(live).not.toContainText('市教育局文件')
  })

  test('W5 说明写进左侧预览，表单上的红头开关还在', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openCatalogType(page, 'notice')
    await page.getByRole('switch', { name: '使用红头' }).click()
    await briefBox(page).fill(NOTICE_BRIEF)
    const live = livePaper(page)
    await expect(page.getByRole('dialog')).toHaveAttribute('data-writing-stage', 'brief')
    await expect(live).toContainText(NOTICE_BRIEF)
    await expect(live).toContainText('特此通知。')
    await expect(live).not.toContainText('发文机关文件')
    await expect(page.getByRole('dialog').getByRole('button', { name: '返回' })).toHaveCount(0)
    await expect(page.getByRole('switch', { name: '使用红头' })).toHaveAttribute('aria-checked', 'false')
    await expect(briefBox(page)).toHaveValue(NOTICE_BRIEF)
  })

  test('W6 交给助手后创建一条任务，并在工作区打开公文副本', async ({ page }) => {
    await installWorkbench(page, { withAgent: true, workspaceFiles: true })
    await openHome(page)
    await openCatalogType(page, 'notice')
    await orgField(page).fill('市教育局')
    await page.getByRole('switch', { name: '使用红头' }).click()
    await briefBox(page).fill(NOTICE_BRIEF)
    await page.getByRole('button', { name: '交给助手' }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect.poll(() => ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(1)
    await expectWorkspaceEditor(page, '通知.docx')
    await expect.poll(() => composerText(page)).toBe('')
    await expect.poll(() => ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(1)
  })

  test('W7 报告磁贴红头关闭，说明里已有汇报种子', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openTile(page, 'report')
    await expect(page.getByRole('switch', { name: '使用红头' })).toHaveAttribute('aria-checked', 'false')
    await expect(briefBox(page)).toHaveValue('向上级汇报工作进展和需要说明的情况。')
  })

  test('W8 请示保留签发人，金额开关关着但仍在', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openTile(page, 'request')
    await expect(page.getByRole('switch', { name: '签发人' })).toHaveAttribute('aria-checked', 'true')
    await expect(page.getByRole('switch', { name: '列明金额' })).toHaveAttribute('aria-checked', 'false')
  })

  test('W9 纪要打开出席，没有金额开关', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openTile(page, 'minutes')
    await expect(page.getByRole('switch', { name: '出席、请假和列席' })).toHaveAttribute('aria-checked', 'true')
    await expect(page.getByRole('switch', { name: '列明金额' })).toHaveCount(0)
    await expect(briefBox(page)).toHaveValue('记载会议的主要情况和议定事项。')
  })

  test('W10 决定、函、议案和通用公文走默认版式', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)

    await openCatalogType(page, 'decision')
    await expect(page.getByRole('switch', { name: '使用红头' })).toHaveAttribute('aria-checked', 'true')
    await briefBox(page).fill('请写明事项。')
    await expect(livePaper(page)).toContainText('本决定自发布之日起施行。')
    await page.getByRole('button', { name: '交给助手' }).click()
    await expect.poll(() => composerText(page)).toContain('这份决定')

    await openCatalogType(page, 'letter')
    await expect(page.getByRole('switch', { name: '使用红头' })).toHaveAttribute('aria-checked', 'false')
    await expect(page.getByRole('switch', { name: '文号' })).toHaveAttribute('aria-checked', 'true')
    await briefBox(page).fill('请写明事项。')
    await expect(livePaper(page)).toContainText('特此函达，请予支持为荷。')
    await page.getByRole('button', { name: '交给助手' }).click()

    await openCatalogType(page, 'proposal')
    await briefBox(page).fill('请写明事项。')
    await expect(livePaper(page)).toContainText('以上议案，请予审议。')
    await page.getByRole('button', { name: '交给助手' }).click()

    await openCatalogType(page, 'general')
    await expect(page.getByRole('switch', { name: '使用红头' })).toHaveAttribute('aria-checked', 'false')
    await briefBox(page).fill('请写明事项。')
    await expect(livePaper(page)).not.toContainText('特此通知。')
    await page.getByRole('button', { name: '交给助手' }).click()
    await expect.poll(() => composerText(page)).toContain('这份通用公文')
  })

  test('W11 份号、密级和出席的输入随开关出现', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openTile(page, 'minutes')
    await expect(page.getByRole('textbox', { name: '份号' })).toHaveCount(0)
    await page.getByRole('switch', { name: '份号' }).click()
    await page.getByRole('textbox', { name: '份号' }).fill('000099')
    await page.getByRole('switch', { name: '密级和保密期限' }).click()
    await page.getByRole('textbox', { name: '密级和保密期限' }).fill('机密★1年')
    await expect(page.getByRole('textbox', { name: '出席' })).toBeVisible()
    await page.getByRole('switch', { name: '出席、请假和列席' }).click()
    await expect(page.getByRole('textbox', { name: '出席' })).toHaveCount(0)
    await expect(page.getByRole('textbox', { name: '请假' })).toHaveCount(0)
    await expect(page.getByRole('textbox', { name: '列席' })).toHaveCount(0)
    const live = livePaper(page)
    await expect(live).toContainText('000099')
    await expect(live).toContainText('机密★1年')
    await expect(live).not.toContainText('出席：')
  })

  test('W12 通知没有金额和出席开关', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openTile(page, 'notice')
    await expect(page.getByRole('switch', { name: '列明金额' })).toHaveCount(0)
    await expect(page.getByRole('switch', { name: '出席、请假和列席' })).toHaveCount(0)
  })

  test('W13 关闭对话框不创建任务', async ({ page }) => {
    await installWorkbench(page, { withAgent: true })
    await openHome(page)
    await openCatalogType(page, 'notice')
    await page.getByRole('dialog').getByRole('button', { name: '关闭' }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    await expect.poll(() => composerText(page)).toBe('')
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(0)
  })

  test('W14 超过 8000 字的说明不创建任务', async ({ page }) => {
    await installWorkbench(page, { withAgent: true })
    await openHome(page)
    await openCatalogType(page, 'notice')
    await briefBox(page).fill(`请写明事项。${'甲'.repeat(8000)}`)
    await page.getByRole('button', { name: '交给助手' }).click()
    await expect(page.getByRole('dialog')).toHaveCount(0)
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(0)
  })

  test('W15 深色下公文纸面仍是白纸，红头仍是正红', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await page.evaluate(() => document.documentElement.classList.add('dark'))
    await openCatalogType(page, 'notice')
    await briefBox(page).fill(NOTICE_BRIEF)
    const draft = livePaper(page)
    await expect(draft.locator('.work-document-page')).toBeVisible()
    await expect
      .poll(async () =>
        draft.evaluate((root) => {
          const sheet = root.querySelector('.work-document-page-sheet')
          const paper = root.querySelector('.work-document-page')
          const node = sheet ?? paper
          return node ? getComputedStyle(node).backgroundColor : ''
        })
      )
      .toBe('rgb(255, 255, 255)')
    const colors = await draft.evaluate((root) => {
      const paper = root.querySelector('.work-document-page')
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
      let banner = ''
      let text = walker.nextNode()
      while (text) {
        if ((text.textContent ?? '').includes('文件')) {
          const parent = text.parentElement
          if (parent) banner = getComputedStyle(parent).color
          break
        }
        text = walker.nextNode()
      }
      return {
        color: paper ? getComputedStyle(paper).color : '',
        banner
      }
    })
    expect(colors.color).toBe('rgb(32, 35, 43)')
    expect(colors.banner).toBe('rgb(255, 0, 0)')
  })

  test('W16 窄屏对话框留在视口内，说明区可以滚动', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 800 })
    await installWorkbench(page)
    await openHome(page)
    await openCatalogType(page, 'notice')
    const box = await page.getByRole('dialog').boundingBox()
    expect(box).toBeTruthy()
    expect(box!.x).toBeGreaterThanOrEqual(-1)
    expect(box!.x + box!.width).toBeLessThanOrEqual(391)
    const layout = await page.locator('[data-ui="workbench.writing.layout"]').evaluate((node) => {
      const style = getComputedStyle(node)
      const brief = node.querySelector('[data-ui="workbench.writing.brief"]')
      const briefStyle = brief ? getComputedStyle(brief) : null
      return {
        columns: style.gridTemplateColumns,
        overflowY: briefStyle?.overflowY ?? ''
      }
    })
    expect(layout.columns.split(' ').filter((part) => part && part !== '/')).toHaveLength(1)
    expect(layout.overflowY).toBe('auto')
  })

  test('W17 宽屏左右分栏，左侧是公文预览', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 })
    await installWorkbench(page)
    await openHome(page)
    await openCatalogType(page, 'notice')
    const columns = await page.locator('[data-ui="workbench.writing.layout"]').evaluate((node) => {
      return getComputedStyle(node).gridTemplateColumns.split(' ').filter(Boolean)
    })
    expect(columns.length).toBe(2)
    const live = page.locator('[data-ui="workbench.writing.live"]')
    await expect(live.locator('.work-document-editor.preview')).toBeVisible()
    await expect(live).toContainText('发文机关文件')
    const border = await live
      .getByText('发文机关', { exact: true })
      .first()
      .evaluate((node) => {
        const root = node instanceof HTMLElement ? node : node.parentElement
        if (!root) return 'none'
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
        const text = walker.nextNode()
        const parent = text?.parentElement
        if (!parent) return 'none'
        const style = getComputedStyle(parent)
        const width = Number.parseFloat(style.borderTopWidth)
        return Number.isFinite(width) && width > 0 ? style.borderTopStyle : 'none'
      })
    expect(border).toBe('dashed')
  })

  test('W18 创作卡和 PDF 行提示暂未开放，不打开公文对话框', async ({ page }) => {
    await installWorkbench(page, { homePins: ['home-writing', 'home-create', 'home-process'] })
    await openHome(page)
    await page.locator('[data-feature="create"]').click()
    await expect(page.getByText('暂未开放').first()).toBeVisible()
    await page.locator('[data-create="ppt"]').click()
    await expect(page.getByText('暂未开放').first()).toBeVisible()
    const pdf = page.locator('[data-feature="pdf"]')
    await pdf.getByRole('button', { name: 'PDF 转 Word' }).click()
    await pdf.getByRole('button', { name: 'Word 转 PDF' }).click()
    await expect(page.locator('[data-writing-stage]')).toHaveCount(0)
  })
})

test.describe('公文任务文件', () => {
  test('W19 打开任务后工作区是公文副本，对话旁逐字打出正文', async ({ page }) => {
    await installWorkbench(page, { withAgent: true, workspaceFiles: true })
    await openHome(page)
    await openCatalogType(page, 'notice')
    await page.getByRole('switch', { name: '使用红头' }).click()
    await orgField(page).fill('市教育局')
    await briefBox(page).fill(NOTICE_BRIEF)
    await page.getByRole('button', { name: '交给助手' }).click()
    await expect(page.locator('[data-writing-caret]')).toBeVisible()
    await expectWorkspaceEditor(page, '通知.docx')
    await expect(page.locator('[data-ui="workbench.writing.document"]')).toContainText('特此通知')
    await expect(page.getByText('另一份完全不同的稿')).toHaveCount(0)
  })

  test('W20 回稿留在会话里，正文在光标处续写', async ({ page }) => {
    await installWorkbench(page, {
      withAgent: true,
      workspaceFiles: true,
      createdMessages: [{ id: 'msg-follow', role: 'assistant', text: '请于周一前报送材料。' }]
    })
    await openHome(page)
    await openCatalogType(page, 'notice')
    await page.getByRole('switch', { name: '使用红头' }).click()
    await orgField(page).fill('市教育局')
    await briefBox(page).fill(NOTICE_BRIEF)
    await expect(livePaper(page)).toContainText(NOTICE_BRIEF)
    await expect(livePaper(page)).toContainText('特此通知。')
    await page.getByRole('button', { name: '交给助手' }).click()
    await expect(page.locator('[data-writing-caret]')).toBeVisible()
    await expectWorkspaceEditor(page, '通知.docx')
    await expect(page.locator('[data-ui="workbench.writing.document"]')).toContainText('特此通知')
    await expect(page.getByText('请于周一前报送材料。').first()).toBeVisible()
  })
})
