/** Grapheme clusters, so a family emoji or a CJK character stays one unit. */
function graphemes(value: string): string[] {
  if (typeof Intl !== 'undefined' && 'Segmenter' in Intl) {
    return [...new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(value)].map((part) => part.segment)
  }
  return [...value]
}

function maskToken(token: string): string {
  const chars = graphemes(token)
  if (chars.length === 0) return ''
  if (chars.length === 1) return `${chars[0]}*`
  if (chars.length === 2) return `${chars[0]}*`
  const hidden = Math.min(chars.length - 2, 2)
  return `${chars[0]}${'*'.repeat(hidden)}${chars[chars.length - 1]}`
}

function maskPhone(value: string): string {
  let digits = value.replace(/\D/g, '')
  if (digits.startsWith('86') && digits.length === 13) digits = digits.slice(2)
  if (digits.length >= 11) return `${digits.slice(0, 3)}****${digits.slice(-4)}`
  if (digits.length >= 7) return `${digits.slice(0, 2)}****${digits.slice(-2)}`
  return '*'.repeat(Math.max(digits.length, 1))
}

function maskEmail(value: string): string {
  const at = value.indexOf('@')
  const local = value.slice(0, at)
  const domain = value.slice(at + 1)
  const head = graphemes(local)[0]
  if (!head || !domain || domain.includes('@')) return maskToken(value)
  return `${head}***@${domain}`
}

/**
 * Mask a display name for a consent card.
 * A phone-shaped or mailbox-shaped name uses that mask. The raw value is not returned.
 */
export function maskDisplayName(value: string): string {
  const name = value.trim().replace(/[\u0000-\u001f]/g, '')
  if (!name) return ''
  if (/^(?:\+?86[-\s]?)?1[3-9]\d{9}$/.test(name.replace(/\s/g, ''))) return maskPhone(name)
  if (/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(name)) return maskEmail(name)
  const parts = name.split(/\s+/).filter(Boolean)
  if (parts.length > 1) return parts.map((part) => maskToken(part)).join(' ')
  return maskToken(name)
}
