import type { Editor } from '@tiptap/core'
import { TextSelection } from '@tiptap/pm/state'

import {
  type CaretState,
  containsMacFunctionKey,
  type EditingAction,
  editingActionForKey,
  forwardDeleteRange,
  macFunctionKeyAction,
  moveCaret,
  pageLineCount
} from '@renderer/utils/editingKeys'

import { getComposerInputText, getComposerPositionAtTextOffset, getComposerTextOffset } from './quickPanel/bridge'

const goalColumns = new WeakMap<Editor, number>()

/** Move the homepage and chat composer caret. Private-use keys never become text. */
export function applyComposerEditingAction(
  editor: Editor,
  action: EditingAction,
  extend: boolean,
  viewHeight: number,
  lineHeight: number
): void {
  if (action === 'ignore' || editor.isDestroyed) return
  const text = getComposerInputText(editor)
  const { anchor, head } = editor.state.selection
  const anchorOffset = getComposerTextOffset(editor, anchor)
  const headOffset = getComposerTextOffset(editor, head)
  const selection = caretFromOffsets(anchorOffset, headOffset)

  if (action === 'deleteForward') {
    if (!editor.isEditable) return
    const range = forwardDeleteRange(text, selection)
    if (!range) return
    const from = getComposerPositionAtTextOffset(editor, range.start)
    const to = getComposerPositionAtTextOffset(editor, range.end)
    if (to <= from) return
    editor.view.dispatch(editor.state.tr.delete(from, to).scrollIntoView())
    goalColumns.delete(editor)
    return
  }

  const vertical = action === 'up' || action === 'down' || action === 'pageUp' || action === 'pageDown'
  const moved = moveCaret(
    text,
    selection,
    action,
    extend,
    pageLineCount(viewHeight, lineHeight),
    vertical ? (goalColumns.get(editor) ?? null) : null
  )
  if (moved.goalColumn === null) goalColumns.delete(editor)
  else goalColumns.set(editor, moved.goalColumn)

  const movedAnchor = moved.selection.direction === 'backward' ? moved.selection.end : moved.selection.start
  const movedHead = moved.selection.direction === 'backward' ? moved.selection.start : moved.selection.end
  const nextAnchor = getComposerPositionAtTextOffset(editor, movedAnchor)
  const nextHead = getComposerPositionAtTextOffset(editor, movedHead)
  const transaction = editor.state.tr.setSelection(TextSelection.create(editor.state.doc, nextAnchor, nextHead))
  editor.view.dispatch(transaction.scrollIntoView())
}

interface EditingKeyEvent {
  key: string
  metaKey: boolean
  ctrlKey: boolean
  shiftKey: boolean
  altKey: boolean
  isComposing: boolean
  defaultPrevented: boolean
  preventDefault: () => void
}

/** Move the composer caret for Home, End, Page Up, Page Down, and private-use keys. */
export function applyComposerEditingKey(
  editor: Editor,
  event: EditingKeyEvent,
  viewHeight: number,
  lineHeight: number
): boolean {
  if (event.isComposing || event.key === 'Process' || event.altKey || event.defaultPrevented || editor.isDestroyed) {
    return false
  }
  const action = editingActionForKey(event.key, { command: event.metaKey || event.ctrlKey })
  if (!action) return false
  event.preventDefault()
  if (action !== 'ignore') applyComposerEditingAction(editor, action, event.shiftKey, viewHeight, lineHeight)
  return true
}

/**
 * Refuse a private-use insert. Returns true when the text must not enter the document.
 * Call this only when keydown did not already move the caret.
 */
export function applyComposerFunctionKeyText(
  editor: Editor,
  insertedText: string,
  viewHeight: number,
  lineHeight: number
): boolean {
  if (!containsMacFunctionKey(insertedText) || editor.isDestroyed) return false
  const action = macFunctionKeyAction(insertedText)
  if (action && action !== 'ignore') applyComposerEditingAction(editor, action, false, viewHeight, lineHeight)
  return true
}

function caretFromOffsets(anchor: number, head: number): CaretState {
  if (anchor === head) return { start: anchor, end: head, direction: 'none' }
  if (head < anchor) return { start: head, end: anchor, direction: 'backward' }
  return { start: anchor, end: head, direction: 'forward' }
}
