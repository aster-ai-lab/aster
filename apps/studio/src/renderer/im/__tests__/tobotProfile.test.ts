import { beforeEach, describe, expect, it } from 'vitest'

import { tobotFaceSignature, tobotShapePath } from '../../pages/messages/tobotAvatar'
import {
  TOBOT_EXPRESSIONS,
  TOBOT_PROFILE_KEY,
  TOBOT_SHAPES,
  defaultToBotProfile,
  normalizeProfile,
  readToBotProfile,
  tobotDisplayName,
  tobotHostContext,
  writeToBotProfile
} from '../tobotProfile'

describe('tobotProfile', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('keeps one snapshot until the profile is written', () => {
    const first = readToBotProfile()
    expect(readToBotProfile()).toBe(first)
    expect(first).toEqual(defaultToBotProfile())
    expect(tobotDisplayName(first, 'ToBot')).toBe('ToBot')
    expect(tobotHostContext(first, 'ToBot')).toBeNull()
  })

  it('stores a name, a personality, and a face, and sends only the identity', () => {
    const saved = writeToBotProfile({
      name: '  小团\n ',
      personality: ' 温和，只用短句 ',
      shape: 'drop',
      expression: 'glad',
      color: 'blue',
      eyes: 'wink'
    })
    expect(saved.name).toBe('小团')
    expect(saved.personality).toBe('温和，只用短句')
    expect(readToBotProfile()).toBe(saved)
    expect(localStorage.getItem(TOBOT_PROFILE_KEY)).toContain('"shape":"drop"')
    expect(tobotDisplayName(saved, 'ToBot')).toBe('小团')
    expect(tobotHostContext(saved, 'ToBot')).toEqual({ personaName: '小团', persona: '温和，只用短句' })
    expect(tobotHostContext({ ...saved, name: '', personality: '' }, 'ToBot')).toBeNull()
    expect(tobotHostContext({ ...defaultToBotProfile(), personality: '直接' }, 'ToBot')).toEqual({
      personaName: 'ToBot',
      persona: '直接'
    })
  })

  it('drops an unknown face and a broken record', () => {
    expect(normalizeProfile({ shape: 'nope', expression: 'nope', color: 'nope', eyes: 'nope' })).toEqual(
      defaultToBotProfile()
    )
    localStorage.setItem(TOBOT_PROFILE_KEY, '{')
    expect(readToBotProfile()).toEqual(defaultToBotProfile())
  })

  it('gives every shape and expression its own drawing', () => {
    const shapes = new Set(TOBOT_SHAPES.map((shape) => tobotShapePath(shape)))
    expect(shapes.size).toBe(TOBOT_SHAPES.length)
    const faces = new Set(
      TOBOT_EXPRESSIONS.map((expression) => tobotFaceSignature({ shape: 'blob', expression, eyes: 'pill' }))
    )
    expect(faces.size).toBe(TOBOT_EXPRESSIONS.length)
    expect(tobotFaceSignature({ shape: 'blob', expression: 'listen', eyes: 'pill' })).not.toBe(
      tobotFaceSignature({ shape: 'blob', expression: 'listen', eyes: 'wink' })
    )
  })
})
