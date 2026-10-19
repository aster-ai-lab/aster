import type { LanguageVarious } from '@shared/data/preference/preferenceTypes'

export const languageEnglishNameMap: Record<LanguageVarious, string> = {
  'en-US': 'English',
  'zh-CN': 'Chinese'
}

/** Native-script display name for each language — mirrors the labels in AppearanceSettings' language picker. */
export const languageNativeNameMap: Record<LanguageVarious, string> = {
  'zh-CN': '中文',
  'en-US': 'English'
}

export const defaultLanguage = 'en-US'
