import { describe, expect, it } from 'vitest'

import {
  containsMacFunctionKey,
  editingActionForKey,
  forwardDeleteRange,
  macFunctionKeyAction,
  moveCaret,
  pageLineCount,
  stripMacFunctionKeys
} from '@renderer/utils/editingKeys'

const caret = (start: number, end = start) => ({
  start,
  end,
  direction: end === start ? ('none' as const) : ('forward' as const)
})

describe('mac function keys', () => {
  it('maps the private-use characters WKWebView inserts for navigation keys', () => {
    expect(macFunctionKeyAction('\uF729')).toBe('lineStart')
    expect(macFunctionKeyAction('\uF72B')).toBe('lineEnd')
    expect(macFunctionKeyAction('\uF72C')).toBe('pageUp')
    expect(macFunctionKeyAction('\uF72D')).toBe('pageDown')
    expect(macFunctionKeyAction('\uF728')).toBe('deleteForward')
    expect(macFunctionKeyAction('\uF700')).toBe('up')
    expect(macFunctionKeyAction('\uF727')).toBe('ignore')
    expect(macFunctionKeyAction('Home')).toBeNull()
    expect(containsMacFunctionKey('a\uF729b')).toBe(true)
    expect(stripMacFunctionKeys('a\uF729b')).toBe('ab')
  })

  it('keeps named arrows with the browser and sends Home to the line or the field', () => {
    expect(editingActionForKey('ArrowLeft', { command: false })).toBeNull()
    expect(editingActionForKey('Home', { command: false })).toBe('lineStart')
    expect(editingActionForKey('End', { command: true })).toBe('docEnd')
    expect(editingActionForKey('PageUp', { command: true })).toBe('docStart')
    expect(editingActionForKey('\uF729', { command: true })).toBe('docStart')
    expect(editingActionForKey('\uF72B', { command: false })).toBe('lineEnd')
  })
})

describe('moveCaret', () => {
  const text = 'hello\nworld'

  it('moves Home and End within the current line, and to the field with Command', () => {
    expect(moveCaret(text, caret(8), 'lineStart', false, 1, null).selection).toEqual(caret(6))
    expect(moveCaret(text, caret(1), 'lineEnd', false, 1, null).selection).toEqual(caret(5))
    expect(moveCaret(text, caret(8), 'docStart', false, 1, null).selection).toEqual(caret(0))
    expect(moveCaret(text, caret(1), 'docEnd', false, 1, null).selection).toEqual(caret(text.length))
  })

  it('extends the selection when Shift is held and keeps the vertical column', () => {
    const extended = moveCaret(text, caret(8), 'lineStart', true, 1, null)
    expect(extended.selection).toEqual({ start: 6, end: 8, direction: 'backward' })

    const down = moveCaret(text, caret(2), 'down', false, 1, null)
    expect(down.selection).toEqual(caret(8))
    expect(down.goalColumn).toBe(2)
    expect(moveCaret(text, down.selection, 'up', false, 1, down.goalColumn).selection).toEqual(caret(2))
  })

  it('pages by the visible line count and stops at the field edges', () => {
    expect(pageLineCount(0, 16)).toBe(1)
    expect(pageLineCount(48, 16)).toBe(3)
    const lines = 'a\nb\nc\nd\ne'
    expect(moveCaret(lines, caret(4), 'pageUp', false, 3, null).selection.start).toBe(0)
    expect(moveCaret(lines, caret(0), 'pageDown', false, 10, null).selection.start).toBe(lines.length)
  })

  it('deletes the next code unit, including a surrogate pair, and leaves the end alone', () => {
    expect(forwardDeleteRange('ab', caret(0))).toEqual({ start: 0, end: 1 })
    expect(forwardDeleteRange('a😀b', caret(1))).toEqual({ start: 1, end: 3 })
    expect(forwardDeleteRange('ab', caret(2))).toBeNull()
    expect(forwardDeleteRange('ab', { start: 0, end: 2, direction: 'forward' })).toEqual({ start: 0, end: 2 })
    expect(moveCaret('a😀b', caret(3), 'left', false, 1, null).selection).toEqual(caret(1))
  })
})
