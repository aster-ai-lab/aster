/** Composable profile avatar. Indices are the saved part order. Do not reorder the lists. */

export const AVATAR_BACKGROUNDS = [
  '#F4D6C6',
  '#F6E3B4',
  '#D7E8C8',
  '#C9DDF6',
  '#E6D6F5',
  '#F6D0DC',
  '#E4DDD6',
  '#CDE8E4',
  '#F3C9A8',
  '#B9D7C9'
] as const

export const AVATAR_SKINS = ['#F8E0D0', '#F0C8A8', '#D9A06A', '#A86B3C', '#6B3E26', '#C68642', '#3F2918'] as const

export const AVATAR_HAIR_COLORS = [
  '#241C18',
  '#5C4033',
  '#A56B32',
  '#E6C98A',
  '#F4F1EC',
  '#2A3A66',
  '#C2546A',
  '#8C3B2A',
  '#C45C26',
  '#8E8E93'
] as const

export const AVATAR_HAIR_STYLES = [
  'short',
  'bob',
  'bun',
  'fringe',
  'wave',
  'none',
  'buzz',
  'tied-back',
  'long',
  'curly'
] as const

export const AVATAR_EYE_STYLES = ['dot', 'round', 'line', 'wink', 'happy', 'sleepy', 'closed', 'wide'] as const

export const AVATAR_MOUTH_STYLES = ['smile', 'flat', 'grin', 'open', 'smirk', 'soft'] as const

export const AVATAR_ACCESSORIES = ['none', 'glasses', 'blush', 'earring', 'headphones', 'bow', 'cap', 'beard'] as const

export type AvatarHairStyle = (typeof AVATAR_HAIR_STYLES)[number]
export type AvatarEyeStyle = (typeof AVATAR_EYE_STYLES)[number]
export type AvatarMouthStyle = (typeof AVATAR_MOUTH_STYLES)[number]
export type AvatarAccessory = (typeof AVATAR_ACCESSORIES)[number]

export type AvatarSpec = {
  background: number
  skin: number
  hair: number
  hairColor: number
  eyes: number
  mouth: number
  accessory: number
}

const PART_COUNTS = [
  AVATAR_BACKGROUNDS.length,
  AVATAR_SKINS.length,
  AVATAR_HAIR_STYLES.length,
  AVATAR_HAIR_COLORS.length,
  AVATAR_EYE_STYLES.length,
  AVATAR_MOUTH_STYLES.length,
  AVATAR_ACCESSORIES.length
] as const

const SPEC_ATTRIBUTE = 'data-a3s-avatar'

export function avatarSpecFromName(name: string): AvatarSpec {
  let hash = 2166136261
  const source = name.trim() || 'a3s'
  for (const char of source) hash = Math.imul(hash ^ (char.codePointAt(0) ?? 0), 16777619)
  const next = () => {
    hash = Math.imul(hash ^ (hash >>> 15), 2246822519) >>> 0
    return (hash & 0xffff) / 0x10000
  }
  return randomAvatarSpec(next)
}

export function randomAvatarSpec(random: () => number = Math.random): AvatarSpec {
  const pick = (length: number) => Math.min(length - 1, Math.floor(Math.max(0, random()) * length))
  return {
    background: pick(AVATAR_BACKGROUNDS.length),
    skin: pick(AVATAR_SKINS.length),
    hair: pick(AVATAR_HAIR_STYLES.length),
    hairColor: pick(AVATAR_HAIR_COLORS.length),
    eyes: pick(AVATAR_EYE_STYLES.length),
    mouth: pick(AVATAR_MOUTH_STYLES.length),
    accessory: pick(AVATAR_ACCESSORIES.length)
  }
}

export function avatarSpecParts(spec: AvatarSpec): number[] {
  return [spec.background, spec.skin, spec.hair, spec.hairColor, spec.eyes, spec.mouth, spec.accessory]
}

function specFromParts(parts: number[]): AvatarSpec | null {
  if (parts.length !== PART_COUNTS.length) return null
  if (parts.some((part, index) => !Number.isInteger(part) || part < 0 || part >= PART_COUNTS[index])) return null
  return {
    background: parts[0],
    skin: parts[1],
    hair: parts[2],
    hairColor: parts[3],
    eyes: parts[4],
    mouth: parts[5],
    accessory: parts[6]
  }
}

export function parseAvatarSpec(value: string): AvatarSpec | null {
  const marker = 'data:image/svg+xml;base64,'
  if (!value.startsWith(marker)) return null
  let xml = ''
  try {
    const binary = atob(value.slice(marker.length))
    const bytes = new Uint8Array(binary.length)
    for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
    xml = new TextDecoder().decode(bytes)
  } catch {
    return null
  }
  const match = new RegExp(`${SPEC_ATTRIBUTE}="([0-9.]+)"`).exec(xml)
  if (!match) return null
  return specFromParts(match[1].split('.').map((part) => Number(part)))
}

function eyes(style: AvatarEyeStyle): string {
  const ink = '#1C1917'
  const left = 52
  const right = 76
  const y = 80
  if (style === 'round') {
    return (
      `<circle cx="${left}" cy="${y}" r="7" fill="#fff"/>` +
      `<circle cx="${right}" cy="${y}" r="7" fill="#fff"/>` +
      `<circle cx="${left}" cy="${y}" r="3.5" fill="${ink}"/>` +
      `<circle cx="${right}" cy="${y}" r="3.5" fill="${ink}"/>`
    )
  }
  if (style === 'line') {
    return `<path d="M${left - 7} ${y}h14M${right - 7} ${y}h14" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>`
  }
  if (style === 'wink') {
    return (
      `<path d="M${left - 7} ${y}c3-4 9-4 14 0" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>` +
      `<circle cx="${right}" cy="${y}" r="3.4" fill="${ink}"/>`
    )
  }
  if (style === 'happy') {
    return `<path d="M${left - 7} ${y + 2}c3-6 11-6 14 0M${right - 7} ${y + 2}c3-6 11-6 14 0" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>`
  }
  if (style === 'sleepy') {
    return (
      `<path d="M${left - 7} ${y - 2}c3 6 11 6 14 0" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>` +
      `<path d="M${right - 7} ${y - 2}c3 6 11 6 14 0" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>` +
      `<circle cx="${left}" cy="${y + 1}" r="1.6" fill="${ink}"/><circle cx="${right}" cy="${y + 1}" r="1.6" fill="${ink}"/>`
    )
  }
  if (style === 'closed') {
    return `<path d="M${left - 7} ${y}c3 4 11 4 14 0M${right - 7} ${y}c3 4 11 4 14 0" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>`
  }
  if (style === 'wide') {
    return (
      `<circle cx="${left}" cy="${y}" r="8.5" fill="#fff"/>` +
      `<circle cx="${right}" cy="${y}" r="8.5" fill="#fff"/>` +
      `<circle cx="${left}" cy="${y}" r="3.2" fill="${ink}"/>` +
      `<circle cx="${right}" cy="${y}" r="3.2" fill="${ink}"/>` +
      `<circle cx="${left - 2}" cy="${y - 2}" r="1.3" fill="#fff"/>` +
      `<circle cx="${right - 2}" cy="${y - 2}" r="1.3" fill="#fff"/>`
    )
  }
  return `<circle cx="${left}" cy="${y}" r="3.4" fill="${ink}"/><circle cx="${right}" cy="${y}" r="3.4" fill="${ink}"/>`
}

function mouth(style: AvatarMouthStyle): string {
  const ink = '#1C1917'
  if (style === 'flat') {
    return `<path d="M54 96h20" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>`
  }
  if (style === 'grin') {
    return `<path d="M50 92c4 14 24 14 28 0-8 4-20 4-28 0z" fill="${ink}"/>`
  }
  if (style === 'open') {
    return `<ellipse cx="64" cy="98" rx="7" ry="8" fill="#3A241C"/>`
  }
  if (style === 'smirk') {
    return `<path d="M54 98c8-2 12-10 22-6" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>`
  }
  if (style === 'soft') {
    return `<path d="M56 96c4 6 12 6 16 0" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>`
  }
  return `<path d="M52 92c4 10 20 10 24 0" fill="none" stroke="${ink}" stroke-width="2.6" stroke-linecap="round"/>`
}

function hairBack(style: AvatarHairStyle, color: string): string {
  if (style === 'bob') {
    return `<path d="M28 86C28 44 42 30 64 30s36 14 36 56v26c-10 10-24 12-36 12s-26-2-36-12z" fill="${color}"/>`
  }
  if (style === 'long') {
    return `<path d="M24 84C24 40 40 26 64 26s40 14 40 58v42c-10 8-24 12-40 12s-30-4-40-12z" fill="${color}"/>`
  }
  if (style === 'curly') {
    return (
      `<circle cx="28" cy="74" r="13" fill="${color}"/>` +
      `<circle cx="100" cy="74" r="13" fill="${color}"/>` +
      `<circle cx="32" cy="96" r="11" fill="${color}"/>` +
      `<circle cx="96" cy="96" r="11" fill="${color}"/>` +
      `<circle cx="40" cy="112" r="9" fill="${color}"/>` +
      `<circle cx="88" cy="112" r="9" fill="${color}"/>`
    )
  }
  if (style === 'tied-back') {
    return `<ellipse cx="108" cy="72" rx="12" ry="18" fill="${color}"/><rect x="98" y="58" width="10" height="8" rx="3" fill="${color}"/>`
  }
  return ''
}

function hairFront(style: AvatarHairStyle, color: string): string {
  const cap = `<path d="M36 78C36 46 46 36 64 36s28 10 28 42c-8-14-16-20-28-20s-20 6-28 20z" fill="${color}"/>`
  if (style === 'short' || style === 'bun') {
    const bun = style === 'bun' ? `<circle cx="64" cy="30" r="11" fill="${color}"/>` : ''
    return cap + bun
  }
  if (style === 'fringe') {
    return `<path d="M38 74C38 46 48 34 64 34C80 34 90 46 90 74C86 60 78 64 64 62C50 64 42 60 38 74Z" fill="${color}"/>`
  }
  if (style === 'wave') {
    return (
      `<path d="M36 78C34 48 48 34 66 34c16 0 28 12 28 36-8-16-18-20-30-16-10 4-18 2-28 24z" fill="${color}"/>` +
      `<path d="M78 46c16 6 22 24 16 46-8-8-12-22-10-34-2-6-4-10-6-12z" fill="${color}"/>`
    )
  }
  if (style === 'bob') {
    return `<path d="M40 62c4-18 14-26 24-26s20 8 24 26c-8-8-16-8-24-4-8-4-16-4-24 4z" fill="${color}"/>`
  }
  if (style === 'buzz') {
    return `<path d="M42 70C42 54 51 46 64 46s22 8 22 24c-6-6-13-9-22-9s-16 3-22 9z" fill="${color}"/>`
  }
  if (style === 'tied-back') return cap
  if (style === 'long') {
    return `<path d="M44 68c2-24 10-34 20-34s18 10 20 34c-6-12-12-14-20-10-8-4-14-2-20 10z" fill="${color}"/>`
  }
  if (style === 'curly') {
    return (
      `<circle cx="46" cy="46" r="13" fill="${color}"/>` +
      `<circle cx="64" cy="38" r="12" fill="${color}"/>` +
      `<circle cx="82" cy="46" r="13" fill="${color}"/>` +
      `<path d="M40 66c2-10 10-16 24-16s22 6 24 16c-8-6-16-6-24-2-8-4-16-4-24 2z" fill="${color}"/>`
    )
  }
  return ''
}

function accessory(style: AvatarAccessory, hairColor: string): string {
  if (style === 'glasses') {
    return (
      `<rect x="40" y="67" width="22" height="15" rx="4" fill="none" stroke="#242424" stroke-width="2.2"/>` +
      `<rect x="66" y="67" width="22" height="15" rx="4" fill="none" stroke="#242424" stroke-width="2.2"/>` +
      `<path d="M62 74h4" fill="none" stroke="#242424" stroke-width="2.2" stroke-linecap="round"/>`
    )
  }
  if (style === 'blush') {
    return `<ellipse cx="44" cy="88" rx="6" ry="3.5" fill="#F08A9A"/><ellipse cx="84" cy="88" rx="6" ry="3.5" fill="#F08A9A"/>`
  }
  if (style === 'earring') {
    return `<circle cx="31" cy="90" r="3" fill="#E2B657"/><circle cx="97" cy="90" r="3" fill="#E2B657"/>`
  }
  if (style === 'headphones') {
    return (
      `<path d="M42 78c0-22 9-36 22-36s22 14 22 36" fill="none" stroke="#242424" stroke-width="4" stroke-linecap="round"/>` +
      `<rect x="30" y="72" width="14" height="20" rx="5" fill="#242424"/>` +
      `<rect x="84" y="72" width="14" height="20" rx="5" fill="#242424"/>` +
      `<rect x="33" y="76" width="8" height="12" rx="3" fill="#5C5C5C"/>` +
      `<rect x="87" y="76" width="8" height="12" rx="3" fill="#5C5C5C"/>`
    )
  }
  if (style === 'bow') {
    return (
      `<path d="M78 30c8-10 20-2 12 8-6 2-8 2-12-2-4 4-6 4-12 2 8-10 4-18 12-8z" fill="#C2546A"/>` +
      `<circle cx="78" cy="36" r="3.2" fill="#8E3048"/>`
    )
  }
  if (style === 'cap') {
    return (
      `<path d="M40 66c2-22 12-32 24-32s22 10 24 32z" fill="#242424"/>` +
      `<path d="M30 64h76c-2 8-16 12-38 12S32 72 30 64z" fill="#242424"/>` +
      `<path d="M48 46h10" fill="none" stroke="#F4F1EC" stroke-width="2.2" stroke-linecap="round"/>`
    )
  }
  if (style === 'beard') {
    return (
      `<path d="M50 90c6 5 22 5 28 0-8 3-20 3-28 0z" fill="${hairColor}"/>` +
      `<path d="M50 104c2 14 26 14 28 0-8 5-20 5-28 0z" fill="${hairColor}"/>`
    )
  }
  return ''
}

export function avatarSvg(spec: AvatarSpec): string {
  const safe = specFromParts(avatarSpecParts(spec)) ?? avatarSpecFromName('')
  const background = AVATAR_BACKGROUNDS[safe.background]
  const skin = AVATAR_SKINS[safe.skin]
  const hairColor = AVATAR_HAIR_COLORS[safe.hairColor]
  const hairStyle = AVATAR_HAIR_STYLES[safe.hair]
  const ears =
    hairStyle === 'bob' || hairStyle === 'long' || hairStyle === 'curly'
      ? ''
      : `<circle cx="33" cy="82" r="7" fill="${skin}"/><circle cx="95" cy="82" r="7" fill="${skin}"/>`
  const code = avatarSpecParts(safe).join('.')
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128" ${SPEC_ATTRIBUTE}="${code}">` +
    `<rect width="128" height="128" fill="${background}"/>` +
    hairBack(hairStyle, hairColor) +
    ears +
    `<circle cx="64" cy="80" r="30" fill="${skin}"/>` +
    hairFront(hairStyle, hairColor) +
    eyes(AVATAR_EYE_STYLES[safe.eyes]) +
    mouth(AVATAR_MOUTH_STYLES[safe.mouth]) +
    accessory(AVATAR_ACCESSORIES[safe.accessory], hairColor) +
    `</svg>`
  )
}

export function avatarDataUrl(spec: AvatarSpec): string {
  const xml = avatarSvg(spec)
  let binary = ''
  for (const byte of new TextEncoder().encode(xml)) binary += String.fromCharCode(byte)
  return `data:image/svg+xml;base64,${btoa(binary)}`
}
