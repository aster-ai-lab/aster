import { expect, test, type Locator, type Page } from '@playwright/test'

import { composerText, expectWorkspaceEditor, installWorkbench, ipcCount, openHome } from './fixture'

/**
 * Homepage official-writing cases.
 *
 * The picker is a flat list of 11 types. Each case opens that type from the
 * homepage. The live preview beside the form is the paper.
 *
 * Covered for every type: catalog order and default red-header flag, default
 * switches, a blank or whitespace brief that cannot be handed off, and a live
 * preview that carries the org, subject, addressee, outline, and closer.
 * Covered once, because the control is shared: every optional element on a
 * 请示, then the ones that change the paper turned off; 纪要 attendance;
 * 函 and 报告 gaining a red header; reopening the catalog clears the brief;
 * handing 通报 to the assistant creates one task.
 */

const BRIEF = '请写明事项。'
const ORG = '市教育局'
const SUBJECT = '周五开会'
const MAIN_TO = '各学校'

const SWITCH = {
  banner: '使用红头',
  copyNo: '份号',
  secret: '密级和保密期限',
  docNo: '文号',
  signer: '签发人',
  urgent: '紧急',
  attachment: '附件说明',
  copyTo: '抄送',
  attendance: '出席、请假和列席',
  amount: '列明金额',
  seal: '印章',
  note: '附注',
  imprint: '印发机关和印发日期'
} as const

type SwitchId = keyof typeof SWITCH

/** on: the switch is shown and checked. off: shown and unchecked. no: this type does not offer it. */
type Offer = 'on' | 'off' | 'no'

type WritingCase = {
  id: string
  title: string
  banner: boolean
  docNo: boolean
  signer: Offer
  urgent: boolean
  attachment: boolean
  copyTo: boolean
  attendance: Offer
  amount: Offer
  closer: string | null
  titleLine: string
  bannerLine: string | null
  outline: string
  extra: string[]
  absent: string[]
}

const SHARED_OFF: SwitchId[] = ['copyNo', 'secret', 'urgent', 'attachment', 'copyTo', 'note', 'imprint']

function paperCase(partial: Omit<WritingCase, 'urgent' | 'attachment' | 'copyTo'> & Partial<WritingCase>): WritingCase {
  return {
    urgent: false,
    attachment: false,
    copyTo: false,
    ...partial
  }
}

const DOWNWARD: WritingCase[] = [
  paperCase({
    id: 'decision',
    title: '决定',
    banner: true,
    docNo: true,
    signer: 'no',
    attendance: 'no',
    amount: 'no',
    closer: '本决定自发布之日起施行。',
    titleLine: `${ORG}关于${SUBJECT}的决定`,
    bannerLine: `${ORG}文件`,
    outline: '一、决定依据与事项',
    extra: [],
    absent: ['列明金额', '出席、请假和列席', '签发人']
  }),
  paperCase({
    id: 'notice',
    title: '通知',
    banner: true,
    docNo: true,
    signer: 'no',
    attendance: 'no',
    amount: 'no',
    closer: '特此通知。',
    titleLine: `${ORG}关于${SUBJECT}的通知`,
    bannerLine: `${ORG}文件`,
    outline: '一、事由与依据',
    extra: [],
    absent: ['列明金额', '出席、请假和列席', '签发人']
  }),
  paperCase({
    id: 'bulletin',
    title: '通报',
    banner: true,
    docNo: true,
    signer: 'no',
    attendance: 'no',
    amount: 'no',
    closer: '特此通报。',
    titleLine: `${ORG}关于${SUBJECT}的通报`,
    bannerLine: `${ORG}文件`,
    outline: '一、情况概述',
    extra: [],
    absent: ['列明金额', '出席、请假和列席', '签发人']
  }),
  paperCase({
    id: 'reply',
    title: '批复',
    banner: true,
    docNo: true,
    signer: 'no',
    attendance: 'no',
    amount: 'no',
    closer: '此复。',
    titleLine: `${ORG}关于${SUBJECT}的批复`,
    bannerLine: `${ORG}文件`,
    outline: '一、批复意见',
    extra: ['收悉。经研究，现批复如下：'],
    absent: ['列明金额', '出席、请假和列席', '签发人']
  })
]

const UPWARD: WritingCase[] = [
  paperCase({
    id: 'request',
    title: '请示',
    banner: true,
    docNo: true,
    signer: 'on',
    amount: 'off',
    attendance: 'no',
    closer: '妥否，请批示。',
    titleLine: `${ORG}关于${SUBJECT}的请示`,
    bannerLine: `${ORG}文件`,
    outline: '一、请示缘由',
    extra: ['为请示目的，现请示如下：'],
    absent: ['金额：', '出席、请假和列席']
  }),
  paperCase({
    id: 'report',
    title: '报告',
    banner: false,
    docNo: false,
    signer: 'off',
    amount: 'no',
    attendance: 'no',
    closer: '特此报告。',
    titleLine: `${ORG}关于${SUBJECT}的报告`,
    bannerLine: null,
    outline: '一、报告目的',
    extra: ['现将有关情况报告如下：'],
    absent: ['列明金额', '出席、请假和列席', `${ORG}文件`]
  })
]

const PARALLEL: WritingCase[] = [
  paperCase({
    id: 'letter',
    title: '函',
    banner: false,
    docNo: true,
    signer: 'no',
    amount: 'no',
    attendance: 'no',
    closer: '特此函达，请予支持为荷。',
    titleLine: `${ORG}关于${SUBJECT}的函`,
    bannerLine: null,
    outline: '一、事由',
    extra: [],
    absent: ['列明金额', '出席、请假和列席', '签发人', `${ORG}文件`]
  }),
  paperCase({
    id: 'opinion',
    title: '意见',
    banner: true,
    docNo: true,
    signer: 'no',
    amount: 'no',
    attendance: 'no',
    closer: '以上意见，请结合实际认真贯彻执行。',
    titleLine: `${ORG}关于${SUBJECT}的意见`,
    bannerLine: `${ORG}文件`,
    outline: '一、背景与问题',
    extra: [],
    absent: ['列明金额', '出席、请假和列席', '签发人']
  })
]

const MEETING_AND_GENERAL: WritingCase[] = [
  paperCase({
    id: 'minutes',
    title: '纪要',
    banner: true,
    docNo: false,
    signer: 'no',
    attendance: 'on',
    amount: 'no',
    closer: null,
    titleLine: `关于${SUBJECT}的会议纪要`,
    bannerLine: `${ORG}纪要`,
    outline: '一、会议概况',
    extra: ['会议听取了', '议定以下事项：', '出席：', '请假：', '列席：'],
    absent: ['列明金额', '签发人', '特此通知。', `${ORG}文件`]
  }),
  paperCase({
    id: 'proposal',
    title: '议案',
    banner: true,
    docNo: true,
    signer: 'off',
    attendance: 'no',
    amount: 'no',
    closer: '以上议案，请予审议。',
    titleLine: `${ORG}关于提请审议${SUBJECT}的议案`,
    bannerLine: `${ORG}文件`,
    outline: '一、案由',
    extra: [],
    absent: ['列明金额', '出席、请假和列席']
  }),
  paperCase({
    id: 'general',
    title: '通用公文',
    banner: false,
    docNo: false,
    signer: 'off',
    attendance: 'no',
    amount: 'no',
    closer: null,
    titleLine: `${ORG}关于事由的文种`,
    bannerLine: null,
    outline: '一、正文',
    extra: ['主送机关：'],
    absent: ['列明金额', '出席、请假和列席', '签发人', '特此通知。', `${ORG}文件`]
  })
]

async function openCatalog(page: Page) {
  await page.getByRole('button', { name: '去拟稿' }).click()
  await expect(page.locator('[data-ui="workbench.writing.picker"]')).toBeVisible()
}

async function openType(page: Page, typeId: string) {
  await openCatalog(page)
  await page.locator(`[data-ui="workbench.writing.picker"] [data-official-type="${typeId}"]`).click()
  await expect(page.locator('[data-writing-stage="brief"]')).toBeVisible()
}

function writingDialog(page: Page): Locator {
  return page.locator('[data-writing-stage]')
}

function liveFirstLine(page: Page): Locator {
  return page.locator('[data-ui="workbench.writing.live"]').getByRole('document').getByRole('paragraph').first()
}

/** Border of the run that contains this text. A zero-width reset border counts as none. */
async function borderIn(paragraph: Locator, label: string): Promise<string> {
  return paragraph.evaluate((node, wanted) => {
    const walker = document.createTreeWalker(node, NodeFilter.SHOW_TEXT)
    let text = walker.nextNode()
    while (text) {
      if ((text.textContent ?? '').includes(wanted)) {
        const parent = text.parentElement
        if (!parent) return 'none'
        const style = getComputedStyle(parent)
        const width = Number.parseFloat(style.borderTopWidth)
        return Number.isFinite(width) && width > 0 ? style.borderTopStyle : 'none'
      }
      text = walker.nextNode()
    }
    return 'missing'
  }, label)
}

/** Dashed character border on an empty slot. A filled value has none. */
async function placeholderBorder(page: Page, label: string): Promise<string> {
  return page
    .locator('[data-ui="workbench.writing.live"]')
    .getByText(label, { exact: true })
    .first()
    .evaluate((node) => {
      const root = node instanceof HTMLElement ? node : node.parentElement
      if (!root) return 'none'
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
      let text = walker.nextNode()
      while (text) {
        if ((text.textContent ?? '').trim().length > 0) {
          const parent = text.parentElement
          if (!parent) return 'none'
          const style = getComputedStyle(parent)
          const width = Number.parseFloat(style.borderTopWidth)
          return Number.isFinite(width) && width > 0 ? style.borderTopStyle : 'none'
        }
        text = walker.nextNode()
      }
      return 'none'
    })
}

/** The Word preview banner. 纪要 drops 会议. 函 is the org name alone. */
function liveBannerText(item: WritingCase): string | null {
  if (!item.banner) return null
  if (item.id === 'minutes') return `${ORG}纪要`
  if (item.id === 'letter') return ORG
  return `${ORG}文件`
}

function briefBox(page: Page): Locator {
  return writingDialog(page).getByRole('textbox', { name: '写作说明' })
}

async function closeWriting(page: Page) {
  await writingDialog(page).getByRole('button', { name: '关闭' }).click()
  await expect(page.locator('[data-writing-stage]')).toHaveCount(0)
}

async function expectSwitch(page: Page, name: string, on: boolean) {
  const control = writingDialog(page).getByRole('switch', { name })
  await expect(control).toHaveCount(1)
  await expect(control).toHaveAttribute('aria-checked', on ? 'true' : 'false')
}

async function expectOffer(page: Page, name: string, offer: Offer) {
  const control = writingDialog(page).getByRole('switch', { name })
  if (offer === 'no') {
    await expect(control).toHaveCount(0)
    return
  }
  await expect(control).toHaveCount(1)
  await expect(control).toHaveAttribute('aria-checked', offer === 'on' ? 'true' : 'false')
}

async function draftOne(page: Page, item: WritingCase, fields: { subject: boolean }) {
  await openType(page, item.id)
  await expect(writingDialog(page).getByRole('heading', { name: '公文写作' })).toBeVisible()
  await expect(writingDialog(page).getByText(item.title, { exact: true }).first()).toBeVisible()

  await expectSwitch(page, SWITCH.banner, item.banner)
  await expectSwitch(page, SWITCH.docNo, item.docNo)
  await expectSwitch(page, SWITCH.seal, true)
  for (const id of SHARED_OFF) {
    if (id === 'urgent' && item.urgent) continue
    await expectSwitch(page, SWITCH[id], false)
  }
  await expectOffer(page, SWITCH.signer, item.signer)
  await expectOffer(page, SWITCH.amount, item.amount)
  await expectOffer(page, SWITCH.attendance, item.attendance)

  const commit = writingDialog(page).getByRole('button', { name: '交给助手' })
  await expect(briefBox(page)).toHaveValue('')
  await expect(commit).toBeDisabled()
  await briefBox(page).fill(' \n\t ')
  await expect(commit).toBeDisabled()
  await commit.click({ force: true })
  await expect(page.locator('[data-writing-stage="brief"]')).toBeVisible()

  await writingDialog(page).locator('[data-writing-field="org"]').fill(ORG)
  const liveBanner = liveBannerText(item)
  if (liveBanner) await expect(liveFirstLine(page)).toHaveText(liveBanner)
  else await expect(page.locator('[data-ui="workbench.writing.live"]')).not.toContainText(`${ORG}文件`)

  if (fields.subject) await writingDialog(page).locator('[data-writing-field="subject"]').fill(SUBJECT)
  await writingDialog(page)
    .locator('[data-writing-field="mainTo"]')
    .fill(fields.subject ? MAIN_TO : '')
  await briefBox(page).fill(BRIEF)
  await expect(commit).toBeEnabled()
  const paper = page.locator('[data-ui="workbench.writing.live"]')
  await expect(paper).toBeVisible()
  await expect(paper).toContainText(BRIEF)
  await expect(paper).toContainText(item.titleLine)
  await expect(paper).toContainText(item.outline)
  if (fields.subject && item.id !== 'minutes') {
    const recipient = item.id === 'proposal' ? `${MAIN_TO}人民代表大会常务委员会：` : `${MAIN_TO}：`
    await expect(paper).toContainText(recipient)
  }
  if (item.closer) await expect(paper).toContainText(item.closer)
  if (item.bannerLine) await expect(paper).toContainText(item.bannerLine)
  await expect(paper).toContainText('成文日期')
  if (item.docNo) await expect(paper).toContainText('发文字号')
  else await expect(paper).not.toContainText('发文字号')
  if (item.signer === 'on') await expect(paper).toContainText('签发人：')
  else await expect(paper).not.toContainText('签发人：')
  await expect(paper).not.toContainText('金额：')
  for (const line of item.extra) await expect(paper).toContainText(line)
  for (const line of item.absent) {
    if (!line.startsWith('列明') && !line.startsWith('出席、') && line !== '签发人') {
      await expect(paper).not.toContainText(line)
    }
  }
  await expect(page.locator('[data-writing-stage="brief"]')).toBeVisible()
  await expect(page.locator('[data-ui="workbench.writing.draft"]')).toHaveCount(0)
  await closeWriting(page)
}

test.describe('公文写作全部文种', () => {
  test('目录按文种顺序列出，红头标记跟默认版式', async ({ page }) => {
    await installWorkbench(page)
    await openHome(page)
    await openCatalog(page)
    const picker = page.locator('[data-ui="workbench.writing.picker"]')
    const found = await picker.locator('[data-official-type]').evaluateAll((nodes) =>
      nodes.map((node) => ({
        id: node.getAttribute('data-official-type'),
        banner: node.getAttribute('data-writing-banner'),
        title: node.textContent ?? ''
      }))
    )
    const expected = [...DOWNWARD, ...UPWARD, ...PARALLEL, ...MEETING_AND_GENERAL]
    expect(found.map((item) => item.id)).toEqual(expected.map((item) => item.id))
    for (const item of expected) {
      const row = found.find((entry) => entry.id === item.id)
      expect(row?.banner).toBe(item.banner ? 'true' : 'false')
      expect(row?.title).toContain(item.title)
    }
  })

  test('下行文四种：空白说明不能交给助手，预览带标题、层次和结束语', async ({ page }) => {
    test.setTimeout(180_000)
    await installWorkbench(page)
    await openHome(page)
    for (const item of DOWNWARD) await draftOne(page, item, { subject: true })
  })

  test('上行文两种：请示保留签发人，报告默认没有红头', async ({ page }) => {
    test.setTimeout(120_000)
    await installWorkbench(page)
    await openHome(page)
    for (const item of UPWARD) await draftOne(page, item, { subject: true })
  })

  test('平行文两种：函默认没有红头，意见保留红头和结束语', async ({ page }) => {
    test.setTimeout(120_000)
    await installWorkbench(page)
    await openHome(page)
    for (const item of PARALLEL) await draftOne(page, item, { subject: true })
  })

  test('会议类和通用公文：纪要列出出席，通用公文在事由为空时用占位', async ({ page }) => {
    test.setTimeout(150_000)
    await installWorkbench(page)
    await openHome(page)
    for (const item of MEETING_AND_GENERAL) await draftOne(page, item, { subject: item.id !== 'general' })
  })

  test('请示打开全部可选要素后再关掉印章、红头、签发人和金额', async ({ page }) => {
    test.setTimeout(120_000)
    await installWorkbench(page)
    await openHome(page)
    await openType(page, 'request')
    const dialog = writingDialog(page)
    await dialog.locator('[data-writing-field="org"]').fill(ORG)
    await dialog.locator('[data-writing-field="subject"]').fill(SUBJECT)
    await dialog.locator('[data-writing-field="mainTo"]').fill(MAIN_TO)
    await briefBox(page).fill(BRIEF)
    await dialog.getByRole('switch', { name: SWITCH.copyNo }).click()
    await dialog.getByRole('textbox', { name: '份号' }).fill('000007')
    await dialog.getByRole('switch', { name: SWITCH.secret }).click()
    await dialog.getByRole('textbox', { name: '密级和保密期限' }).fill('机密★1年')
    await dialog.getByRole('switch', { name: SWITCH.urgent }).click()
    await dialog.getByRole('textbox', { name: '紧急程度' }).fill('特急')
    await dialog.locator('[data-writing-field="docNumber"]').fill('教请〔2026〕1号')
    await dialog.locator('[data-writing-field="signerName"]').fill('赵局长')
    await dialog.getByRole('switch', { name: SWITCH.attachment }).click()
    await dialog.getByRole('textbox', { name: '附件' }).fill('报名表')
    await dialog.getByRole('switch', { name: SWITCH.amount }).click()
    await dialog.getByRole('textbox', { name: '金额' }).fill('十二万元')
    await dialog.getByRole('switch', { name: SWITCH.note }).click()
    await dialog.getByRole('textbox', { name: '附注' }).fill('此件公开发布')
    await dialog.getByRole('switch', { name: SWITCH.copyTo }).click()
    await dialog.getByRole('textbox', { name: '抄送机关' }).fill('市财政局')
    await dialog.getByRole('switch', { name: SWITCH.imprint }).click()
    await dialog.locator('[data-writing-field="printOrg"]').fill('市教育局办公室')
    await dialog.locator('[data-writing-field="printDate"]').fill('2026年10月7日')
    await dialog.locator('[data-writing-field="signature"]').fill(ORG)
    await dialog.locator('[data-writing-field="issueDate"]').fill('2026年10月7日')
    const paper = page.locator('[data-ui="workbench.writing.live"]')
    await expect(page.locator('[data-writing-stage="brief"]')).toBeVisible()
    await expect(paper).toContainText('000007')
    await expect(paper).toContainText('机密★1年')
    await expect(paper).toContainText('特急')
    await expect(paper).toContainText('教请〔2026〕1号')
    await expect(paper).toContainText('签发人：赵局长')
    await expect(paper).toContainText('附件：报名表')
    await expect(paper).toContainText('金额：十二万元')
    await expect(paper).toContainText('（此件公开发布）')
    await expect(paper).toContainText('抄送：市财政局')
    await expect(paper).toContainText('市教育局办公室2026年10月7日印发')
    await expect(paper).toContainText('妥否，请批示。')
    await expect(briefBox(page)).toHaveValue(BRIEF)

    await dialog.getByRole('switch', { name: SWITCH.seal }).click()
    await dialog.getByRole('switch', { name: SWITCH.banner }).click()
    await dialog.getByRole('switch', { name: SWITCH.signer }).click()
    await dialog.getByRole('switch', { name: SWITCH.amount }).click()
    await expect(paper).not.toContainText(`${ORG}文件`)
    await expect(paper).not.toContainText('签发人：')
    await expect(paper).not.toContainText('金额：')
    await expect(paper.locator('[data-ui="workbench.writing.seal"]')).toHaveCount(0)
    await expect(paper).toContainText('报名表')
    await expect(paper).toContainText('妥否，请批示。')
  })

  test('纪要写上出席人员，函和报告可以打开红头，重开目录会清空说明', async ({ page }) => {
    test.setTimeout(120_000)
    await installWorkbench(page)
    await openHome(page)

    await openType(page, 'minutes')
    const dialog = writingDialog(page)
    await dialog.locator('[data-writing-field="present"]').fill('张三')
    await dialog.locator('[data-writing-field="absent"]').fill('李四')
    await dialog.locator('[data-writing-field="observer"]').fill('王五')
    await briefBox(page).fill(BRIEF)
    const paper = page.locator('[data-ui="workbench.writing.live"]')
    await expect(paper).toContainText('出席：张三')
    await expect(paper).toContainText('请假：李四')
    await expect(paper).toContainText('列席：王五')
    await dialog.getByRole('switch', { name: SWITCH.attendance }).click()
    await expect(dialog.getByRole('textbox', { name: '出席' })).toHaveCount(0)
    await expect(paper).not.toContainText('出席：')
    await expect(paper).not.toContainText('张三')
    await closeWriting(page)

    for (const typeId of ['letter', 'report'] as const) {
      await openType(page, typeId)
      await writingDialog(page).locator('[data-writing-field="org"]').fill(ORG)
      const first = liveFirstLine(page)
      if (typeId === 'letter') {
        await expect(first).toHaveText('发文字号')
        expect(await placeholderBorder(page, '发文字号')).toBe('dashed')
      } else {
        await expect(first).toHaveText(`${ORG}关于事由的报告`)
        expect(await borderIn(first, '事由')).toBe('dashed')
        expect(await borderIn(first, ORG)).toBe('none')
      }
      await writingDialog(page).getByRole('switch', { name: SWITCH.banner }).click()
      await expect(first).toHaveText(typeId === 'letter' ? ORG : `${ORG}文件`)
      if (typeId === 'letter') expect(await placeholderBorder(page, ORG)).toBe('none')
      await closeWriting(page)
    }

    await openType(page, 'notice')
    await briefBox(page).fill('旧说明')
    await closeWriting(page)
    await openType(page, 'bulletin')
    await expect(briefBox(page)).toHaveValue('')
    await closeWriting(page)
  })

  test('通报交给助手后只创建一条任务，并打开工作区里的公文副本', async ({ page }) => {
    await installWorkbench(page, { withAgent: true, workspaceFiles: true })
    await openHome(page)
    await openType(page, 'bulletin')
    await writingDialog(page).locator('[data-writing-field="org"]').fill(ORG)
    await briefBox(page).fill(BRIEF)
    await writingDialog(page).getByRole('button', { name: '交给助手' }).click()
    await expect(page.locator('[data-writing-stage]')).toHaveCount(0)
    await expect.poll(() => ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(1)
    await expectWorkspaceEditor(page, '通报.docx')
    await expect.poll(() => composerText(page)).toBe('')
  })
})
