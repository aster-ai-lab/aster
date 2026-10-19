import type { Editor } from '@tiptap/core'

/** Keep the drag handle from floating over stale content after a focused editor scrolls. */
export function hideDragHandleOnScroll(editor: Editor | null | undefined): void {
  if (editor?.view.hasFocus()) editor.commands.setMeta('hideDragHandle', true)
}
