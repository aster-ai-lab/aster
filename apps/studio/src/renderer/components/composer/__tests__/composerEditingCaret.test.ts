import { Editor } from '@tiptap/core'
import { StarterKit } from '@tiptap/starter-kit'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  applyComposerEditingKey,
  applyComposerFunctionKeyText
} from '@renderer/components/composer/composerEditingCaret'
import {
  getComposerInputText,
  getComposerPositionAtTextOffset,
  getComposerTextOffset
} from '@renderer/components/composer/quickPanel/bridge'

let editor: Editor | undefined

function mount(): Editor {
  editor?.destroy()
  editor = new Editor({
    element: document.createElement('div'),
    extensions: [StarterKit],
    content: {
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          content: [{ type: 'text', text: 'hello' }, { type: 'hardBreak' }, { type: 'text', text: 'world' }]
        }
      ]
    }
  })
  return editor
}

function place(instance: Editor, offset: number): void {
  instance.commands.setTextSelection(getComposerPositionAtTextOffset(instance, offset))
}

function headOffset(instance: Editor): number {
  return getComposerTextOffset(instance, instance.state.selection.head)
}

function key(name: string, init: Partial<KeyboardEventInit> = {}) {
  return {
    key: name,
    metaKey: false,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    isComposing: false,
    defaultPrevented: false,
    preventDefault: vi.fn(),
    ...init
  }
}

describe('composer editing caret', () => {
  afterEach(() => {
    editor?.destroy()
    editor = undefined
  })

  it('moves Home and End by line and Command by the whole field', () => {
    const instance = mount()
    expect(getComposerInputText(instance)).toBe('hello\nworld')
    place(instance, 8)

    const home = key('Home')
    expect(applyComposerEditingKey(instance, home, 40, 20)).toBe(true)
    expect(home.preventDefault).toHaveBeenCalledOnce()
    expect(headOffset(instance)).toBe(6)

    applyComposerEditingKey(instance, key('End', { ctrlKey: true }), 40, 20)
    expect(headOffset(instance)).toBe(11)

    applyComposerEditingKey(instance, key('\uF729', { metaKey: true }), 40, 20)
    expect(headOffset(instance)).toBe(0)
    expect(getComposerInputText(instance)).toBe('hello\nworld')
  })

  it('refuses a private-use insert and moves only when keydown did not', () => {
    const instance = mount()
    place(instance, 8)

    expect(applyComposerFunctionKeyText(instance, '\uF72B', 40, 20)).toBe(true)
    expect(headOffset(instance)).toBe(11)
    expect(getComposerInputText(instance)).toBe('hello\nworld')
    expect(applyComposerFunctionKeyText(instance, 'hello', 40, 20)).toBe(false)
  })

  it('does not steal a key the quick panel or IME already claimed', () => {
    const instance = mount()
    place(instance, 8)
    const claimed = key('Home', { defaultPrevented: true })
    expect(applyComposerEditingKey(instance, claimed, 40, 20)).toBe(false)
    expect(headOffset(instance)).toBe(8)

    const composing = key('End', { isComposing: true })
    expect(applyComposerEditingKey(instance, composing, 40, 20)).toBe(false)
    expect(headOffset(instance)).toBe(8)
  })
})
