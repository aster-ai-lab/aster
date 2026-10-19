import { describe, expect, it } from 'vitest'

import type { ComposerDraftToken, ComposerSerializedToken } from '../../tokens'
import { retainPromptFileTokens } from '../shared/composerTokens'

const managed: ComposerDraftToken = { id: 'file:kept', kind: 'file', label: 'notes.txt' }

describe('retainPromptFileTokens', () => {
  it('keeps a file chip whose sentence is not already in the file list', () => {
    const hidden: ComposerSerializedToken = {
      id: 'file:review',
      kind: 'file',
      label: '合同.docx',
      promptText: '/tmp/合同.docx',
      index: 0,
      textOffset: 12
    }

    expect(retainPromptFileTokens([managed], [hidden]).map((token) => token.id)).toEqual(['file:kept', 'file:review'])
  })

  it('does not duplicate a file chip the file list already owns', () => {
    const hidden: ComposerSerializedToken = {
      id: 'file:kept',
      kind: 'file',
      label: 'notes.txt',
      promptText: '/tmp/notes.txt',
      index: 0,
      textOffset: 0
    }

    expect(retainPromptFileTokens([managed], [hidden])).toEqual([managed])
  })
})
