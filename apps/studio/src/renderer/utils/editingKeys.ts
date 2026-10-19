/**
 * Caret movement for Home, End, Page Up, Page Down, and the private-use
 * characters WKWebView inserts for those keys.
 *
 * Apple's NSEvent function keys are U+F700–U+F7FF. On macOS, the web view
 * delivers them as `insertText` in addition to (or instead of) a navigation
 * key. A text field must move the caret and must not keep the character.
 * Cocoa maps Home/End to the current line, and Command/Ctrl plus Home/End
 * to the whole field. Shift extends the selection.
 */

export type EditingAction =
  | 'left'
  | 'right'
  | 'up'
  | 'down'
  | 'lineStart'
  | 'lineEnd'
  | 'docStart'
  | 'docEnd'
  | 'pageUp'
  | 'pageDown'
  | 'deleteForward'
  | 'ignore'

export interface EditingModifiers {
  /** Command on macOS or Control on other platforms. Either jumps to the field edge. */
  command: boolean
}

export interface CaretState {
  start: number
  end: number
  direction: 'forward' | 'backward' | 'none'
}

export interface CaretMove {
  selection: CaretState
  /** Column to keep for the next vertical move. Null after a horizontal move. */
  goalColumn: number | null
}

const NAMED_NAVIGATION = new Set(['Home', 'End', 'PageUp', 'PageDown'])

/** One Apple function-key character, or null when the text is ordinary. */
export function macFunctionKeyAction(text: string): EditingAction | null {
  if (text.length !== 1) return null
  const code = text.charCodeAt(0)
  if (code < 0xf700 || code > 0xf7ff) return null
  switch (code) {
    case 0xf700:
      return 'up'
    case 0xf701:
      return 'down'
    case 0xf702:
      return 'left'
    case 0xf703:
      return 'right'
    case 0xf728:
      return 'deleteForward'
    case 0xf729:
      return 'lineStart'
    case 0xf72b:
      return 'lineEnd'
    case 0xf72c:
      return 'pageUp'
    case 0xf72d:
      return 'pageDown'
    default:
      return 'ignore'
  }
}

export function containsMacFunctionKey(text: string): boolean {
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index)
    if (code >= 0xf700 && code <= 0xf7ff) return true
  }
  return false
}

export function stripMacFunctionKeys(text: string): string {
  let next = ''
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index)
    if (code < 0xf700 || code > 0xf7ff) next += text[index]
  }
  return next
}

/**
 * The editing action for a keydown. Named arrows stay with the browser.
 * The private-use arrow characters do not: WKWebView would insert them.
 */
export function editingActionForKey(key: string, modifiers: EditingModifiers): EditingAction | null {
  if (NAMED_NAVIGATION.has(key)) {
    const toEdge = modifiers.command
    if (key === 'Home') return toEdge ? 'docStart' : 'lineStart'
    if (key === 'End') return toEdge ? 'docEnd' : 'lineEnd'
    if (key === 'PageUp') return toEdge ? 'docStart' : 'pageUp'
    return toEdge ? 'docEnd' : 'pageDown'
  }

  const fromCharacter = macFunctionKeyAction(key)
  if (!fromCharacter) return null
  if (fromCharacter === 'lineStart') return modifiers.command ? 'docStart' : 'lineStart'
  if (fromCharacter === 'lineEnd') return modifiers.command ? 'docEnd' : 'lineEnd'
  if (fromCharacter === 'pageUp') return modifiers.command ? 'docStart' : 'pageUp'
  if (fromCharacter === 'pageDown') return modifiers.command ? 'docEnd' : 'pageDown'
  return fromCharacter
}

export function pageLineCount(height: number, lineHeight: number): number {
  if (!Number.isFinite(height) || !Number.isFinite(lineHeight) || lineHeight <= 0 || height <= 0) return 1
  return Math.max(1, Math.floor(height / lineHeight))
}

export function moveCaret(
  value: string,
  selection: CaretState,
  action: Exclude<EditingAction, 'ignore' | 'deleteForward'>,
  extend: boolean,
  pageLines: number,
  goalColumn: number | null,
  singleLine = false
): CaretMove {
  const { anchor, focus } = caretEnds(selection)
  const vertical = action === 'up' || action === 'down' || action === 'pageUp' || action === 'pageDown'
  const column = vertical ? (goalColumn ?? focus - lineStart(value, focus)) : null
  const destination = caretDestination(value, focus, action, pageLines, column ?? 0, singleLine)
  const selectionNext = extend ? caretRange(anchor, destination) : collapsed(destination)
  return { selection: selectionNext, goalColumn: vertical ? column : null }
}

/** Range removed by forward delete. Null when the caret is already at the end. */
export function forwardDeleteRange(value: string, selection: CaretState): { start: number; end: number } | null {
  if (selection.start !== selection.end) return { start: selection.start, end: selection.end }
  const next = nextUnit(value, selection.start)
  if (next === selection.start) return null
  return { start: selection.start, end: next }
}

function caretDestination(
  value: string,
  focus: number,
  action: Exclude<EditingAction, 'ignore' | 'deleteForward'>,
  pageLines: number,
  column: number,
  singleLine: boolean
): number {
  if (singleLine) {
    if (action === 'left') return previousUnit(value, focus)
    if (action === 'right') return nextUnit(value, focus)
    if (action === 'lineEnd' || action === 'docEnd' || action === 'down' || action === 'pageDown') return value.length
    return 0
  }

  switch (action) {
    case 'left':
      return previousUnit(value, focus)
    case 'right':
      return nextUnit(value, focus)
    case 'lineStart':
      return lineStart(value, focus)
    case 'lineEnd':
      return lineEnd(value, focus)
    case 'docStart':
      return 0
    case 'docEnd':
      return value.length
    case 'up':
    case 'pageUp': {
      const line = lineIndex(value, focus)
      const steps = action === 'up' ? 1 : Math.max(1, pageLines)
      if (line === 0 || line - steps < 0) return line === 0 ? 0 : indexAt(value, 0, column)
      return indexAt(value, line - steps, column)
    }
    case 'down':
    case 'pageDown': {
      const line = lineIndex(value, focus)
      const last = lineIndex(value, value.length)
      const steps = action === 'down' ? 1 : Math.max(1, pageLines)
      if (line >= last || line + steps > last) {
        if (line >= last) return value.length
      }
      if (line + steps >= last && action === 'pageDown') return value.length
      return indexAt(value, Math.min(last, line + steps), column)
    }
    default:
      return focus
  }
}

function caretEnds(selection: CaretState): { anchor: number; focus: number } {
  if (selection.start === selection.end || selection.direction !== 'backward') {
    return { anchor: selection.start, focus: selection.end }
  }
  return { anchor: selection.end, focus: selection.start }
}

function collapsed(index: number): CaretState {
  return { start: index, end: index, direction: 'none' }
}

function caretRange(anchor: number, focus: number): CaretState {
  if (anchor === focus) return collapsed(anchor)
  if (focus < anchor) return { start: focus, end: anchor, direction: 'backward' }
  return { start: anchor, end: focus, direction: 'forward' }
}

function lineStart(value: string, index: number): number {
  if (index <= 0) return 0
  const newline = value.lastIndexOf('\n', index - 1)
  return newline === -1 ? 0 : newline + 1
}

function lineEnd(value: string, index: number): number {
  const newline = value.indexOf('\n', index)
  return newline === -1 ? value.length : newline
}

function lineIndex(value: string, index: number): number {
  let line = 0
  let from = 0
  while (from < index) {
    const newline = value.indexOf('\n', from)
    if (newline === -1 || newline >= index) break
    line += 1
    from = newline + 1
  }
  return line
}

function indexAt(value: string, line: number, column: number): number {
  let current = 0
  let remaining = line
  while (remaining > 0) {
    const newline = value.indexOf('\n', current)
    if (newline === -1) return value.length
    current = newline + 1
    remaining -= 1
  }
  return Math.min(current + Math.max(0, column), lineEnd(value, current))
}

function nextUnit(value: string, index: number): number {
  if (index >= value.length) return value.length
  const code = value.charCodeAt(index)
  if (code >= 0xd800 && code <= 0xdbff && index + 1 < value.length) return index + 2
  return index + 1
}

function previousUnit(value: string, index: number): number {
  if (index <= 0) return 0
  if (index >= 2) {
    const lead = value.charCodeAt(index - 2)
    const trail = value.charCodeAt(index - 1)
    if (lead >= 0xd800 && lead <= 0xdbff && trail >= 0xdc00 && trail <= 0xdfff) return index - 2
  }
  return index - 1
}
