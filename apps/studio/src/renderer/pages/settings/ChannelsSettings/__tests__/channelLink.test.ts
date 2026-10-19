import { describe, expect, it } from 'vitest'

import { channelPrompt, channelServesToBot, chatAllowed, replyAfterPrompt } from '../channelLink'

const prompt = channelPrompt('telegram', '42', 'hello')

describe('channelLink', () => {
  it('treats an empty agent as ToBot and filters chats only when the list has ids', () => {
    expect(channelServesToBot(null)).toBe(true)
    expect(channelServesToBot('')).toBe(true)
    expect(channelServesToBot('tobot')).toBe(true)
    expect(channelServesToBot('agent-1')).toBe(false)
    expect(chatAllowed([], '42')).toBe(true)
    expect(chatAllowed(undefined, '42')).toBe(true)
    expect(chatAllowed(['7'], '42')).toBe(false)
    expect(chatAllowed(['42'], '42')).toBe(true)
    expect(prompt).toBe('[telegram 42] hello')
  })

  it('reads the assistant line after the matching user text on a newest-first page', () => {
    const page = [
      {
        role: 'assistant',
        status: 'success',
        searchableText: 'reply',
        data: { parts: [{ type: 'text', text: 'reply' }] }
      },
      { role: 'user', status: 'success', searchableText: prompt }
    ]
    expect(replyAfterPrompt(page, prompt)).toBe('reply')
    expect(
      replyAfterPrompt(
        [
          { role: 'assistant', status: 'pending', searchableText: '' },
          { role: 'user', searchableText: prompt }
        ],
        prompt
      )
    ).toBeUndefined()
    expect(
      replyAfterPrompt(
        [
          { role: 'assistant', status: 'error', searchableText: '' },
          { role: 'user', searchableText: prompt }
        ],
        prompt
      )
    ).toBeNull()
    expect(
      replyAfterPrompt(
        [
          { role: 'user', searchableText: 'newer' },
          { role: 'user', searchableText: prompt }
        ],
        prompt
      )
    ).toBeNull()
    expect(replyAfterPrompt([], prompt)).toBeUndefined()
  })
})
