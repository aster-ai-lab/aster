import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  createOfficialWritingPreviewCache,
  syncOfficialWritingPreview,
  type OfficialWritingPreviewCache
} from '../officialWritingPreviewSync'
import { OFFICIAL_WRITING_SLOT_BORDER, officialWritingTemplateHtml } from '../officialWritingTemplate'

const editors: Editor[] = []

interface FakeMark {
  type: { name: string; create: (attrs: Record<string, unknown>) => FakeMark }
  attrs: { fontFamily: string; runBorder: string | null }
}

interface FakeText {
  isText: true
  text: string
  marks: FakeMark[]
}

function textStyleMark(runBorder: string | null): FakeMark {
  return {
    type: {
      name: 'textStyle',
      create(attrs) {
        return textStyleMark(typeof attrs.runBorder === 'string' ? attrs.runBorder : null)
      }
    },
    attrs: { fontFamily: 'SimSun', runBorder }
  }
}

function textNode(text: string, runBorder: string | null): FakeText {
  return { isText: true, text, marks: [textStyleMark(runBorder)] }
}

function fakeEditor(nodes: FakeText[]) {
  const ranges = () => {
    let pos = 1
    return nodes.map((node) => {
      const from = pos
      pos += node.text.length
      return { node, from, to: pos }
    })
  }
  const doc = {
    descendants(visitor: (node: FakeText, pos: number) => void) {
      for (const range of ranges()) visitor(range.node, range.from)
    },
    nodesBetween(from: number, to: number, visitor: (node: FakeText) => void) {
      for (const range of ranges()) {
        if (range.to > from && range.from < to) visitor(range.node)
      }
    },
    textBetween(from: number, to: number) {
      const text = nodes.map((node) => node.text).join('')
      return text.slice(from - 1, to - 1)
    }
  }
  const transaction = {
    doc,
    docChanged: false,
    replaceWith(from: number, to: number, content: unknown) {
      const index = ranges().findIndex((range) => range.from === from && range.to === to)
      if (index >= 0) nodes[index] = content as FakeText
      transaction.docChanged = true
      return transaction
    },
    setMeta() {
      return transaction
    }
  }
  return {
    commands: {
      setContent: (_html: string) => true
    },
    state: {
      doc,
      schema: {
        text(text: string, marks?: readonly unknown[] | null) {
          return { isText: true as const, text, marks: [...(marks ?? [])] }
        }
      },
      get tr() {
        transaction.docChanged = false
        return transaction
      }
    },
    view: {
      dispatch() {}
    }
  }
}

function mount(html: string) {
  const editor = new Editor({
    extensions: [StarterKit],
    content: html,
    immediatelyRender: true
  })
  editors.push(editor)
  return editor
}

function sync(
  editor: Editor,
  cache: OfficialWritingPreviewCache,
  html: string,
  fields: { org?: string; subject?: string; mainTo?: string } = {},
  brief = ''
) {
  syncOfficialWritingPreview(editor, {
    brief,
    cache,
    fields,
    filledHtml: html,
    structureHtml: officialWritingTemplateHtml('notice', '') ?? ''
  })
}

afterEach(() => {
  for (const editor of editors) editor.destroy()
  editors.length = 0
})

describe('officialWritingPreviewSync', () => {
  it('patches placeholder text without calling setContent again', () => {
    const structure = officialWritingTemplateHtml('notice', '') ?? ''
    const editor = mount(structure)
    const setContent = vi.spyOn(editor.commands, 'setContent')
    const cache = createOfficialWritingPreviewCache()

    const first = officialWritingTemplateHtml('notice', '', {}, { org: '市教育局', mainTo: '各学校' }) ?? ''
    sync(editor, cache, first, { org: '市教育局', mainTo: '各学校' })
    expect(editor.getText()).toContain('市教育局文件')
    expect(editor.getText()).toContain('市教育局关于事由的通知')
    expect(editor.getText()).toContain('各学校：')
    expect(editor.getText()).not.toContain('{{')
    expect(editor.getText()).not.toContain('【')
    expect(setContent).not.toHaveBeenCalled()

    const second = officialWritingTemplateHtml('notice', '', {}, { org: '市教育局办公室', mainTo: '各学校' }) ?? ''
    sync(editor, cache, second, { org: '市教育局办公室', mainTo: '各学校' })
    expect(editor.getText()).toContain('市教育局办公室文件')
    expect(editor.getText()).not.toContain('发文机关文件')
    expect(editor.getText()).not.toContain('发文机关关于')
    expect(setContent).not.toHaveBeenCalled()
  })

  it('fills the subject slot and leaves the section heading', () => {
    const structure = officialWritingTemplateHtml('notice', '') ?? ''
    const editor = mount(structure)
    const cache = createOfficialWritingPreviewCache()
    const filled =
      officialWritingTemplateHtml(
        'notice',
        '请写明事项。',
        {},
        { org: '市教育局', subject: '周五开会', mainTo: '各学校' }
      ) ?? ''
    sync(editor, cache, filled, { org: '市教育局', subject: '周五开会', mainTo: '各学校' }, '请写明事项。')
    const text = editor.getText()
    expect(text).toContain('市教育局关于周五开会的通知')
    expect(text).toContain('一、事由与依据')
    expect(text).toContain('请写明事项。')
    expect(text).not.toContain('一、周五开会与依据')
  })

  it('writes the note into the first section slot and restores an empty field', () => {
    const structure = officialWritingTemplateHtml('notice', '') ?? ''
    const editor = mount(structure)
    const cache = createOfficialWritingPreviewCache()
    const noted = officialWritingTemplateHtml('notice', '周五开会', {}, { org: '市教育局' }) ?? ''
    sync(editor, cache, noted, { org: '市教育局' }, '周五开会')
    expect(editor.getText()).toContain('周五开会')
    expect(editor.getText()).not.toContain('待填写：事由与依据')
    expect(editor.getText()).toContain('待填写：具体事项')

    const cleared = officialWritingTemplateHtml('notice', '周五开会', {}, {}) ?? ''
    sync(editor, cache, cleared, {}, '周五开会')
    expect(editor.getText()).toContain('发文机关文件')
    expect(editor.getText()).toContain('周五开会')
  })

  it('fills the new skeleton after a structure change without a second field refresh', () => {
    const structure = officialWritingTemplateHtml('notice', '') ?? ''
    const editor = mount(structure)
    const setContent = vi.spyOn(editor.commands, 'setContent')
    const cache = createOfficialWritingPreviewCache()
    sync(editor, cache, officialWritingTemplateHtml('notice', '', {}, { org: '市教育局' }) ?? '', { org: '市教育局' })
    expect(setContent).not.toHaveBeenCalled()

    const plain = officialWritingTemplateHtml('notice', '', { banner: false }, { org: '市教育局' }) ?? ''
    editor.commands.setContent(officialWritingTemplateHtml('notice', '', { banner: false }) ?? '', {
      emitUpdate: false
    })
    setContent.mockClear()
    syncOfficialWritingPreview(editor, {
      brief: '',
      cache,
      fields: { org: '市教育局' },
      filledHtml: plain,
      structureHtml: officialWritingTemplateHtml('notice', '', { banner: false }) ?? ''
    })
    expect(editor.getText()).not.toContain('市教育局文件')
    expect(editor.getText()).toContain('市教育局关于事由的通知')
    expect(setContent).not.toHaveBeenCalled()
  })

  it('drops the dashed border when a value replaces the name and restores it when the field is cleared', () => {
    const border = OFFICIAL_WRITING_SLOT_BORDER
    const nodes = [
      textNode('发文机关', border),
      textNode('文件', null),
      textNode('事由', border),
      textNode('待填写：事由与依据', border),
      textNode('待填写：具体事项', border)
    ]
    const editor = fakeEditor(nodes)
    const setContent = vi.spyOn(editor.commands, 'setContent')
    const cache = createOfficialWritingPreviewCache()
    const structure = officialWritingTemplateHtml('notice', '') ?? ''

    syncOfficialWritingPreview(editor, {
      brief: '周五开会',
      cache,
      fields: { org: '市教育局' },
      filledHtml: structure,
      structureHtml: structure
    })
    expect(nodes[0]?.text).toBe('市教育局')
    expect(nodes[0]?.marks[0]?.attrs.runBorder).toBeNull()
    expect(nodes[1]?.text).toBe('文件')
    expect(nodes[1]?.marks[0]?.attrs.runBorder).toBeNull()
    expect(nodes[2]?.text).toBe('事由')
    expect(nodes[2]?.marks[0]?.attrs.runBorder).toBe(border)
    expect(nodes[3]?.text).toBe('周五开会')
    expect(nodes[3]?.marks[0]?.attrs.runBorder).toBeNull()
    expect(nodes[4]?.text).toBe('待填写：具体事项')
    expect(nodes[4]?.marks[0]?.attrs.runBorder).toBe(border)
    expect(setContent).not.toHaveBeenCalled()

    syncOfficialWritingPreview(editor, {
      brief: '周五开会',
      cache,
      fields: {},
      filledHtml: structure,
      structureHtml: structure
    })
    expect(nodes[0]?.text).toBe('发文机关')
    expect(nodes[0]?.marks[0]?.attrs.runBorder).toBe(border)
    expect(setContent).not.toHaveBeenCalled()
  })
})
