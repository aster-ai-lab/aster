import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@a3s-lab/ui'
import { AppWindow, Home } from 'lucide-react'
import { useState, type FC } from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import {
  SettingDivider,
  SettingGroup,
  SettingRow,
  SettingRowTitle,
  SettingsContentColumn,
  SettingTitle
} from '@renderer/components/SettingsPrimitives'
import { useTheme } from '@renderer/hooks/useTheme'
import { appLanguageOptions, isAppLanguage, resolveAppLanguage } from '@renderer/i18n/languages'
import i18n from '@renderer/i18n/resolver'
import { popup } from '@renderer/services/popup'
import { toast } from '@renderer/services/toast'
import { formatErrorMessage } from '@renderer/utils/error'
import { cn } from '@renderer/utils/style'
import type { MenuPresentationMode } from '@shared/data/preference/preferenceTypes'
import { ThemeMode } from '@shared/data/preference/preferenceTypes'

type TFunction = (key: string) => string
type MenuPresentationModeChangeOptions = {
  currentMode: MenuPresentationMode
  mode: MenuPresentationMode
  setMenuPresentationMode: (mode: MenuPresentationMode) => Promise<unknown> | unknown
  setTimeoutTimer: (key: string, callback: () => void, delay: number) => void
  t: TFunction
}

export async function confirmMenuPresentationModeChange({
  currentMode,
  mode,
  setMenuPresentationMode,
  setTimeoutTimer,
  t
}: MenuPresentationModeChangeOptions): Promise<void> {
  if (mode === currentMode) return

  const confirmed = await popup.confirm({
    title: t('settings.general.common.menu.presentation_mode.restart.title'),
    content: t('settings.general.common.menu.presentation_mode.restart.content'),
    okText: t('common.confirm'),
    cancelText: t('common.cancel'),
    centered: true
  })
  if (!confirmed) return

  try {
    await setMenuPresentationMode(mode)
  } catch (error) {
    toast.error(formatErrorMessage(error))
    throw error
  }

  setTimeoutTimer(
    'handleMenuPresentationModeChange',
    () => {
      void window.api.application.relaunch()
    },
    500
  )
}

const themeCards = [
  { value: ThemeMode.light, labelKey: 'settings.theme.light' },
  { value: ThemeMode.dark, labelKey: 'settings.theme.dark' },
  { value: ThemeMode.system, labelKey: 'settings.appearance.follow_system' }
] as const

function WindowFace({ tone }: { tone: 'light' | 'dark' }) {
  const dark = tone === 'dark'
  return (
    <div
      data-theme-sample=""
      style={{
        display: 'flex',
        minWidth: 0,
        flex: 1,
        flexDirection: 'column',
        background: dark
          ? 'linear-gradient(180deg, #243044 0 36%, #1b2433 36%)'
          : 'linear-gradient(180deg, #d5e0ee 0 36%, #ffffff 36%)'
      }}>
      <div style={{ height: '36%' }} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '8px 10px 0' }}>
        <Home size={12} color={dark ? '#d7dde8' : '#8d97a6'} strokeWidth={1.8} aria-hidden />
        <span
          style={{
            height: 6,
            width: 34,
            borderRadius: 999,
            background: dark ? '#314056' : '#e6e8ee'
          }}
        />
      </div>
    </div>
  )
}

function WindowPreview({ mode }: { mode: ThemeMode }) {
  const faces = mode === ThemeMode.system ? (['light', 'dark'] as const) : ([mode] as const)
  return (
    <div className="flex h-full overflow-hidden rounded-[10px]">
      {faces.map((tone) => (
        <WindowFace key={tone} tone={tone} />
      ))}
    </div>
  )
}

const sectionTitleClass = 'justify-start gap-2 text-[13px] font-medium'

const AppearanceSettings: FC = () => {
  const { t } = useTranslation()
  const { theme, settedTheme, setTheme } = useTheme()
  const [language, setLanguage] = usePreference('app.language')
  const displayLanguage = resolveAppLanguage(language)
  const [languageMenuRoot, setLanguageMenuRoot] = useState<HTMLDivElement | null>(null)

  const onSelectLanguage = (value: string) => {
    if (!isAppLanguage(value)) return
    void i18n.changeLanguage(value)
    void setLanguage(value)
  }

  return (
    <SettingsContentColumn theme={theme} innerClassName="[&>*+*]:mt-8">
      <SettingGroup theme={theme} variant="plain" className="mt-0 border-0 pt-0">
        <SettingTitle className={sectionTitleClass}>
          <AppWindow className="size-4" strokeWidth={1.7} aria-hidden />
          {t('settings.appearance.window')}
        </SettingTitle>
        <div
          id="setting-appearance-theme-mode"
          role="radiogroup"
          aria-label={t('settings.appearance.window')}
          className="mt-4 grid scroll-mt-6 grid-cols-3 gap-4">
          {themeCards.map((card) => {
            const selected = settedTheme === card.value
            return (
              <button
                key={card.value}
                type="button"
                role="radio"
                aria-checked={selected}
                onClick={() => setTheme(card.value)}
                className="group flex cursor-pointer flex-col items-center gap-2.5 border-0 bg-transparent p-0 outline-none focus-visible:rounded-[12px] focus-visible:ring-2 focus-visible:ring-black/10">
                <span
                  data-theme-preview=""
                  className={cn(
                    'aspect-[16/10] w-full overflow-hidden rounded-[12px] border-2 transition-colors',
                    selected ? 'border-[#171717]' : 'border-transparent'
                  )}>
                  <WindowPreview mode={card.value} />
                </span>
                <span className="text-[13px] leading-5 text-[#3f3f3f]">{t(card.labelKey)}</span>
              </button>
            )
          })}
        </div>
      </SettingGroup>

      <SettingGroup theme={theme} variant="plain" className="border-0 pt-0">
        <SettingTitle>{t('settings.general.common.sections.display_language')}</SettingTitle>
        <SettingDivider />
        <SettingRow id="setting-appearance-display-language" className="scroll-mt-6">
          <SettingRowTitle>{t('common.language')}</SettingRowTitle>
          <div ref={setLanguageMenuRoot}>
            <Select value={displayLanguage} onValueChange={onSelectLanguage}>
              <SelectTrigger
                aria-label={t('common.language')}
                className="h-9 min-w-36 rounded-[8px] border-0 bg-[#f5f5f5] px-3 text-[13px] text-[#171717] shadow-none focus-visible:border-transparent focus-visible:ring-2 focus-visible:ring-black/10">
                <SelectValue />
              </SelectTrigger>
              <SelectContent
                portalContainer={languageMenuRoot ?? undefined}
                align="end"
                className="rounded-[8px] border-[#ececec] bg-white text-[#171717]">
                {appLanguageOptions.map((lang) => (
                  <SelectItem key={lang.value} value={lang.value}>
                    {lang.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </SettingRow>
      </SettingGroup>
    </SettingsContentColumn>
  )
}

export default AppearanceSettings
