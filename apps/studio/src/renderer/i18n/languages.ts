import type { LanguageVarious } from '@shared/data/preference/preferenceTypes'
import { languageNativeNameMap } from '@shared/utils/languages'

/** Product UI languages. Anything else falls back to Chinese. */
const APP_LANGUAGE_FLAGS: ReadonlyArray<{ value: 'zh-CN' | 'en-US'; flag: string }> = [
  { value: 'zh-CN', flag: '🇨🇳' },
  { value: 'en-US', flag: '🇺🇸' }
]

export const defaultAppLanguage = 'zh-CN' as const

export const appLanguageOptions: ReadonlyArray<{
  value: LanguageVarious
  label: string
  flag: string
}> = APP_LANGUAGE_FLAGS.map(({ value, flag }) => ({ value, flag, label: languageNativeNameMap[value] }))

export function isAppLanguage(value: string | null | undefined): value is 'zh-CN' | 'en-US' {
  return value === 'zh-CN' || value === 'en-US'
}

/** Chinese when unset or when a stored language is outside the product pair. */
export function resolveAppLanguage(value: string | null | undefined): 'zh-CN' | 'en-US' {
  return isAppLanguage(value) ? value : defaultAppLanguage
}
