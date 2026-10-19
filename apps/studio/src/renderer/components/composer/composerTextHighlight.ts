import { Extension } from '@tiptap/core'
import type { Node as ProseMirrorNode } from '@tiptap/pm/model'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'

/** Colors plain-text spans in the composer without changing the draft text. */
export interface ComposerTextHighlight {
  /** Global regex; each match is decorated. Must be stable across renders. */
  pattern: RegExp
  className: string
  /** Extra attributes for the decorated span, e.g. a `data-*` marker for tests and styling. */
  attributes?: Record<string, string>
  /** Per-match attributes, such as the avatar of the agent named in the span. */
  attributesForMatch?: (text: string) => Record<string, string> | undefined
  /** DOM node painted at the start of the match. Used when a CSS `::before` does not show in the editor. */
  leadingWidget?: (text: string) => HTMLElement
}

const composerTextHighlightKey = new PluginKey<DecorationSet>('composerTextHighlight')

function buildDecorations(doc: ProseMirrorNode, highlights: readonly ComposerTextHighlight[]) {
  const decorations: Decoration[] = []
  doc.descendants((node, pos) => {
    if (!node.isText || !node.text) return
    for (const highlight of highlights) {
      const pattern = new RegExp(
        highlight.pattern.source,
        highlight.pattern.flags.includes('g') ? highlight.pattern.flags : `${highlight.pattern.flags}g`
      )
      for (const match of node.text.matchAll(pattern)) {
        if (!match[0] || match.index === undefined) continue
        const from = pos + match.index
        const matched = match[0]
        if (highlight.leadingWidget) {
          const renderWidget = highlight.leadingWidget
          decorations.push(Decoration.widget(from, () => renderWidget(matched), { side: -1 }))
        }
        decorations.push(
          Decoration.inline(from, from + match[0].length, {
            class: highlight.className,
            ...highlight.attributes,
            ...highlight.attributesForMatch?.(match[0])
          })
        )
      }
    }
  })
  return DecorationSet.create(doc, decorations)
}

export function createComposerTextHighlightExtension(highlights: readonly ComposerTextHighlight[]) {
  return Extension.create({
    name: 'composerTextHighlight',

    addProseMirrorPlugins() {
      return [
        new Plugin<DecorationSet>({
          key: composerTextHighlightKey,
          state: {
            init: (_, state) => buildDecorations(state.doc, highlights),
            apply: (transaction, previous) =>
              transaction.docChanged ? buildDecorations(transaction.doc, highlights) : previous
          },
          props: {
            decorations: (state) => composerTextHighlightKey.getState(state)
          }
        })
      ]
    }
  })
}
