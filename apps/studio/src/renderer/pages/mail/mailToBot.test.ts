import { afterEach, describe, expect, it } from 'vitest'

import { loadMailToBotPolicy, mailToBotNote, normalizeMailToBotPolicy, saveMailToBotPolicy } from './mailToBot'

afterEach(() => {
  localStorage.removeItem('mail.tobot.v1')
})

describe('mailToBot', () => {
  it('stays closed until the member allows it, and drops unknown folders', () => {
    expect(normalizeMailToBotPolicy(null)).toEqual({
      allow: false,
      list: true,
      read: false,
      folders: ['inbox', 'unread', 'flagged', 'sent']
    })
    expect(normalizeMailToBotPolicy({ allow: true, folders: ['nope', 'inbox', 'inbox'] })).toEqual({
      allow: true,
      list: true,
      read: false,
      folders: ['inbox']
    })
    const denied = mailToBotNote(normalizeMailToBotPolicy({ allow: true, list: false }), 'roy@mail.towork', 3, 'hello')
    expect(denied).toBe('ToBot is not allowed to manage this mailbox. Do not use member-mail tools or invent messages.')
  })

  it('remembers the grant', () => {
    saveMailToBotPolicy({ allow: true, list: true, read: true, folders: ['sent', 'sent', 'drafts'] })
    expect(loadMailToBotPolicy()).toEqual({
      allow: true,
      list: true,
      read: true,
      folders: ['sent', 'drafts']
    })
  })
})
