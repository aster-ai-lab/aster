/**
 * Original ToBot face. Shape, expression, color, and eyes are our own paths.
 * They are not taken from grok-icon-study or from Grok Bot geometry.
 */
import type { ToBotColor, ToBotExpression, ToBotEyes, ToBotShape } from '@renderer/im/tobotProfile'

export const TOBOT_COLOR_HEX: Record<ToBotColor, string> = {
  ink: '#171717',
  brown: '#8A6A4F',
  red: '#E15B5B',
  orange: '#F08A3A',
  gold: '#E6B325',
  green: '#6FBF73',
  teal: '#3FBFB0',
  blue: '#5B8DEF',
  violet: '#8B6CFF',
  pink: '#E56BB0',
  gray: '#8E8E8E'
}

export function tobotEyeInk(color: ToBotColor): string {
  return color === 'gold' ? '#171717' : '#ffffff'
}

const SHAPE_PATH: Record<ToBotShape, string> = {
  blob: 'M32 10a22 22 0 1 0 0.01 0',
  bean: 'M8 32a24 16 0 1 0 48 0a24 16 0 1 0 -48 0',
  egg: 'M32 8c10 0 18 12 18 26S42 58 32 58 14 48 14 34 22 8 32 8z',
  drop: 'M32 8c8 10 16 18 16 28a16 16 0 1 1-32 0c0-10 8-18 16-28z',
  cloud: 'M22 42h22c5 0 9-4 9-8 0-4-3-7-7-8 0-6-5-11-12-11-5 0-10 3-11 8-5 0-9 4-9 9 0 5 4 10 8 10z',
  leaf: 'M16 46c12-6 14-24 32-34-8 16-14 28-24 38-4-1-6-2-8-4z',
  cube: 'M16 14h32a6 6 0 0 1 6 6v24a6 6 0 0 1-6 6H16a6 6 0 0 1-6-6V20a6 6 0 0 1 6-6z',
  capsule: 'M22 14h20a16 16 0 0 1 0 36H22a16 16 0 0 1 0-36z'
}

export function tobotShapePath(shape: ToBotShape): string {
  return SHAPE_PATH[shape]
}

type Eye = { x: number; y: number; rx: number; ry: number; rot: number }

function eye(x: number, y: number, rx = 4.2, ry = 8.5, rot = 0): Eye {
  return { x, y, rx, ry, rot }
}

function baseEyes(expression: ToBotExpression): [Eye, Eye] {
  switch (expression) {
    case 'rest':
      return [eye(24, 34, 6, 1.2), eye(40, 34, 6, 1.2)]
    case 'listen':
      return [eye(24, 32), eye(40, 32)]
    case 'excited':
      return [eye(23, 28, 5, 10), eye(41, 28, 5, 10)]
    case 'surprised':
      return [eye(23, 30, 6, 7), eye(41, 30, 6, 7)]
    case 'skeptical':
      return [eye(23, 30, 4.2, 7, -18), eye(41, 34, 4.2, 7, 12)]
    case 'angry':
      return [eye(24, 31, 4.5, 7, 24), eye(40, 31, 4.5, 7, -24)]
    case 'sleepy':
      return [eye(24, 35, 6, 2), eye(40, 35, 6, 2)]
    case 'glad':
      return [eye(24, 31, 4.5, 7, -12), eye(40, 31, 4.5, 7, 12)]
    case 'curious':
      return [eye(22, 30, 5, 10), eye(42, 33, 3.2, 6)]
    case 'confused':
      return [eye(23, 31, 4, 8, 16), eye(41, 33, 4, 6, -8)]
    case 'bored':
      return [eye(24, 33, 6, 1.4), eye(40, 33, 6, 1.4)]
    case 'proud':
      return [eye(25, 28, 3.4, 6), eye(39, 28, 3.4, 6)]
    case 'shy':
      return [eye(27, 36, 3, 5), eye(37, 36, 3, 5)]
    case 'sad':
      return [eye(24, 34, 4, 7, -20), eye(40, 34, 4, 7, 20)]
    case 'laugh':
      return [eye(24, 32, 5, 2, -8), eye(40, 32, 5, 2, 8)]
    case 'scared':
      return [eye(20, 30, 3, 5), eye(44, 30, 3, 5)]
    case 'playful':
      return [eye(24, 33, 5, 1.3), eye(40, 31, 4, 8)]
  }
}

function styleEye(eyeMark: Eye, style: ToBotEyes, side: 'left' | 'right'): Eye {
  if (style === 'dot') return { ...eyeMark, rx: 2.2, ry: 2.2, rot: 0 }
  if (style === 'round') {
    const radius = Math.max(eyeMark.rx, eyeMark.ry) * 0.62
    return { ...eyeMark, rx: radius, ry: radius, rot: 0 }
  }
  if (style === 'line' || (style === 'wink' && side === 'left')) {
    return { ...eyeMark, rx: Math.max(eyeMark.rx, 5), ry: 1.2, rot: eyeMark.rot }
  }
  return eyeMark
}

export function tobotEyes(expression: ToBotExpression, style: ToBotEyes): [Eye, Eye] {
  const [left, right] = baseEyes(expression)
  return [styleEye(left, style, 'left'), styleEye(right, style, 'right')]
}

export function tobotMouth(expression: ToBotExpression): string | null {
  switch (expression) {
    case 'glad':
    case 'playful':
      return 'M26 42c2 3 10 3 12 0'
    case 'laugh':
      return 'M24 40c2 8 14 8 16 0'
    case 'sad':
    case 'scared':
      return 'M26 46c2-3 10-3 12 0'
    default:
      return null
  }
}

export function tobotFaceSignature(input: { shape: ToBotShape; expression: ToBotExpression; eyes: ToBotEyes }): string {
  const eyes = tobotEyes(input.expression, input.eyes)
  const drawn = eyes.map((item) => `${item.x},${item.y},${item.rx},${item.ry},${item.rot}`).join(';')
  return `${input.shape}|${drawn}|${tobotMouth(input.expression) ?? ''}`
}
