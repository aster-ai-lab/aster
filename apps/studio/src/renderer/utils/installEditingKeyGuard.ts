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

const TEXT_INPUT_TYPES = new Set(['', 'text', 'search', 'url', 'tel', 'password', 'email', 'number'])
const goalColumns = new WeakMap<HTMLElement, number>()

/**
 * One pair of listeners per window. Capture `beforeinput` drops the private-use
 * character before any editor inserts it. Bubble `keydown` then moves the caret,
 * after a composer that already handled the key.
 */
export function installEditingKeyGuard(): () => void {
  // keydown runs before beforeinput. A claimed key must not move the caret twice.
  let claimed = false

  const onKeyDown = (event: KeyboardEvent) => {
    claimed = false
    if (event.isComposing || event.key === 'Process' || event.altKey) return
    if (event.defaultPrevented) {
      claimed = true
      return
    }
    const action = editingActionForKey(event.key, { command: event.metaKey || event.ctrlKey })
    if (!action) return
    const host = editingHost(event.target)
    if (!host) return
    event.preventDefault()
    claimed = true
    if (action === 'ignore') return
    if (host instanceof HTMLInputElement || host instanceof HTMLTextAreaElement) {
      applyTextControl(host, action, event.shiftKey)
      return
    }
    applyContentEditable(host, action, event.shiftKey)
  }

  const onBeforeInput = (event: InputEvent) => {
    if (event.isComposing || !event.data || !containsMacFunctionKey(event.data)) return
    event.preventDefault()
    event.stopPropagation()
    if (claimed) return
    const host = editingHost(event.target)
    if (!(host instanceof HTMLInputElement || host instanceof HTMLTextAreaElement)) return
    const action = macFunctionKeyAction(event.data)
    if (!action || action === 'ignore') return
    claimed = true
    applyTextControl(host, action, false)
  }

  document.addEventListener('keydown', onKeyDown)
  document.addEventListener('beforeinput', onBeforeInput, true)
  return () => {
    document.removeEventListener('keydown', onKeyDown)
    document.removeEventListener('beforeinput', onBeforeInput, true)
  }
}

function editingHost(target: EventTarget | null): HTMLInputElement | HTMLTextAreaElement | HTMLElement | null {
  if (target instanceof HTMLTextAreaElement) return target.disabled || target.readOnly ? null : target
  if (target instanceof HTMLInputElement) {
    if (target.disabled || target.readOnly || !TEXT_INPUT_TYPES.has(target.type)) return null
    return target
  }
  const element = target instanceof HTMLElement ? target : target instanceof Node ? target.parentElement : null
  if (!element) return null
  // The attribute is the source of truth. "true", "" and "plaintext-only" edit.
  // "inherit" walks up. isContentEditable covers a child of that host.
  const marked = element.closest('[contenteditable]')
  if (marked instanceof HTMLElement) {
    const mode = (marked.getAttribute('contenteditable') ?? '').toLowerCase()
    if (mode === 'false') return null
    if (mode !== 'inherit') return marked
  }
  if (!element.isContentEditable) return null
  return element
}

function applyTextControl(
  element: HTMLInputElement | HTMLTextAreaElement,
  action: EditingAction,
  extend: boolean
): void {
  const selection = readSelection(element)
  if (action === 'deleteForward') {
    if (element.readOnly) return
    const range = forwardDeleteRange(element.value, selection)
    if (!range) return
    const next = element.value.slice(0, range.start) + element.value.slice(range.end)
    element.value = next
    element.setSelectionRange(range.start, range.start, 'none')
    element.dispatchEvent(new Event('input', { bubbles: true }))
    element.dispatchEvent(new Event('select', { bubbles: true }))
    goalColumns.delete(element)
    return
  }

  const vertical = action === 'up' || action === 'down' || action === 'pageUp' || action === 'pageDown'
  const moved = moveCaret(
    element.value,
    selection,
    action,
    extend,
    linesInView(element),
    vertical ? (goalColumns.get(element) ?? null) : null,
    element instanceof HTMLInputElement
  )
  if (moved.goalColumn === null) goalColumns.delete(element)
  else goalColumns.set(element, moved.goalColumn)
  const direction = moved.selection.direction === 'none' ? 'none' : moved.selection.direction
  element.setSelectionRange(moved.selection.start, moved.selection.end, direction)
  element.dispatchEvent(new Event('select', { bubbles: true }))
}

function applyContentEditable(element: HTMLElement, action: EditingAction, extend: boolean): void {
  const selection = document.getSelection()
  if (!selection) return
  if (action === 'deleteForward') {
    document.execCommand('forwardDelete')
    return
  }
  if (typeof selection.modify === 'function') {
    const alter = extend ? 'extend' : 'move'
    const steps = action === 'pageUp' || action === 'pageDown' ? linesInView(element) : 1
    for (let index = 0; index < steps; index += 1) modifySelection(selection, alter, action)
    return
  }
  const textNode = element.firstChild
  if (!(textNode instanceof Text)) return
  const current = selectionFromDom(selection, textNode)
  if (!current) return
  const moved = moveCaret(textNode.data, current, action, extend, linesInView(element), null, false)
  const range = document.createRange()
  range.setStart(textNode, moved.selection.start)
  range.setEnd(textNode, moved.selection.end)
  selection.removeAllRanges()
  selection.addRange(range)
}

function modifySelection(selection: Selection, alter: 'move' | 'extend', action: EditingAction): void {
  switch (action) {
    case 'left':
      selection.modify(alter, 'backward', 'character')
      return
    case 'right':
      selection.modify(alter, 'forward', 'character')
      return
    case 'up':
      selection.modify(alter, 'backward', 'line')
      return
    case 'down':
      selection.modify(alter, 'forward', 'line')
      return
    case 'lineStart':
    case 'docStart':
      selection.modify(alter, 'backward', action === 'docStart' ? 'documentboundary' : 'lineboundary')
      return
    case 'lineEnd':
    case 'docEnd':
      selection.modify(alter, 'forward', action === 'docEnd' ? 'documentboundary' : 'lineboundary')
      return
    case 'pageUp':
      selection.modify(alter, 'backward', 'line')
      return
    case 'pageDown':
      selection.modify(alter, 'forward', 'line')
      return
    default:
      return
  }
}

function readSelection(element: HTMLInputElement | HTMLTextAreaElement): CaretState {
  const start = element.selectionStart ?? 0
  const end = element.selectionEnd ?? start
  const direction = element.selectionDirection === 'backward' ? 'backward' : end === start ? 'none' : 'forward'
  return { start, end, direction }
}

function selectionFromDom(selection: Selection, textNode: Text): CaretState | null {
  if (selection.rangeCount === 0) return { start: textNode.data.length, end: textNode.data.length, direction: 'none' }
  const range = selection.getRangeAt(0)
  if (range.startContainer !== textNode || range.endContainer !== textNode) return null
  return {
    start: range.startOffset,
    end: range.endOffset,
    direction: 'forward'
  }
}

function linesInView(element: HTMLElement): number {
  const lineHeight = Number.parseFloat(getComputedStyle(element).lineHeight)
  return pageLineCount(element.clientHeight, lineHeight)
}
