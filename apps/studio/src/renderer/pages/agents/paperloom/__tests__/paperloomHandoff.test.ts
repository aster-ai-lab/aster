import { describe, expect, it } from 'vitest'

import { openPaperloom, PAPERLOOM_OPEN } from '../paperloomApi'
import { paperloomDraftQuestion, samePaperloomDraft } from '../paperloomHandoff'

describe('paper workspace handoff', () => {
  it('uses the installed package identity and removes only the exact stored token text', () => {
    const token = {
      id: 'agent-mention:a3s-paperloom:unique',
      kind: 'reference' as const,
      index: 0,
      textOffset: 0,
      label: '使用智能体 论文精读：',
      promptText: '使用智能体 论文精读：\nInstalled instructions'
    }
    expect(paperloomDraftQuestion({ text: `${token.promptText} Explain methods`, tokens: [token] })).toBe(
      'Explain methods'
    )
    expect(paperloomDraftQuestion({ text: token.promptText, tokens: [token] })).toBe('')
    expect(paperloomDraftQuestion({ text: '使用智能体 论文精读：', tokens: [] })).toBeNull()
    expect(
      paperloomDraftQuestion({
        text: 'Other',
        tokens: [{ ...token, id: 'agent-mention:local-agent:unique' }]
      })
    ).toBeNull()
  })

  it('removes resource spans without removing the same words in the question', () => {
    const path = '/work/paper.pdf'
    const phrase = '使用智能体 论文精读：'
    const question = `解释 ${path}，保留「${phrase}」这句话`
    expect(
      paperloomDraftQuestion({
        text: `${path} ${phrase} ${question}`,
        tokens: [
          { id: 'folder-token:one', kind: 'folder', label: 'paper.pdf', promptText: path, index: 0, textOffset: 0 },
          {
            id: 'agent-mention:a3s-paperloom:one',
            kind: 'reference',
            label: phrase,
            promptText: phrase,
            index: 1,
            textOffset: path.length + 1
          }
        ]
      })
    ).toBe(question)
  })

  it('only acknowledges a handoff accepted by a mounted host', () => {
    expect(openPaperloom('question', { root: '/work', paths: [] })).toBe(false)
    const receive = (event: Event) => event.preventDefault()
    window.addEventListener(PAPERLOOM_OPEN, receive)
    try {
      expect(openPaperloom('question', { root: '/work', paths: [] })).toBe(true)
    } finally {
      window.removeEventListener(PAPERLOOM_OPEN, receive)
    }
  })

  it('does not consume a new draft after an older source finishes opening', () => {
    const token = {
      id: 'agent-mention:a3s-paperloom:one',
      kind: 'reference' as const,
      label: '论文精读',
      index: 0,
      textOffset: 0
    }
    const sent = { text: 'Question', tokens: [token] }
    expect(samePaperloomDraft({ ...sent }, sent)).toBe(true)
    expect(samePaperloomDraft({ ...sent, text: 'New question' }, sent)).toBe(false)
    expect(samePaperloomDraft({ ...sent, tokens: [{ ...token, id: 'other-token' }] }, sent)).toBe(false)
  })
})
