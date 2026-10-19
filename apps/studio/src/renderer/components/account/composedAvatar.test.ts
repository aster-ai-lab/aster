import { describe, expect, it } from 'vitest'

import {
  AVATAR_ACCESSORIES,
  AVATAR_BACKGROUNDS,
  AVATAR_EYE_STYLES,
  AVATAR_HAIR_COLORS,
  AVATAR_HAIR_STYLES,
  AVATAR_MOUTH_STYLES,
  AVATAR_SKINS,
  avatarDataUrl,
  avatarSpecFromName,
  parseAvatarSpec,
  randomAvatarSpec,
  type AvatarSpec
} from './composedAvatar'

const sample: AvatarSpec = { background: 2, skin: 4, hair: 5, hairColor: 6, eyes: 3, mouth: 2, accessory: 1 }

describe('avatar composer', () => {
  it('stores every selected part inside an image the profile API accepts', () => {
    const url = avatarDataUrl(sample)
    expect(url.startsWith('data:image/svg+xml;base64,')).toBe(true)
    expect(url.length).toBeLessThan(400_000)
    expect(parseAvatarSpec(url)).toEqual(sample)
  })

  it('rejects an uploaded photo and an out-of-range part list', () => {
    expect(parseAvatarSpec('data:image/webp;base64,QQ==')).toBeNull()
    const xml = `<svg xmlns="http://www.w3.org/2000/svg" data-a3s-avatar="0.0.${AVATAR_HAIR_STYLES.length}.0.0.0.0"></svg>`
    expect(parseAvatarSpec(`data:image/svg+xml;base64,${btoa(xml)}`)).toBeNull()
  })

  it('appends parts after the original indexes so a saved avatar still parses', () => {
    expect(AVATAR_HAIR_STYLES.slice(0, 6)).toEqual(['short', 'bob', 'bun', 'fringe', 'wave', 'none'])
    expect(AVATAR_EYE_STYLES.slice(0, 5)).toEqual(['dot', 'round', 'line', 'wink', 'happy'])
    expect(AVATAR_MOUTH_STYLES.slice(0, 4)).toEqual(['smile', 'flat', 'grin', 'open'])
    expect(AVATAR_ACCESSORIES.slice(0, 4)).toEqual(['none', 'glasses', 'blush', 'earring'])
    const spec: AvatarSpec = {
      ...sample,
      hair: AVATAR_HAIR_STYLES.indexOf('tied-back'),
      eyes: AVATAR_EYE_STYLES.indexOf('wide'),
      mouth: AVATAR_MOUTH_STYLES.indexOf('soft'),
      accessory: AVATAR_ACCESSORIES.indexOf('headphones')
    }
    const url = avatarDataUrl(spec)
    expect(parseAvatarSpec(url)).toEqual(spec)
    expect(url).not.toBe(avatarDataUrl(sample))
  })

  it('keeps a name on the same starting avatar and stays inside each part list', () => {
    expect(avatarSpecFromName('林')).toEqual(avatarSpecFromName('林'))
    const spec = randomAvatarSpec(() => 0.999)
    expect(spec.background).toBe(AVATAR_BACKGROUNDS.length - 1)
    expect(spec.skin).toBe(AVATAR_SKINS.length - 1)
    expect(spec.hair).toBe(AVATAR_HAIR_STYLES.length - 1)
    expect(spec.hairColor).toBe(AVATAR_HAIR_COLORS.length - 1)
    expect(spec.eyes).toBe(AVATAR_EYE_STYLES.length - 1)
    expect(spec.mouth).toBe(AVATAR_MOUTH_STYLES.length - 1)
    expect(spec.accessory).toBe(AVATAR_ACCESSORIES.length - 1)
  })
})
