import { preferenceService } from '@data/PreferenceService'
import { applyAppearanceAttributes, readCachedAppearanceLayout } from '@renderer/utils/appearanceAttributes'
import type {
  AppearanceChatWidth,
  AppearanceContrast,
  AppearanceDensity,
  AppearanceThemePack
} from '@shared/data/preference/preferenceTypes'

export const THEME_PACKS = ['default', 'ocean', 'paper'] as const satisfies readonly AppearanceThemePack[]
export const DENSITIES = ['compact', 'comfortable', 'airy'] as const satisfies readonly AppearanceDensity[]
export const CHAT_WIDTHS = ['standard', 'full'] as const satisfies readonly AppearanceChatWidth[]
export const CONTRASTS = ['default', 'high'] as const satisfies readonly AppearanceContrast[]

export type ThemePackId = AppearanceThemePack
export type DensityId = AppearanceDensity
export type ChatWidthId = AppearanceChatWidth
export type ContrastId = AppearanceContrast

export type AppearanceLayout = {
  themePack: ThemePackId
  density: DensityId
  chatWidth: ChatWidthId
  contrast: ContrastId
}

export function readAppearanceLayout(): AppearanceLayout {
  return readCachedAppearanceLayout()
}

export async function writeAppearanceLayout(patch: Partial<AppearanceLayout>): Promise<AppearanceLayout> {
  const next = { ...readAppearanceLayout(), ...patch }
  // Paint the layout before the preference write so the page updates in the same turn.
  applyAppearanceAttributes(next)
  if (patch.themePack) await preferenceService.set('app.appearance.theme_pack', patch.themePack)
  if (patch.density) await preferenceService.set('app.appearance.density', patch.density)
  if (patch.chatWidth) await preferenceService.set('app.appearance.chat_width', patch.chatWidth)
  if (patch.contrast) await preferenceService.set('app.appearance.contrast', patch.contrast)
  return next
}
