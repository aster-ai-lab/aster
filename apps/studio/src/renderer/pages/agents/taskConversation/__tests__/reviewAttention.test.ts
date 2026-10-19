import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { expect, it } from 'vitest'

import { ReviewAttentionDecorations, setReviewAttention } from '../reviewAttention'

it('paints one unique host quote without adding comments, moving the caret or changing the document', () => {
  const editor = new Editor({
    extensions: [StarterKit, ReviewAttentionDecorations],
    content: '<p>甲方应按时付款。</p>'
  })
  try {
    const source = editor.getJSON()
    const selection = editor.state.selection.from
    setReviewAttention(editor, '按时付款')
    expect(editor.view.dom.querySelector('[data-review-attention]')?.textContent).toBe('按时付款')
    expect(editor.getJSON()).toEqual(source)
    expect(editor.state.selection.from).toBe(selection)
    expect(editor.view.dom.querySelector('[data-comment-id]')).toBeNull()
    setReviewAttention(editor, '不存在的原文')
    expect(editor.view.dom.querySelector('[data-review-attention]')).toBeNull()
  } finally {
    editor.destroy()
  }
})

it('refuses an ambiguous quote and clears attention after the run', () => {
  const editor = new Editor({ extensions: [StarterKit, ReviewAttentionDecorations], content: '<p>付款，付款。</p>' })
  try {
    setReviewAttention(editor, '付款')
    expect(editor.view.dom.querySelector('[data-review-attention]')).toBeNull()
    setReviewAttention(editor, '付款。')
    expect(editor.view.dom.querySelector('[data-review-attention]')).not.toBeNull()
    setReviewAttention(editor, '')
    expect(editor.view.dom.querySelector('[data-review-attention]')).toBeNull()
  } finally {
    editor.destroy()
  }
})
