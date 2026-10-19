import { describe, expect, it } from 'vitest'

import {
  clearCachedPortraits,
  readCachedPortraits,
  readCachedSelfPortrait,
  writeCachedPortraits
} from '../messagePortraitCache'

describe('message portrait cache', () => {
  it('keeps the last portraits for the next time the message page opens', () => {
    clearCachedPortraits()
    writeCachedPortraits(
      { 'member-1': { name: '林知夏', phone: '', avatar: 'data:image/webp;base64,AA' } },
      { name: '我', phone: '13800138000', avatar: 'file:///me.webp' }
    )
    expect(readCachedPortraits()['member-1']?.avatar).toBe('data:image/webp;base64,AA')
    expect(readCachedSelfPortrait().avatar).toBe('file:///me.webp')
    clearCachedPortraits()
    expect(readCachedPortraits()).toEqual({})
    expect(readCachedSelfPortrait()).toEqual({ name: '', phone: '', avatar: '' })
  })
})
