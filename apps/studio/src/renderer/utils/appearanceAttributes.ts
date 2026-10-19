import { preferenceService } from '@data/PreferenceService'

export type AppearanceAttributeLayout = {
  themePack: 'default' | 'ocean' | 'paper'
  density: 'compact' | 'comfortable' | 'airy'
  chatWidth: 'standard' | 'full'
  contrast: 'default' | 'high'
}

const THEME_PACKS = ['default', 'ocean', 'paper'] as const satisfies readonly AppearanceAttributeLayout['themePack'][]
const DENSITIES = ['compact', 'comfortable', 'airy'] as const satisfies readonly AppearanceAttributeLayout['density'][]
const CHAT_WIDTHS = ['standard', 'full'] as const satisfies readonly AppearanceAttributeLayout['chatWidth'][]
const CONTRASTS = ['default', 'high'] as const satisfies readonly AppearanceAttributeLayout['contrast'][]

function pick<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === 'string' && (allowed as readonly string[]).includes(value) ? (value as T) : fallback
}

/** Reads the last cached appearance layout. Unknown values fall back to the shipped defaults. */
export function readCachedAppearanceLayout(): AppearanceAttributeLayout {
  return {
    themePack: pick(preferenceService.getCachedValue('app.appearance.theme_pack'), THEME_PACKS, 'default'),
    density: pick(preferenceService.getCachedValue('app.appearance.density'), DENSITIES, 'comfortable'),
    chatWidth: pick(preferenceService.getCachedValue('app.appearance.chat_width'), CHAT_WIDTHS, 'standard'),
    contrast: pick(preferenceService.getCachedValue('app.appearance.contrast'), CONTRASTS, 'default')
  }
}

const DENSITY_GAP = {
  compact: '8px',
  comfortable: '12px',
  airy: '18px'
} as const

/** Writes the appearance choices onto the document so theme, density, width, and contrast take effect. */
export function applyAppearanceAttributes(layout: AppearanceAttributeLayout): void {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  root.dataset.themePack = layout.themePack
  root.dataset.density = layout.density
  root.dataset.chatWidth = layout.chatWidth
  root.dataset.contrast = layout.contrast
  root.style.setProperty('--app-chat-max-width', layout.chatWidth === 'full' ? 'none' : '768px')
  root.style.setProperty('--app-density-gap', DENSITY_GAP[layout.density])
}
