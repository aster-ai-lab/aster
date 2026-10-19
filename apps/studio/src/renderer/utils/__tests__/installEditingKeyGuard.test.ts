import { afterEach, describe, expect, it } from 'vitest'

import { installEditingKeyGuard } from '@renderer/utils/installEditingKeyGuard'

function press(target: HTMLElement, key: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init })
  target.dispatchEvent(event)
  return event
}

describe('installEditingKeyGuard', () => {
  const uninstalls: Array<() => void> = []

  afterEach(() => {
    for (const uninstall of uninstalls) uninstall()
    uninstalls.length = 0
    document.body.innerHTML = ''
  })

  it('moves a textarea caret for Home and End and does not insert the private-use character', () => {
    const textarea = document.createElement('textarea')
    textarea.value = 'hello\nworld'
    document.body.appendChild(textarea)
    textarea.setSelectionRange(8, 8)
    uninstalls.push(installEditingKeyGuard())

    const home = press(textarea, 'Home')
    expect(home.defaultPrevented).toBe(true)
    expect(textarea.selectionStart).toBe(6)
    expect(textarea.selectionEnd).toBe(6)

    press(textarea, 'End', { shiftKey: true })
    expect(textarea.selectionStart).toBe(6)
    expect(textarea.selectionEnd).toBe(11)

    press(textarea, 'Home', { metaKey: true })
    expect(textarea.selectionStart).toBe(0)

    const before = new InputEvent('beforeinput', {
      data: '\uF729',
      bubbles: true,
      cancelable: true
    })
    textarea.dispatchEvent(before)
    expect(before.defaultPrevented).toBe(true)
    expect(textarea.value).toBe('hello\nworld')
  })

  it('moves from a private-use insert when keydown did not identify the key', () => {
    const textarea = document.createElement('textarea')
    textarea.value = 'hello\nworld'
    document.body.appendChild(textarea)
    textarea.setSelectionRange(8, 8)
    uninstalls.push(installEditingKeyGuard())

    const before = new InputEvent('beforeinput', {
      data: '\uF72B',
      bubbles: true,
      cancelable: true
    })
    textarea.dispatchEvent(before)

    expect(before.defaultPrevented).toBe(true)
    expect(textarea.value).toBe('hello\nworld')
    expect(textarea.selectionStart).toBe(11)
  })

  it('deletes forward for the private-use delete key and swallows other private-use characters', () => {
    const textarea = document.createElement('textarea')
    textarea.value = 'ab'
    document.body.appendChild(textarea)
    textarea.setSelectionRange(0, 0)
    uninstalls.push(installEditingKeyGuard())

    press(textarea, '\uF728')
    expect(textarea.value).toBe('b')
    expect(textarea.selectionStart).toBe(0)

    const ignored = press(textarea, '\uF727')
    expect(ignored.defaultPrevented).toBe(true)
    expect(textarea.value).toBe('b')
  })

  it('leaves named arrows, composition, and non-text inputs alone', () => {
    const textarea = document.createElement('textarea')
    textarea.value = 'ab'
    document.body.appendChild(textarea)
    textarea.setSelectionRange(1, 1)
    const box = document.createElement('input')
    box.type = 'checkbox'
    document.body.appendChild(box)
    uninstalls.push(installEditingKeyGuard())

    const arrow = press(textarea, 'ArrowLeft')
    expect(arrow.defaultPrevented).toBe(false)
    expect(textarea.selectionStart).toBe(1)

    press(textarea, 'Home', { isComposing: true })
    expect(textarea.selectionStart).toBe(1)

    const check = press(box, 'Home')
    expect(check.defaultPrevented).toBe(false)
  })

  it('moves a plain contenteditable caret when the view has no Selection.modify', () => {
    const field = document.createElement('div')
    field.setAttribute('contenteditable', 'true')
    const text = document.createTextNode('hello')
    field.appendChild(text)
    document.body.appendChild(field)
    const selection = document.getSelection()
    expect(selection).not.toBeNull()
    const range = document.createRange()
    range.setStart(text, 5)
    range.collapse(true)
    selection?.removeAllRanges()
    selection?.addRange(range)
    uninstalls.push(installEditingKeyGuard())

    press(field, 'Home')
    expect(selection?.getRangeAt(0).startOffset).toBe(0)
  })
})
