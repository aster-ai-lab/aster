import { SearchSelect } from '@a3s-lab/ui'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

import { SettingDivider, SettingRow, SettingRowTitle } from '@renderer/components/SettingsPrimitives'

import {
  CHAT_WIDTHS,
  CONTRASTS,
  DENSITIES,
  readAppearanceLayout,
  THEME_PACKS,
  writeAppearanceLayout,
  type AppearanceLayout,
  type ChatWidthId,
  type ContrastId,
  type DensityId,
  type ThemePackId
} from './appearanceLayout'

const PACK_LABELS: Record<ThemePackId, string> = {
  default: 'settings.aligned.appearance.theme_pack.default',
  ocean: 'settings.aligned.appearance.theme_pack.ocean',
  paper: 'settings.aligned.appearance.theme_pack.paper'
}

const DENSITY_LABELS: Record<DensityId, string> = {
  compact: 'settings.aligned.appearance.density.compact',
  comfortable: 'settings.aligned.appearance.density.comfortable',
  airy: 'settings.aligned.appearance.density.airy'
}

const WIDTH_LABELS: Record<ChatWidthId, string> = {
  standard: 'settings.aligned.appearance.chat_width.standard',
  full: 'settings.aligned.appearance.chat_width.full'
}

const CONTRAST_LABELS: Record<ContrastId, string> = {
  default: 'settings.aligned.appearance.contrast.default',
  high: 'settings.aligned.appearance.contrast.high'
}

/** Theme pack, density, chat width, and contrast. These are rows, not new page headings. */
export function AppearanceLayoutRows() {
  const { t } = useTranslation()
  const [layout, setLayout] = useState<AppearanceLayout>(readAppearanceLayout)

  const apply = (patch: Partial<AppearanceLayout>) => {
    const next = { ...layout, ...patch }
    setLayout(next)
    void writeAppearanceLayout(patch)
  }

  return (
    <>
      <SettingDivider />
      <SettingRow id="setting-appearance-theme-pack" className="scroll-mt-6">
        <SettingRowTitle>{t('settings.aligned.appearance.theme_pack')}</SettingRowTitle>
        <SearchSelect
          size="sm"
          searchable={false}
          allowClear={false}
          aria-label={t('settings.aligned.appearance.theme_pack')}
          value={layout.themePack}
          options={THEME_PACKS.map((id) => ({ value: id, label: t(PACK_LABELS[id]) }))}
          onChange={(raw) => {
            const next = raw as ThemePackId
            if (THEME_PACKS.includes(next)) apply({ themePack: next })
          }}
          className="w-[180px]"
        />
      </SettingRow>
      <SettingDivider />
      <SettingRow id="setting-appearance-density" className="scroll-mt-6">
        <SettingRowTitle>{t('settings.aligned.appearance.density')}</SettingRowTitle>
        <SearchSelect
          size="sm"
          searchable={false}
          allowClear={false}
          aria-label={t('settings.aligned.appearance.density')}
          value={layout.density}
          options={DENSITIES.map((id) => ({ value: id, label: t(DENSITY_LABELS[id]) }))}
          onChange={(raw) => {
            const next = raw as DensityId
            if (DENSITIES.includes(next)) apply({ density: next })
          }}
          className="w-[180px]"
        />
      </SettingRow>
      <SettingDivider />
      <SettingRow id="setting-appearance-chat-width" className="scroll-mt-6">
        <SettingRowTitle>{t('settings.aligned.appearance.chat_width')}</SettingRowTitle>
        <SearchSelect
          size="sm"
          searchable={false}
          allowClear={false}
          aria-label={t('settings.aligned.appearance.chat_width')}
          value={layout.chatWidth}
          options={CHAT_WIDTHS.map((id) => ({ value: id, label: t(WIDTH_LABELS[id]) }))}
          onChange={(raw) => {
            const next = raw as ChatWidthId
            if (CHAT_WIDTHS.includes(next)) apply({ chatWidth: next })
          }}
          className="w-[180px]"
        />
      </SettingRow>
      <SettingDivider />
      <SettingRow id="setting-appearance-contrast" className="scroll-mt-6">
        <SettingRowTitle>{t('settings.aligned.appearance.contrast')}</SettingRowTitle>
        <SearchSelect
          size="sm"
          searchable={false}
          allowClear={false}
          aria-label={t('settings.aligned.appearance.contrast')}
          value={layout.contrast}
          options={CONTRASTS.map((id) => ({ value: id, label: t(CONTRAST_LABELS[id]) }))}
          onChange={(raw) => {
            const next = raw as ContrastId
            if (CONTRASTS.includes(next)) apply({ contrast: next })
          }}
          className="w-[180px]"
        />
      </SettingRow>
    </>
  )
}
