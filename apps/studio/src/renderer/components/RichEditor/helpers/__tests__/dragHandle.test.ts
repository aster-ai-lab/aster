import { describe, expect, it, vi } from 'vitest'

import { hideDragHandleOnScroll } from '../dragHandle'

describe('drag handle scroll behavior', () => {
  it('hides and clears the handle through Tiptap metadata while the editor has focus', () => {
    const editor = { view: { hasFocus: () => true }, commands: { setMeta: vi.fn() } }

    hideDragHandleOnScroll(editor as never)

    expect(editor.commands.setMeta).toHaveBeenCalledWith('hideDragHandle', true)
  })

  it('leaves the handle unchanged when another surface owns focus', () => {
    const editor = { view: { hasFocus: () => false }, commands: { setMeta: vi.fn() } }

    hideDragHandleOnScroll(editor as never)

    expect(editor.commands.setMeta).not.toHaveBeenCalled()
  })
})
