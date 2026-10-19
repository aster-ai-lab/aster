import { mergeAttributes, Node } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import Suggestion from '@tiptap/suggestion'

import { findNote, noteTitle } from './knowledgeLinks'

export interface WikiNoteRef {
  name: string
  path: string
}

export interface WikiSuggestRect {
  left: number
  top: number
  bottom: number
}

export interface WikiSuggestState {
  items: WikiNoteRef[]
  selected: number
  rect: WikiSuggestRect | null
  select: (item: WikiNoteRef) => void
}

const WIKI_TOKEN = /^\[\[([^\]|]+?)(?:\|([^\]]+))?\]\]/

/** Titles of listed notes that match the text typed after `[[`. */
export function wikiLinkCandidates(notes: readonly WikiNoteRef[], query: string): WikiNoteRef[] {
  const needle = query.trim().toLowerCase()
  return notes
    .filter((note) => {
      const title = noteTitle(note.name).toLowerCase()
      return !needle || title.includes(needle) || note.name.toLowerCase().includes(needle)
    })
    .slice(0, 8)
}

/** Wikilink source. A display name that repeats the target stays `[[target]]`. */
export function renderWikiLink(target: string, label: string): string {
  const page = target.trim()
  const shown = label.trim()
  if (!page) return ''
  if (!shown || shown === page) return `[[${page}]]`
  return `[[${page}|${shown}]]`
}

/** File name for an unresolved target. Separators and reserved names are refused. */
export function unresolvedFileName(target: string): { name: string } | { error: 'empty' | 'invalid' | 'reserved' } {
  const raw = target.trim()
  if (!raw || raw === '.' || raw === '..') return { error: 'empty' }
  if (raw.includes('/') || raw.includes('\\') || raw.includes('#') || raw.startsWith('.')) return { error: 'invalid' }
  const name = /\.(?:md|markdown)$/i.test(raw) ? raw : `${raw}.md`
  if (name === 'index.md' || name === 'log.md') return { error: 'reserved' }
  return { name }
}

export function planUnresolvedNote(
  notes: readonly WikiNoteRef[],
  target: string
):
  | { kind: 'open'; path: string }
  | { kind: 'create'; name: string }
  | { kind: 'refuse'; error: 'empty' | 'invalid' | 'reserved' } {
  const existing = findNote(notes, target)
  if (existing) return { kind: 'open', path: existing.path }
  const parsed = unresolvedFileName(target)
  if ('error' in parsed) return { kind: 'refuse', error: parsed.error }
  return { kind: 'create', name: parsed.name }
}

export function createKnowledgeWikiLink(options: {
  notes: readonly WikiNoteRef[]
  onOpen: (path: string) => void
  onSuggest: (state: WikiSuggestState | null) => void
}) {
  return Node.create({
    name: 'knowledgeWikiLink',
    group: 'inline',
    inline: true,
    atom: true,
    selectable: true,
    addOptions() {
      return {
        notes: options.notes,
        onOpen: options.onOpen,
        onSuggest: options.onSuggest
      }
    },
    addAttributes() {
      return {
        target: { default: '' },
        label: { default: '' }
      }
    },
    parseHTML() {
      return [
        {
          tag: 'span[data-wiki-link]',
          getAttrs: (element) => {
            if (!(element instanceof HTMLElement)) return false
            const target = element.getAttribute('data-wiki-link') ?? ''
            return { target, label: element.textContent ?? target }
          }
        }
      ]
    },
    renderHTML({ HTMLAttributes, node }) {
      const target = String(node.attrs.target ?? '')
      const label = String(node.attrs.label ?? '') || target
      return ['span', mergeAttributes(HTMLAttributes, { 'data-wiki-link': target }), label]
    },
    markdownTokenName: 'knowledgeWikiLink',
    markdownTokenizer: {
      name: 'knowledgeWikiLink',
      level: 'inline',
      start: (src: string) => src.indexOf('[['),
      tokenize: (src: string) => {
        const match = WIKI_TOKEN.exec(src)
        if (!match?.[1]) return undefined
        const target = match[1].trim()
        const label = (match[2] ?? target).trim()
        return { type: 'knowledgeWikiLink', raw: match[0], target, label }
      }
    },
    parseMarkdown: (token, helpers) => {
      const target = String(token.target ?? '').trim()
      const label = String(token.label ?? target).trim()
      return helpers.createNode('knowledgeWikiLink', { target, label })
    },
    renderMarkdown: (node) => renderWikiLink(String(node.attrs?.target ?? ''), String(node.attrs?.label ?? '')),
    addProseMirrorPlugins() {
      const wiki = this
      return [
        Suggestion({
          editor: this.editor,
          pluginKey: new PluginKey('knowledgeWikiSuggest'),
          char: '[[',
          allowedPrefixes: null,
          items: ({ query }) => wikiLinkCandidates(wiki.options.notes, query),
          command: ({ editor, range, props }) => {
            const title = noteTitle(props.name)
            editor
              .chain()
              .focus()
              .insertContentAt(range, {
                type: 'knowledgeWikiLink',
                attrs: { target: title, label: title }
              })
              .run()
          },
          render: () => {
            let current: {
              items: WikiNoteRef[]
              command: (item: WikiNoteRef) => void
              clientRect?: (() => DOMRect | null) | null
            } | null = null
            let selected = 0
            const publish = () => {
              if (!current) {
                wiki.options.onSuggest(null)
                return
              }
              const box = current.clientRect?.() ?? null
              wiki.options.onSuggest({
                items: current.items,
                selected,
                rect: box ? { left: box.left, top: box.top, bottom: box.bottom } : null,
                select: (item) => current?.command(item)
              })
            }
            return {
              onStart: (props) => {
                current = props
                selected = 0
                publish()
              },
              onUpdate: (props) => {
                current = props
                selected = 0
                publish()
              },
              onExit: () => {
                current = null
                wiki.options.onSuggest(null)
              },
              onKeyDown: ({ event }) => {
                if (!current || current.items.length === 0) return false
                if (event.key === 'ArrowDown') {
                  selected = Math.min(selected + 1, current.items.length - 1)
                  publish()
                  return true
                }
                if (event.key === 'ArrowUp') {
                  selected = Math.max(selected - 1, 0)
                  publish()
                  return true
                }
                if (event.key === 'Enter') {
                  const item = current.items[selected]
                  if (!item) return false
                  current.command(item)
                  return true
                }
                return false
              }
            }
          }
        }),
        new Plugin({
          key: new PluginKey('knowledgeWikiClick'),
          props: {
            handleClickOn: (_view, _pos, node) => {
              if (node.type.name !== 'knowledgeWikiLink') return false
              const found = findNote(wiki.options.notes, String(node.attrs.target ?? ''))
              if (found) wiki.options.onOpen(found.path)
              return true
            }
          }
        })
      ]
    }
  })
}
