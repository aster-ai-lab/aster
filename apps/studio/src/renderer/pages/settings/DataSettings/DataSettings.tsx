import { MenuItem, MenuList } from '@a3s-lab/ui'
import { getRouteApi, useNavigate } from '@tanstack/react-router'
import { type FC, lazy, Suspense } from 'react'
import { useTranslation } from 'react-i18next'

import Scrollbar from '@renderer/components/Scrollbar'
import { SettingsContentColumn } from '@renderer/components/SettingsPrimitives'
import { useTheme } from '@renderer/hooks/useTheme'
import {
  settingsSubmenuItemClassName,
  settingsSubmenuItemLabelClassName,
  settingsSubmenuListClassName
} from '@renderer/pages/settings/settingsStyles'
import type { AppRouter } from '@renderer/types/router'

import BasicDataSettings from './BasicDataSettings'
import { dataPanelUsesToBot } from './dataLink'
import { dataNavSpec } from './dataNav'
import { DATA_PANEL_KEYS, DEFAULT_DATA_PANEL } from './dataPanels'

const dataRouteApi = getRouteApi('/settings/data')

const ExportMenuOptions = lazy(() => import('./ExportMenuSettings'))
const JoplinSettings = lazy(() => import('./JoplinSettings'))
const LocalBackupSettings = lazy(() => import('./LocalBackupSettings'))
const MarkdownExportSettings = lazy(() => import('./MarkdownExportSettings'))
const NotionSettings = lazy(() => import('./NotionSettings'))
const NutstoreSettings = lazy(() => import('./NutstoreSettings'))
const ObsidianSettings = lazy(() => import('./ObsidianSettings'))
const S3Settings = lazy(() => import('./S3Settings'))
const SiyuanSettings = lazy(() => import('./SiyuanSettings'))
const WebDavSettings = lazy(() => import('./WebDavSettings'))
const YuqueSettings = lazy(() => import('./YuqueSettings'))
const ImportMenuOptions = lazy(() => import('./ImportMenuSettings'))

const DataSettings: FC = () => {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { theme } = useTheme()
  // The URL is the single source of truth. Unknown values were already dropped by the route schema.
  const search = dataRouteApi.useSearch<AppRouter>()
  const rawPanel = typeof search.panel === 'string' ? search.panel : undefined
  const panelParam = rawPanel && (DATA_PANEL_KEYS as readonly string[]).includes(rawPanel) ? rawPanel : undefined
  const menu = panelParam ?? DEFAULT_DATA_PANEL

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-1">
      <nav
        aria-label={t('settings.data.title')}
        className="flex h-full min-h-0 w-[176px] min-w-[176px] shrink-0 flex-col border-r border-[#efefef] bg-white pt-2">
        <Scrollbar className="min-h-0 flex-1">
          <MenuList className={settingsSubmenuListClassName}>
            {dataNavSpec.map((item) => {
              const active = item.panel === menu
              return (
                <MenuItem
                  key={item.id}
                  label={t(item.titleKey)}
                  active={active}
                  aria-current={active ? 'page' : undefined}
                  onClick={() => void navigate({ to: '/settings/data', search: { panel: item.panel } })}
                  icon={item.icon}
                  className={settingsSubmenuItemClassName}
                  labelClassName={settingsSubmenuItemLabelClassName}
                />
              )
            })}
          </MenuList>
        </Scrollbar>
      </nav>
      <SettingsContentColumn theme={theme}>
        <p className="mb-4 text-[15px] leading-6 text-[#8a8a8a]">
          {t(dataPanelUsesToBot(menu) ? 'settings.data.tobot.channel' : 'settings.data.tobot.local')}
        </p>
        {menu === 'data' ? (
          <BasicDataSettings />
        ) : (
          <Suspense fallback={null}>
            {menu === 'webdav' && <WebDavSettings />}
            {menu === 'nutstore' && <NutstoreSettings />}
            {menu === 's3' && <S3Settings />}
            {menu === 'import_settings' && <ImportMenuOptions />}
            {menu === 'export_menu' && <ExportMenuOptions />}
            {menu === 'markdown_export' && <MarkdownExportSettings />}
            {menu === 'local_backup' && <LocalBackupSettings />}
            {menu === 'notion' && <NotionSettings />}
            {menu === 'yuque' && <YuqueSettings />}
            {menu === 'joplin' && <JoplinSettings />}
            {menu === 'obsidian' && <ObsidianSettings />}
            {menu === 'siyuan' && <SiyuanSettings />}
          </Suspense>
        )}
      </SettingsContentColumn>
    </div>
  )
}

export default DataSettings
