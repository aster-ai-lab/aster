import { describe, expect, it } from 'vitest'

import { applyTerminalChunk, createTerminalScreen, renderTerminalScreen } from '../terminalScreen'

describe('terminalScreen', () => {
  it('prints text and starts a new line', () => {
    const screen = applyTerminalChunk(createTerminalScreen(), 'hello\r\nworld')

    expect(renderTerminalScreen(screen)).toBe('hello\nworld')
    expect(screen.row).toBe(1)
    expect(screen.col).toBe(5)
  })

  it('redraws a line with carriage return and ignores color', () => {
    const screen = applyTerminalChunk(createTerminalScreen(), 'wait\r\u001b[32mready\u001b[0m')

    expect(renderTerminalScreen(screen)).toBe('ready')
  })

  it('erases to the end of the line', () => {
    const screen = applyTerminalChunk(createTerminalScreen(), 'abcdef\r\u001b[3G\u001b[K')

    expect(renderTerminalScreen(screen)).toBe('ab')
  })
})
