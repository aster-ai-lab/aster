import { describe, expect, it } from 'vitest'

import { composeFailure, EMPTY_COMPOSE, listedRecipients, recipientTone } from '../mailCompose'

describe('recipient tone', () => {
  it('treats another domain as outside this mailbox', () => {
    expect(recipientTone('18770221825@163.com')).toBe('outside')
    expect(recipientTone('person@example.com')).toBe('outside')
    expect(recipientTone('ada@mail.towork')).toBe('local')
    expect(recipientTone('nope')).toBe('bad')
    expect(recipientTone('a@mail.towork')).toBe('bad')
    expect(recipientTone('foo@bar')).toBe('bad')
  })

  it('keeps an outside address out of the send list', () => {
    const listed = listedRecipients({
      ...EMPTY_COMPOSE,
      to: [{ value: '18770221825@163.com', valid: false }],
      cc: [{ value: 'ada@mail.towork', valid: true }]
    })
    expect(listed.to).toEqual([])
    expect(listed.cc).toEqual(['ada@mail.towork'])
    expect(listed.outside).toBe(true)
    expect(listed.bad).toBe(false)
    expect(listed.invalid).toBe(true)
    expect(listed.count).toBe(2)
  })

  it('maps the API domain rejection to the outside explanation', () => {
    expect(composeFailure('recipient must be an @mail.towork address', (key) => key)).toBe('mail.compose.outside')
    expect(composeFailure('a recipient is required', (key) => key)).toBe('mail.compose.invalid')
  })
})
