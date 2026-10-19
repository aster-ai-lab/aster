import { Extension, type Editor } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'

type Range = { from: number; to: number }
const attentionKey = new PluginKey<Range | null>('studioReviewAttention')

/** Work-style attention is ephemeral; it never creates a document comment. */
export const ReviewAttentionDecorations = Extension.create({
  name: 'studioReviewAttention',
  addProseMirrorPlugins() {
    return [
      new Plugin<Range | null>({
        key: attentionKey,
        state: {
          init: () => null,
          apply(transaction, previous) {
            const patch = transaction.getMeta(attentionKey) as { range: Range | null } | undefined
            if (patch) return patch.range
            if (!previous || !transaction.docChanged) return previous
            return { from: transaction.mapping.map(previous.from, 1), to: transaction.mapping.map(previous.to, -1) }
          }
        },
        props: {
          decorations(state) {
            const range = attentionKey.getState(state)
            return range && range.to > range.from
              ? DecorationSet.create(state.doc, [
                  Decoration.inline(range.from, range.to, {
                    class: 'workspace-review-attention workspace-review-attention--primary',
                    'data-review-attention': 'true'
                  })
                ])
              : null
          }
        }
      })
    ]
  }
})

export function setReviewAttention(editor: Editor, quote: string) {
  if (editor.isDestroyed || !editor.view?.dispatch || !editor.state.tr) return
  const range = quote ? reviewQuoteRange(editor.state.doc, quote) : null
  const previous = attentionKey.getState(editor.state)
  if (previous?.from === range?.from && previous?.to === range?.to) return
  editor.view.dispatch(editor.state.tr.setMeta(attentionKey, { range }))
  const mark = editor.view.dom.querySelector('[data-review-attention]')
  const scroller = mark?.closest('.work-document-scroll')
  if (mark && scroller instanceof HTMLElement) {
    const frame = scroller.getBoundingClientRect()
    const rect = mark.getBoundingClientRect()
    scroller.scrollTo({ top: Math.max(0, scroller.scrollTop + rect.top - frame.top - 48), behavior: 'smooth' })
  }
}

export function reviewQuoteRange(
  doc: { descendants: (visitor: (node: { isText: boolean; text?: string }, pos: number) => boolean | void) => void },
  quote: string
): { from: number; to: number } | null {
  const trimmed = quote.trim()
  if (!trimmed) return null
  let source = ''
  const docPosAt: number[] = []
  let lastEnd: number | null = null
  doc.descendants((node, pos) => {
    if (!node.isText || !node.text) return
    if (lastEnd !== null && pos > lastEnd) {
      source += ' '
      docPosAt.push(-1)
    }
    for (let index = 0; index < node.text.length; index += 1) {
      source += node.text[index]
      docPosAt.push(pos + index)
    }
    lastEnd = pos + node.text.length
  })
  const start = source.indexOf(trimmed)
  if (start < 0) return null
  if (source.indexOf(trimmed, start + 1) >= 0) return null
  let from = -1
  let to = -1
  for (let index = start; index < start + trimmed.length; index += 1) {
    const position = docPosAt[index]
    if (position === undefined || position < 0) continue
    if (from < 0) from = position
    to = position + 1
  }
  return from >= 0 && to > from ? { from, to } : null
}
