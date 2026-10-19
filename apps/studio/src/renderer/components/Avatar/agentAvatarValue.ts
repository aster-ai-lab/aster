import { catalogIconPath } from '@renderer/services/capabilityIcon'

/**
 * What an agent avatar string holds. One stored string carries an emoji, a bundled
 * image, a remote or inline image, or an authenticated catalog icon path; every
 * renderer reads it through `readAgentAvatar` instead of guessing on its own.
 */
export type AgentAvatarValue =
  | { kind: 'glyph'; glyph: string }
  | { kind: 'image'; src: string }
  | { kind: 'catalog-icon'; path: string }

const IMAGE_ADDRESS = /^(?:https?:|blob:|file:|asset:|data:image\/)/i
const GLYPH_LIMIT = 2

const segmenter = typeof Intl !== 'undefined' && 'Segmenter' in Intl ? new Intl.Segmenter() : null

function graphemeCount(value: string): number {
  if (!segmenter) return Array.from(value).length
  let count = 0
  for (const _ of segmenter.segment(value)) count += 1
  return count
}

/** Null when the string is no avatar a renderer can show; the caller draws its fallback. */
export function readAgentAvatar(raw: string | null | undefined): AgentAvatarValue | null {
  const value = raw?.trim()
  if (!value) return null
  const catalog = catalogIconPath(value)
  if (catalog) return { kind: 'catalog-icon', path: catalog }
  if (IMAGE_ADDRESS.test(value) || (value.startsWith('/') && !value.startsWith('//'))) {
    return { kind: 'image', src: value }
  }
  if (/[\s/\\:]/.test(value) || graphemeCount(value) > GLYPH_LIMIT) return null
  return { kind: 'glyph', glyph: value }
}

/** True when the string can draw as an avatar at all. */
export function isAgentAvatar(raw: string | null | undefined): boolean {
  return readAgentAvatar(raw) !== null
}

/** The first letter of the name, the last resort an avatar draws. */
export function agentAvatarInitial(name: string | null | undefined): string {
  const trimmed = name?.trim() ?? ''
  if (!trimmed) return ''
  if (!segmenter) return Array.from(trimmed)[0]?.toUpperCase() ?? ''
  for (const { segment } of segmenter.segment(trimmed)) return segment.toUpperCase()
  return ''
}
