import { Extension } from '@tiptap/core'
import type { Node as ProseMirrorNode } from '@tiptap/pm/model'

/** First case-insensitive match of `needle` in the document, as a text selection. */
export function findRevealRange(doc: ProseMirrorNode, needle: string): { from: number; to: number } | null {
  const query = needle.trim().toLowerCase()
  if (!query) return null
  let found: { from: number; to: number } | null = null
  doc.descendants((node, pos) => {
    if (found || !node.isText || !node.text) return
    const at = node.text.toLowerCase().indexOf(query)
    if (at < 0) return
    found = { from: pos + at, to: pos + at + query.length }
    return false
  })
  return found
}

export function createKnowledgeReveal(text: string) {
  return Extension.create({
    name: 'knowledgeReveal',
    addOptions() {
      return { text }
    },
    onCreate() {
      const range = findRevealRange(this.editor.state.doc, this.options.text)
      if (!range) return
      this.editor.commands.setTextSelection(range)
      try {
        this.editor.commands.scrollIntoView()
      } catch {
        // A host without layout still keeps the selection.
      }
    }
  })
}
