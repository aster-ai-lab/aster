export interface TerminalScreen {
  rows: string[]
  row: number
  col: number
  cols: number
}

const MAX_ROWS = 500

export function createTerminalScreen(cols = 80): TerminalScreen {
  return { rows: [''], row: 0, col: 0, cols }
}

export function renderTerminalScreen(screen: TerminalScreen): string {
  return screen.rows.join('\n')
}

/** Paint one chunk of shell output, including the cursor motions a prompt uses. */
export function applyTerminalChunk(screen: TerminalScreen, chunk: string): TerminalScreen {
  const next: TerminalScreen = {
    rows: screen.rows.slice(),
    row: screen.row,
    col: screen.col,
    cols: screen.cols
  }
  const chars = Array.from(chunk)
  let index = 0
  while (index < chars.length) {
    const ch = chars[index]
    if (ch === '\u001b') {
      index = consumeEscape(next, chars, index)
      continue
    }
    if (ch === '\r') {
      next.col = 0
      index += 1
      continue
    }
    if (ch === '\n') {
      next.row += 1
      next.col = 0
      ensureRow(next)
      index += 1
      continue
    }
    if (ch === '\b' || ch === '\u007f') {
      next.col = Math.max(0, next.col - 1)
      index += 1
      continue
    }
    if (ch === '\t') {
      const width = 8 - (next.col % 8)
      next.col = Math.min(next.cols - 1, next.col + width)
      index += 1
      continue
    }
    if (ch < ' ') {
      index += 1
      continue
    }
    putChar(next, ch)
    index += 1
  }
  if (next.rows.length > MAX_ROWS) {
    const dropped = next.rows.length - MAX_ROWS
    next.rows.splice(0, dropped)
    next.row = Math.max(0, next.row - dropped)
  }
  return next
}

function ensureRow(screen: TerminalScreen) {
  while (screen.rows.length <= screen.row) screen.rows.push('')
}

function putChar(screen: TerminalScreen, ch: string) {
  if (screen.col >= screen.cols) {
    screen.row += 1
    screen.col = 0
  }
  ensureRow(screen)
  const chars = Array.from(screen.rows[screen.row] ?? '')
  while (chars.length < screen.col) chars.push(' ')
  chars[screen.col] = ch
  screen.rows[screen.row] = chars.join('')
  screen.col += 1
}

function consumeEscape(screen: TerminalScreen, chars: string[], index: number): number {
  const next = chars[index + 1]
  if (next === '[') return consumeCsi(screen, chars, index + 2)
  if (next === ']') return consumeOsc(chars, index + 2)
  return index + (next ? 2 : 1)
}

function consumeOsc(chars: string[], index: number): number {
  let cursor = index
  while (cursor < chars.length) {
    if (chars[cursor] === '\u0007') return cursor + 1
    if (chars[cursor] === '\u001b' && chars[cursor + 1] === '\\') return cursor + 2
    cursor += 1
  }
  return chars.length
}

function consumeCsi(screen: TerminalScreen, chars: string[], index: number): number {
  let cursor = index
  while (cursor < chars.length && (chars[cursor] < '@' || chars[cursor] > '~')) cursor += 1
  if (cursor >= chars.length) return chars.length
  const command = chars[cursor]
  const body = chars.slice(index, cursor).join('')
  const privateMode = body.startsWith('?')
  const params = (privateMode ? body.slice(1) : body).split(';').map((part) => Number(part) || 0)
  if (!privateMode) applyCsi(screen, command, params)
  return cursor + 1
}

function applyCsi(screen: TerminalScreen, command: string, params: number[]) {
  const primary = params[0] ?? 0
  if (command === 'A') screen.row = Math.max(0, screen.row - (primary || 1))
  if (command === 'B') {
    screen.row += primary || 1
    ensureRow(screen)
  }
  if (command === 'C') screen.col = Math.min(screen.cols - 1, screen.col + (primary || 1))
  if (command === 'D') screen.col = Math.max(0, screen.col - (primary || 1))
  if (command === 'G') screen.col = Math.max(0, Math.min(screen.cols - 1, (primary || 1) - 1))
  if (command === 'H' || command === 'f') {
    screen.row = Math.max(0, (params[0] || 1) - 1)
    screen.col = Math.max(0, Math.min(screen.cols - 1, (params[1] || 1) - 1))
    ensureRow(screen)
  }
  if (command === 'K') eraseLine(screen, primary)
  if (command === 'J' && primary === 2) {
    screen.rows = ['']
    screen.row = 0
    screen.col = 0
  }
  if (command === 'P') {
    const count = primary || 1
    const chars = Array.from(screen.rows[screen.row] ?? '')
    chars.splice(screen.col, count)
    screen.rows[screen.row] = chars.join('')
  }
}

function eraseLine(screen: TerminalScreen, mode: number) {
  ensureRow(screen)
  const chars = Array.from(screen.rows[screen.row] ?? '')
  if (mode === 1) {
    for (let index = 0; index < screen.col && index < chars.length; index += 1) chars[index] = ' '
  } else if (mode === 2) {
    screen.rows[screen.row] = ''
    return
  } else {
    chars.splice(screen.col)
  }
  screen.rows[screen.row] = chars.join('').trimEnd()
}
