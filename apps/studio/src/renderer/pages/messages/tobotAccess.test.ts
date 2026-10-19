import { afterEach, describe, expect, it } from 'vitest'

import { loadToBotPhone, loadToBotWallet, saveToBotPhone, saveToBotWallet, tobotAccessNote } from './tobotAccess'

afterEach(() => {
  localStorage.removeItem('messages.tobot.phone.v1')
  localStorage.removeItem('messages.tobot.wallet.v1')
})

describe('tobotAccess', () => {
  it('denies the phone and the wallet until the member allows them', () => {
    expect(tobotAccessNote('13800138000')).toBe(
      'ToBot is not allowed to use the member phone. Do not invent a phone number, place a call, or send an SMS. ToBot is not allowed to manage the wallet. Do not invent a balance, pay, transfer, or change money.'
    )
    saveToBotPhone({ share: true })
    saveToBotWallet({ view: true, cap: 200 })
    expect(tobotAccessNote('13800138000')).toContain('The member phone is 13800138000')
    expect(tobotAccessNote('13800138000')).toContain('above 200 yuan')
    expect(tobotAccessNote('not-a-phone')).toContain('no phone on file')
    expect(loadToBotWallet()).toEqual({ view: true, cap: 200 })
    saveToBotWallet({ view: true, cap: 0 })
    expect(loadToBotWallet().cap).toBeNull()
    expect(loadToBotPhone()).toEqual({ share: true })
  })
})
