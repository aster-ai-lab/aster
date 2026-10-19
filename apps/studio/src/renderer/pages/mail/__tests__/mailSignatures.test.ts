import { beforeEach, describe, expect, it } from 'vitest'

import { editorSeed, quoteMailBody } from '../mailDocument'
import {
  defaultSignature,
  loadSignatures,
  openingBody,
  placeSignature,
  saveSignatures,
  SIGNATURE_KEY,
  type MailSignature
} from '../mailSignatures'

const one = (id: string, name: string, text: string, isDefault = false): MailSignature => ({
  id,
  name,
  text,
  isDefault
})

describe('mail signatures', () => {
  beforeEach(() => {
    localStorage.removeItem(SIGNATURE_KEY)
  })

  it('stores several signatures and keeps a single default', () => {
    saveSignatures([one('a', '工作', 'Roy\nA3S', true), one('b', '个人', 'Roy')])
    const stored = loadSignatures()
    expect(stored.map((item) => item.name)).toEqual(['工作', '个人'])
    expect(defaultSignature()?.id).toBe('a')
    saveSignatures(stored.map((item) => ({ ...item, isDefault: item.id === 'b' })))
    expect(defaultSignature()?.name).toBe('个人')
    expect(loadSignatures().filter((item) => item.isDefault)).toHaveLength(1)
  })

  it('puts the default signature before a reply quote and leaves a draft alone', () => {
    const quote = quoteMailBody('Ada', 'hello')
    const reply = openingBody('reply', quote, 'Roy')
    expect(reply.indexOf('Roy')).toBeLessThan(reply.indexOf('<blockquote'))
    expect(reply).toContain('hello')
    expect(openingBody('new', '', 'Roy')).toContain('Roy')
    expect(openingBody('draft', quote, 'Roy')).toBe(quote)
    expect(openingBody('new', '', '')).toBe('')

    const fresh = openingBody('new', '<p>林书豪</p>', 'Roy')
    expect(fresh.indexOf('林书豪')).toBeLessThan(fresh.indexOf('Roy'))
    expect(fresh).toContain('<mail-signature>')
    const seeded = editorSeed(fresh)
    expect(seeded.indexOf('林书豪')).toBeLessThan(seeded.indexOf('Roy'))
    expect(seeded).toContain('<mail-signature>')
    const replaced = placeSignature(fresh, 'Lin')
    expect(replaced).toContain('林书豪')
    expect(replaced).toContain('Lin')
    expect(replaced).not.toContain('Roy')

    const quoted = '<p>林书豪</p><blockquote><mail-signature><p>Old</p></mail-signature></blockquote>'
    const placed = placeSignature(quoted, 'Roy')
    expect(placed).toContain('Old')
    expect(placed).toContain('林书豪')
    expect(placed.indexOf('Roy')).toBeLessThan(placed.indexOf('<blockquote'))
  })
})
