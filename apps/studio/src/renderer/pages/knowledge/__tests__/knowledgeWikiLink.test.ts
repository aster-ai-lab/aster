import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { Editor } from '@tiptap/core'
import { Markdown } from '@tiptap/markdown'
import type { Node as ProseMirrorNode } from '@tiptap/pm/model'
import { StarterKit } from '@tiptap/starter-kit'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { createKnowledgeReveal } from '../knowledgeReveal'
import {
  createKnowledgeWikiLink,
  planUnresolvedNote,
  renderWikiLink,
  unresolvedFileName,
  wikiLinkCandidates,
  type WikiSuggestState
} from '../knowledgeWikiLink'

let editor: Editor | undefined

afterEach(() => editor?.destroy())

const notes = [
  { name: '乙.md', path: '/vault/乙.md' },
  { name: '甲.md', path: '/vault/甲.md' },
  { name: '目标.md', path: '/vault/目标.md' }
]

function wikiNode(doc: ProseMirrorNode, target: string) {
  let found: { node: ProseMirrorNode; pos: number } | null = null
  doc.descendants((node, pos) => {
    if (found || node.type.name !== 'knowledgeWikiLink' || node.attrs.target !== target) return
    found = { node, pos }
    return false
  })
  return found
}

describe('knowledge wikilinks', () => {
  it('round-trips bracket links and does not turn them into urls', () => {
    editor = new Editor({
      element: document.createElement('div'),
      extensions: [
        StarterKit,
        createKnowledgeWikiLink({ notes, onOpen: () => undefined, onSuggest: () => undefined }),
        Markdown
      ],
      content: '见 [[目标]] 和 [[目标|显示名]]',
      contentType: 'markdown'
    })
    expect(editor.getMarkdown()).toBe('见 [[目标]] 和 [[目标|显示名]]')
    expect(editor.getMarkdown()).not.toContain('](')
    expect(wikiNode(editor.state.doc, '目标')?.node.attrs.label).toBe('目标')
    const named = editor.state.doc
    let label = ''
    named.descendants((node) => {
      if (node.type.name === 'knowledgeWikiLink' && node.attrs.label === '显示名') label = String(node.attrs.target)
    })
    expect(label).toBe('目标')
    expect(renderWikiLink('目标', '目标')).toBe('[[目标]]')
    expect(renderWikiLink('目标', '显示名')).toBe('[[目标|显示名]]')
  })

  it('offers listed titles after [[ and opens a resolved link without creating a file', async () => {
    const offered: string[][] = []
    let opened = ''
    editor = new Editor({
      element: document.createElement('div'),
      extensions: [
        StarterKit,
        createKnowledgeWikiLink({
          notes,
          onOpen: (path) => {
            opened = path
          },
          onSuggest: (state: WikiSuggestState | null) => {
            offered.push((state?.items ?? []).map((item) => item.name))
          }
        }),
        Markdown
      ]
    })
    editor.commands.insertContent('[[乙')
    await vi.waitFor(() => {
      expect(offered.at(-1)).toEqual(['乙.md'])
    })
    expect(wikiLinkCandidates(notes, '甲').map((item) => item.name)).toEqual(['甲.md'])

    editor.commands.setContent('见 [[乙]]', { contentType: 'markdown' })
    const hit = wikiNode(editor.state.doc, '乙')
    expect(hit).not.toBeNull()
    const handled = editor.view.someProp('handleClickOn', (handle) =>
      handle(editor!.view, hit!.pos, hit!.node, hit!.pos, new MouseEvent('click'), true)
    )
    expect(handled).toBe(true)
    expect(opened).toBe('/vault/乙.md')

    editor.commands.setContent('[[还没有]]', { contentType: 'markdown' })
    const missing = wikiNode(editor.state.doc, '还没有')
    expect(missing).not.toBeNull()
    opened = ''
    editor.view.someProp('handleClickOn', (handle) =>
      handle(editor!.view, missing!.pos, missing!.node, missing!.pos, new MouseEvent('click'), true)
    )
    expect(opened).toBe('')
  })

  it('moves the caret list with the arrow keys and inserts the selected title', async () => {
    const offered: Array<WikiSuggestState | null> = []
    editor = new Editor({
      element: document.createElement('div'),
      extensions: [
        StarterKit,
        createKnowledgeWikiLink({
          notes,
          onOpen: () => undefined,
          onSuggest: (state) => {
            offered.push(state)
          }
        }),
        Markdown
      ]
    })
    editor.commands.insertContent('[[')
    await vi.waitFor(() => expect((offered.at(-1)?.items.length ?? 0) > 1).toBe(true))
    const down = new KeyboardEvent('keydown', { key: 'ArrowDown' })
    editor.view.someProp('handleKeyDown', (handle) => handle(editor!.view, down))
    await vi.waitFor(() => expect(offered.at(-1)?.selected).toBe(1))
    const chosen = offered.at(-1)?.items[1]?.name.replace(/\.md$/u, '') ?? ''
    editor.view.someProp('handleKeyDown', (handle) =>
      handle(editor!.view, new KeyboardEvent('keydown', { key: 'Escape' }))
    )
    await vi.waitFor(() => expect(offered.at(-1)).toBeNull())
    editor.commands.insertContent('[[')
    await vi.waitFor(() => expect((offered.at(-1)?.items.length ?? 0) > 1).toBe(true))
    editor.view.someProp('handleKeyDown', (handle) =>
      handle(editor!.view, new KeyboardEvent('keydown', { key: 'ArrowDown' }))
    )
    await vi.waitFor(() => expect(offered.at(-1)?.selected).toBe(1))
    editor.view.someProp('handleKeyDown', (handle) =>
      handle(editor!.view, new KeyboardEvent('keydown', { key: 'Enter' }))
    )
    await vi.waitFor(() => expect(editor!.getMarkdown()).toContain(`[[${chosen}]]`))
  })

  it('reads note titles from the same list after that list gains a note', async () => {
    const live = notes.map((note) => ({ ...note }))
    const offered: string[][] = []
    editor = new Editor({
      element: document.createElement('div'),
      extensions: [
        StarterKit,
        createKnowledgeWikiLink({
          notes: live,
          onOpen: () => undefined,
          onSuggest: (state: WikiSuggestState | null) => {
            offered.push((state?.items ?? []).map((item) => item.name))
          }
        }),
        Markdown
      ]
    })
    live.push({ name: '后来.md', path: '/vault/后来.md' })
    editor.commands.insertContent('[[后来')
    await vi.waitFor(() => {
      expect(offered.at(-1)).toEqual(['后来.md'])
    })
  })

  it('refuses an empty, separated, or reserved name and opens an existing title', () => {
    expect(unresolvedFileName('')).toEqual({ error: 'empty' })
    expect(unresolvedFileName('目录/秘密')).toEqual({ error: 'invalid' })
    expect(unresolvedFileName('index.md')).toEqual({ error: 'reserved' })
    expect(unresolvedFileName('log')).toEqual({ error: 'reserved' })
    expect(planUnresolvedNote(notes, '乙')).toEqual({ kind: 'open', path: '/vault/乙.md' })
    expect(planUnresolvedNote(notes, '团队纪要')).toEqual({ kind: 'create', name: '团队纪要.md' })
  })

  it('reveals the matched text inside the note editor', async () => {
    editor = new Editor({
      element: document.createElement('div'),
      extensions: [StarterKit, createKnowledgeReveal('买船票'), Markdown],
      content: '<p>下午买船票</p>'
    })
    await vi.waitFor(() => {
      expect(editor!.state.doc.textBetween(editor!.state.selection.from, editor!.state.selection.to)).toBe('买船票')
    })
  })

  it('stays off the console article editor', () => {
    const source = readFileSync(join(process.cwd(), 'src/renderer/pages/documents/OfficeDocumentSurface.tsx'), 'utf8')
    expect(source).not.toContain('knowledgeWikiLink')
    expect(source).not.toContain('createKnowledgeWikiLink')
  })
})
