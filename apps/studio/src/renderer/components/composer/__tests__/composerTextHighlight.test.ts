import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { afterEach, describe, expect, it } from 'vitest'

import { createComposerTextHighlightExtension } from '../composerTextHighlight'

let editor: Editor | undefined

afterEach(() => {
  editor?.destroy()
  editor = undefined
})

function highlighted(kind: string) {
  return [...(editor?.view.dom.querySelectorAll(`[data-highlight="${kind}"]`) ?? [])].map((node) => node.textContent)
}

describe('createComposerTextHighlightExtension', () => {
  it('decorates every match and follows edits without changing the text', () => {
    editor = new Editor({
      element: document.createElement('div'),
      extensions: [
        StarterKit,
        createComposerTextHighlightExtension([
          { pattern: /\/goal(?=\s|$)/g, className: 'cmd', attributes: { 'data-highlight': 'command' } }
        ])
      ],
      content: '<p>/goal ship it</p>'
    })

    expect(highlighted('command')).toEqual(['/goal'])
    expect(editor.view.dom.querySelector('.cmd')).not.toBeNull()

    editor.commands.setContent('<p>no command here</p>')
    expect(highlighted('command')).toEqual([])
    expect(editor.getText()).toBe('no command here')
  })
})
