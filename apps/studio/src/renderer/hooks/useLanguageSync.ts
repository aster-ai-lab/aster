import { useEffect } from 'react'

import { usePreference } from '@data/hooks/usePreference'
import { resolveAppLanguage } from '@renderer/i18n/languages'
import i18n from '@renderer/i18n/resolver'

/**
 * Applies the product language (Chinese or English) for every UI window.
 * A missing or unsupported preference stays Chinese.
 * It deliberately does NOT touch the dayjs locale.
 */
export function useLanguageSync(): void {
  const [language] = usePreference('app.language')

  useEffect(() => {
    void i18n.changeLanguage(resolveAppLanguage(language))
  }, [language])
}
