import { useTranslation } from 'react-i18next'

/** Studio stand-in for the console docs runtime. The console shim reads console prefs. */
export function useLang(): string {
  const { i18n } = useTranslation()
  return i18n.language.toLowerCase().startsWith('en') ? 'en' : 'zh'
}

export function useVersion(): string {
  return '1.1.0'
}

export function useSite() {
  return { site: { multiVersion: { default: '1.1.0', versions: ['1.1.0'] } } }
}

export function withBase(path: string): string {
  return path
}
