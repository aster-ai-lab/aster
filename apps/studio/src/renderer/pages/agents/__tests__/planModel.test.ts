import { describe, expect, it } from 'vitest'

import { planLineDiff, planRewriteText } from '../planAssist'
import { planDocumentDocx } from '../planDocx'
import { planExportName } from '../planExport'
import {
  acceptPlanLogo,
  createPlanDraft,
  insertPlanMarkup,
  loadPlanDrafts,
  removePlanImage,
  PLAN_STYLE_DEFAULT,
  planAccent,
  planCaptionLabel,
  planDocumentHtml,
  planEditedPhrase,
  planFlowPages,
  planHeadingNumber,
  planHeadings,
  planTocChunks,
  planTocEntries,
  planImageSlot,
  planListMark,
  planPageCount,
  planPages,
  parsePlanMarkdown,
  planTemplate,
  planTemplatesIn,
  resolvePlanImage,
  PLAN_DRAFTS_KEY,
  PLAN_TEMPLATES,
  savePlanDrafts
} from '../planModel'

function memory(): Storage {
  const values = new Map<string, string>()
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => {
      values.set(key, value)
    },
    removeItem: (key) => {
      values.delete(key)
    },
    clear: () => values.clear(),
    key: () => null,
    length: 0
  }
}

describe('planModel', () => {
  it('filters templates by category and keeps one blank start', () => {
    expect(planTemplatesIn('all')).toHaveLength(PLAN_TEMPLATES.length)
    expect(new Set(PLAN_TEMPLATES.map((item) => item.id)).size).toBe(PLAN_TEMPLATES.length)
    expect(PLAN_TEMPLATES).toHaveLength(16)
    expect(planTemplatesIn('academic').map((item) => item.id)).toEqual(['research', 'thesis'])
    expect(planTemplatesIn('business').map((item) => item.id)).toEqual(['proposal', 'metrics'])
    expect(planTemplatesIn('meeting').map((item) => item.id)).toEqual(['minutes', 'weekly'])
    expect(planTemplatesIn('development').map((item) => item.id)).toEqual(['architecture', 'prd', 'incident'])
    expect(planTemplatesIn('design').map((item) => item.id)).toEqual(['whitepaper', 'tokens'])
    expect(planTemplatesIn('delivery').map((item) => item.id)).toEqual(['delivery', 'approval', 'sop'])
    expect(planTemplatesIn('blank')[0]?.markdown.startsWith('# 开始')).toBe(true)
    expect(planHeadings(planTemplate('approval')?.markdown ?? '').map((item) => item.text)).toEqual([
      '企业信息化项目签审交付方案',
      '文档签审',
      '1. 项目概述',
      '2. 建设范围',
      '3. 总体方案',
      '4. 交付清单',
      '5. 验收标准',
      '6. 版本记录'
    ])
    expect(planHeadings(planTemplate('research')?.markdown ?? '').map((item) => item.text)).toEqual([
      '调研与实验报告',
      '一、研究背景与目标',
      '二、研究方法',
      '三、实验结果',
      '四、分析与讨论',
      '五、结论与建议'
    ])
  })

  it('creates a draft from the template and round-trips it', () => {
    const template = planTemplate('guide')
    expect(template).toBeTruthy()
    const draft = createPlanDraft({
      template: template!,
      title: '方案编制说明',
      subtitle: '左侧写正文',
      organization: '本机',
      department: '方案组',
      now: Date.UTC(2026, 9, 7, 1, 0, 0),
      id: 'draft-guide'
    })
    expect(draft.meta.title).toBe('方案编制说明')
    expect(draft.meta.date).toMatch(/^\d{4}-\d{2}-\d{2}$/)
    expect(draft.markdown.startsWith('# 概述')).toBe(true)
    expect(draft.style).toEqual(PLAN_STYLE_DEFAULT)
    expect(planPageCount(draft.markdown)).toBe(3)
    expect(planPageCount('')).toBe(1)
    expect(planFlowPages(draft.markdown, 'manual')).toEqual(planPages(draft.markdown))
    expect(planFlowPages('# One\n\n<!-- pagebreak -->\n# Two', 'auto')).toHaveLength(2)
    const packed = Array.from({ length: 40 }, (_, index) => `paragraph ${index} `.repeat(8)).join('\n\n')
    expect(planFlowPages(packed, 'manual')).toHaveLength(1)
    expect(planFlowPages(packed, 'auto').length).toBeGreaterThan(1)
    const captionPage = `${Array.from({ length: 8 }, () => 'word '.repeat(40)).join('\n\n')}\n\n<!-- caption: 对照 -->\n| A | B |\n| --- | --- |\n| 1 | 2 |`
    for (const page of planFlowPages(captionPage, 'auto')) {
      const tableAt = page.findIndex((block) => block.kind === 'table')
      if (tableAt >= 0) expect(page[tableAt - 1]?.kind).toBe('p')
    }
    expect(planCaptionLabel('Fig ', 2, 'Result', true)).toBe('Fig 2  Result')
    expect(planListMark('ol', PLAN_STYLE_DEFAULT, 0)).toBe('1.')
    expect(planListMark('ol', { ...PLAN_STYLE_DEFAULT, numbers: 'cjk' }, 0)).toBe('一、')
    expect(planListMark('ul', { ...PLAN_STYLE_DEFAULT, bullet: 'check' }, 0)).toBe('\u2713')

    const store = memory()
    savePlanDrafts(store, [draft])
    expect(loadPlanDrafts(store)).toEqual([draft])
    const raw = JSON.parse(JSON.stringify(draft)) as {
      style?: {
        pagination?: string
        imagePrefix?: string
        watermarkImage?: string
        watermarkKind?: string
        watermarkOn?: boolean
      }
    }
    delete raw.style
    store.setItem(PLAN_DRAFTS_KEY, JSON.stringify([raw]))
    expect(loadPlanDrafts(store)[0]?.style).toEqual(PLAN_STYLE_DEFAULT)
    raw.style = { watermarkImage: 'https://example.com/mark.png', watermarkKind: 'image' }
    store.setItem(PLAN_DRAFTS_KEY, JSON.stringify([raw]))
    expect(loadPlanDrafts(store)[0]?.style.watermarkImage).toBe('https://example.com/mark.png')
    expect(loadPlanDrafts(store)[0]?.style.watermarkKind).toBe('image')
    raw.style = {
      pagination: 'flow',
      imagePrefix: '123456789',
      watermarkImage: 'http://example.com/a.png',
      watermarkOn: false
    }
    store.setItem(PLAN_DRAFTS_KEY, JSON.stringify([raw]))
    const restored = loadPlanDrafts(store)[0]?.style
    expect(restored?.pagination).toBe('manual')
    expect(restored?.imagePrefix).toBe('12345678')
    expect(restored?.watermarkImage).toBe('')
    expect(restored?.watermarkOn).toBe(false)
    store.setItem(PLAN_DRAFTS_KEY, '{')
    expect(loadPlanDrafts(store)).toEqual([])
  })

  it('numbers headings, splits on heading 1, and builds a contents list', () => {
    const source = '# A\n\ntext\n\n# B'
    expect(planFlowPages(source, 'manual', true, false)).toHaveLength(1)
    expect(planFlowPages(source, 'manual', true, true)).toHaveLength(2)
    expect(planHeadingNumber('decimal', [1, 2, 3, 0], 1)).toBe('1')
    expect(planHeadingNumber('decimal', [1, 2, 3, 0], 3)).toBe('1.2.3')
    expect(planHeadingNumber('none', [1, 0, 0, 0], 1)).toBe('')
    expect(planHeadingNumber('skip', [4, 2, 0, 0], 1)).toBe('')
    expect(planHeadingNumber('skip', [4, 2, 0, 0], 2)).toBe('2')
    expect(planHeadingNumber('cjk', [1, 2, 1, 0], 1)).toBe('一、')
    expect(planHeadingNumber('cjk', [1, 2, 1, 0], 2)).toBe('（二）')
    expect(planHeadingNumber('cjk', [1, 2, 1, 0], 3)).toBe('1')
    expect(planHeadingNumber('mixed', [1, 2, 3, 0], 1)).toBe('一、')
    expect(planHeadingNumber('mixed', [1, 2, 3, 0], 3)).toBe('2.3')
    const pages = planFlowPages('# A\n\n## B\n\n### C', 'manual')
    expect(planTocEntries(pages, 2).map((item) => item.text)).toEqual(['A', 'B'])
    expect(planTocEntries(pages, 1)).toHaveLength(1)
    expect(planAccent('#abcdef', 'cover')).toBe('#abcdef')
    expect(planAccent('#abcdef', 'pine')).toBe('#1f3d32')
    const template = planTemplate('blank')
    expect(template).toBeTruthy()
    const draft = createPlanDraft({
      template: template!,
      title: '方案',
      subtitle: '',
      organization: '',
      department: '',
      now: 1,
      id: 'draft-toc'
    })
    expect(planDocumentHtml({ ...draft, style: { ...draft.style, tocOn: false } })).not.toContain('Contents')
    const html = planDocumentHtml({
      ...draft,
      style: { ...draft.style, tocOn: true, tocTitle: 'Contents', numbering: 'decimal' }
    })
    expect(html).toContain('Contents')
    expect(html).toContain('1 开始')
    const many = Array.from({ length: 30 }, (_, index) => `# H${index}`).join('\n\n')
    const packed = planTocEntries(planFlowPages(many, 'manual'), 3)
    expect(planTocChunks(packed)).toHaveLength(2)
    expect(planTocChunks([])).toHaveLength(1)
    const wide = planDocumentHtml({
      ...draft,
      markdown: many,
      style: { ...draft.style, tocOn: true, tocTitle: 'Contents' }
    })
    expect(wide.match(/class="page"/g)).toHaveLength(3)
    expect(wide).toContain('<p class="num">2</p>')
    expect(wide).toContain('<p class="num">4</p>')
  })

  it('inserts markdown at the caret and phrases the edit time', () => {
    expect(insertPlanMarkup('hello', 0, 5, 'bold')).toEqual({ value: '**hello**', cursor: 7 })
    expect(insertPlanMarkup('', 0, 0, 'h1').value).toBe('# ')
    expect(insertPlanMarkup('', 0, 0, 'h3').value).toBe('### ')
    expect(insertPlanMarkup('x', 0, 1, 'sup').value).toBe('<sup>x</sup>')
    expect(removePlanImage('![a](plan-image:img1)\n# t', { img1: 'data:image/png;base64,aa' }, 'img1')).toEqual({
      markdown: '# t',
      images: {}
    })
    expect(planLineDiff('a\nb\nc', 'a\nB\nc').map((row) => row.kind)).toEqual(['same', 'del', 'add', 'same'])
    expect(insertPlanMarkup('', 0, 0, 'ordered')).toEqual({ value: '1. ', cursor: 3 })
    expect(insertPlanMarkup('a', 1, 1, 'rule')).toEqual({ value: 'a\n---', cursor: 5 })
    expect(
      parsePlanMarkdown('<!-- caption: 对照 -->\n| A | B |\n| --- | --- |\n| 1 | 2 |').map((item) => item.kind)
    ).toEqual(['p', 'table'])
    expect(planHeadings('#### 步骤')).toEqual([{ level: 4, text: '步骤' }])
    const now = 1_700_000_000_000
    expect(planEditedPhrase(now, now)).toEqual({ key: 'workbench.plan.draft.edited_just' })
    expect(planEditedPhrase(now - 26 * 60_000, now)).toEqual({ key: 'workbench.plan.draft.edited_minutes', count: 26 })
    expect(acceptPlanLogo(180 * 1024)).toBe(true)
    expect(acceptPlanLogo(180 * 1024 + 1)).toBe(false)
  })

  it('keeps pictures, diagrams, and formulas as their own blocks', () => {
    expect(parsePlanMarkdown('![图](plan-image:img1)')).toEqual([{ kind: 'image', alt: '图', ref: 'plan-image:img1' }])
    expect(parsePlanMarkdown('![x](javascript:alert(1))')[0]?.kind).toBe('p')
    expect(parsePlanMarkdown('```mermaid\nflowchart LR\nA-->B\n```')).toEqual([
      { kind: 'code', lang: 'mermaid', text: 'flowchart LR\nA-->B' }
    ])
    expect(parsePlanMarkdown('$$\nE=mc^2\n$$')).toEqual([{ kind: 'math', text: 'E=mc^2' }])
    expect(resolvePlanImage('https://example.com/a.png', {})).toBe('https://example.com/a.png')
    expect(resolvePlanImage('http://example.com/a.png', {})).toBeNull()
    const picture = 'data:image/png;base64,aaaa'
    let images: Record<string, string> = {}
    for (let index = 0; index < 8; index += 1) {
      const slot = planImageSlot(images, picture, index + 1)
      expect(slot).toBeTruthy()
      images = slot!.images
    }
    expect(planImageSlot(images, picture, 9)).toBeNull()
    expect(insertPlanMarkup('见这里', 0, 3, 'link').value).toBe('[见这里](https://)')
    expect(insertPlanMarkup('', 0, 0, 'break').value).toBe('<!-- pagebreak -->')
    expect(planExportName('a/b:c', 'md')).toBe('abc.md')
    expect(planRewriteText('```markdown\n# 标题\n```')).toBe('# 标题')

    const template = planTemplate('blank')
    expect(template).toBeTruthy()
    const draft = createPlanDraft({
      template: template!,
      title: '<script>alert(1)</script>',
      subtitle: '',
      organization: '本机',
      department: '方案组',
      now: 1,
      id: 'draft-html'
    })
    draft.markdown = '# <script>alert(1)</script>'
    const html = planDocumentHtml(draft)
    expect(html).not.toContain('<script>')
    expect(html).toContain('&lt;script&gt;')
  })

  it('writes a word document from the plan blocks', async () => {
    const template = planTemplate('blank')
    expect(template).toBeTruthy()
    const draft = createPlanDraft({
      template: template!,
      title: '方案',
      subtitle: '说明',
      organization: '本机',
      department: '方案组',
      now: 1,
      id: 'draft-docx'
    })
    draft.markdown = '# 标题\n\n一段话\n\n![图](plan-image:img1)'
    draft.images = {
      img1: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='
    }
    const bytes = await planDocumentDocx(draft)
    expect(bytes[0]).toBe(0x50)
    expect(bytes[1]).toBe(0x4b)
  })
})
