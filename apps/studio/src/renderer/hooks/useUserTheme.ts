import Color from 'color'

import { usePreference } from '@data/hooks/usePreference'
import { studioIsAsterTenant } from '@renderer/tenantBrand'
import { applyAppearanceAttributes, readCachedAppearanceLayout } from '@renderer/utils/appearanceAttributes'
import { getForegroundColor } from '@renderer/utils/style'
import { LEGACY_THEME_PRIMARY, resolveThemePrimary } from '@renderer/utils/tenantTheme'

export default function useUserTheme() {
  const [colorPrimary, setColorPrimary] = usePreference('ui.theme_user.color_primary')
  const [userFontFamily, setUserFontFamily] = usePreference('ui.theme_user.font_family')
  const [userCodeFontFamily, setUserCodeFontFamily] = usePreference('ui.theme_user.code_font_family')
  const [themePack] = usePreference('app.appearance.theme_pack')
  const [density] = usePreference('app.appearance.density')
  const [chatWidth] = usePreference('app.appearance.chat_width')
  const [contrast] = usePreference('app.appearance.contrast')

  const setOptionalCssVar = (name: string, value?: string) => {
    if (value?.trim()) {
      document.documentElement.style.setProperty(name, `'${value}'`)
      return
    }

    document.documentElement.style.removeProperty(name)
  }

  const initUserTheme = (theme: { colorPrimary: string } = { colorPrimary }) => {
    const colorPrimary = Color(resolveThemePrimary(theme.colorPrimary))

    const savedPrimary = (theme.colorPrimary ?? '').trim()
    if (studioIsAsterTenant() && (!savedPrimary || savedPrimary.toLowerCase() === LEGACY_THEME_PRIMARY)) {
      // The tenant stylesheet changes its default with light/dark; a chosen color stays inline.
      document.documentElement.style.removeProperty('--cs-theme-primary')
      document.documentElement.style.removeProperty('--cs-theme-primary-foreground')
    } else {
      document.documentElement.style.setProperty('--cs-theme-primary', colorPrimary.toString())
      document.documentElement.style.setProperty(
        '--cs-theme-primary-foreground',
        getForegroundColor(colorPrimary.hex())
      )
    }
    setOptionalCssVar('--app-user-font-family', userFontFamily)
    setOptionalCssVar('--app-user-code-font-family', userCodeFontFamily)
    const saved = readCachedAppearanceLayout()
    applyAppearanceAttributes({
      themePack:
        themePack === 'ocean' || themePack === 'paper' || themePack === 'default' ? themePack : saved.themePack,
      density: density === 'compact' || density === 'comfortable' || density === 'airy' ? density : saved.density,
      chatWidth: chatWidth === 'standard' || chatWidth === 'full' ? chatWidth : saved.chatWidth,
      contrast: contrast === 'default' || contrast === 'high' ? contrast : saved.contrast
    })
  }

  return {
    colorPrimary: Color(resolveThemePrimary(colorPrimary)),

    initUserTheme,

    userTheme: { colorPrimary: resolveThemePrimary(colorPrimary), userFontFamily, userCodeFontFamily },

    setUserTheme(userTheme: { colorPrimary: string; userFontFamily: string; userCodeFontFamily: string }) {
      void setColorPrimary(userTheme.colorPrimary)
      void setUserFontFamily(userTheme.userFontFamily)
      void setUserCodeFontFamily(userTheme.userCodeFontFamily)
      initUserTheme(userTheme)
    }
  }
}
