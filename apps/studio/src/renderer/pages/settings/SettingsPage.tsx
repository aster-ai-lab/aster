import { MenuItem, MenuList } from '@a3s-lab/ui'
import { Outlet, useLocation, useNavigate, useSearch } from '@tanstack/react-router'
import { ArrowLeft, Search } from 'lucide-react'
import type { FC, MouseEvent } from 'react'
import { Fragment, useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import Scrollbar from '@renderer/components/Scrollbar'
import { matchSettingsNavItem, type SettingsNavItem, settingsNav } from '@renderer/components/settingsMenu'
import { PAGE_SIDEBAR_DEFAULT_WIDTH, pageSidebarEdgeClassName } from '@renderer/layout/pageSidebar'
import { useSettingsPageKeyboard } from '@renderer/pages/settings/settingsKeyboard'
import { useSettingsLeave } from '@renderer/pages/settings/settingsLeave'
import SettingsFocusScroll from '@renderer/pages/settings/settingsSearch/SettingsFocusScroll'
import SettingsFocusUrl from '@renderer/pages/settings/settingsSearch/SettingsFocusUrl'
import SettingsSearchBox from '@renderer/pages/settings/settingsSearch/SettingsSearchBox'
import { SettingsSearchDomIdsProvider } from '@renderer/pages/settings/settingsSearch/SettingsSearchDomIds'
import {
  settingsContentHeaderDescriptionClassName,
  settingsContentHeaderTitleClassName,
  settingsSubmenuItemClassName,
  settingsSubmenuItemLabelClassName,
  settingsSubmenuListClassName,
  settingsSubmenuSectionTitleClassName
} from '@renderer/pages/settings/settingsStyles'
import { openExternalWebsite } from '@renderer/services/website'

const SettingsPage: FC = () => {
  const location = useLocation()
  const navigate = useNavigate()
  const search = useSearch({ strict: false }) as { section?: unknown; panel?: unknown }
  const { pathname } = location
  const { t } = useTranslation()
  const section = typeof search.section === 'string' ? search.section : ''
  const panel = typeof search.panel === 'string' ? search.panel : ''
  const leaveSettings = useSettingsLeave()
  // Anchor-lookup scope for SettingsFocusScroll (this tab's content column)
  const contentRef = useRef<HTMLDivElement>(null)
  const searchParams = new URLSearchParams()
  if (section) searchParams.set('section', section)
  if (panel) searchParams.set('panel', panel)
  const query = searchParams.toString()
  const sectionUrl = query ? `${pathname}?${query}` : pathname
  const activeItem = matchSettingsNavItem(sectionUrl)
  const searching = pathname === '/settings/search'
  const [searchOpen, setSearchOpen] = useState(searching)
  const focusSearchRef = useRef(false)
  useEffect(() => {
    if (searching) setSearchOpen(true)
  }, [searching])
  useEffect(() => {
    if (!searchOpen || !focusSearchRef.current) return
    focusSearchRef.current = false
    document.querySelector<HTMLInputElement>('[data-ui="settings.navigation"] input')?.focus()
  }, [searchOpen])
  const openSearch = useCallback(() => {
    const field = document.querySelector<HTMLInputElement>('[data-ui="settings.navigation"] input')
    if (field) {
      field.focus()
      if (typeof field.select === 'function') field.select()
      return
    }
    focusSearchRef.current = true
    setSearchOpen(true)
  }, [])
  useSettingsPageKeyboard({ leave: leaveSettings, openSearch })

  const pathMatches = (path: string) => pathname === path || pathname.startsWith(`${path}/`)
  const isActive = (item: SettingsNavItem) => {
    if (!pathMatches(item.to)) return false
    if (item.panel) return panel === item.panel
    if (item.section) return section === item.section
    return !settingsNav.some(
      (other) =>
        other.to === item.to && ((other.section && other.section === section) || (other.panel && other.panel === panel))
    )
  }
  const openNavItem = (item: SettingsNavItem) => {
    if (item.panel) {
      void navigate({ to: item.to, search: { panel: item.panel } })
      return
    }
    if (item.section) {
      void navigate({
        to: item.to,
        search: { section: item.section, ...(item.focusId ? { focusId: item.focusId } : {}) }
      })
      return
    }
    void navigate({ to: item.to, search: {} })
  }

  const openExternalLink = (event: MouseEvent<HTMLDivElement>) => {
    if (event.type === 'auxclick' && event.button !== 1) return
    const anchor = (event.target as Element).closest<HTMLAnchorElement>('a[href]')
    if (!anchor || !/^https?:\/\//i.test(anchor.getAttribute('href') ?? '')) return
    event.preventDefault()
    event.stopPropagation()
    void openExternalWebsite(anchor.href)
  }

  return (
    <SettingsSearchDomIdsProvider>
      <div
        data-ui="settings.view"
        onClickCapture={openExternalLink}
        onAuxClickCapture={openExternalLink}
        className="flex min-h-0 flex-1 flex-col bg-white text-[#171717]">
        <div className="flex min-h-0 flex-1 flex-row">
          <div
            data-ui="settings.navigation"
            className={`flex min-h-0 shrink-0 flex-col bg-[var(--settings-nav-background)] pt-3 ${pageSidebarEdgeClassName}`}
            style={{ width: PAGE_SIDEBAR_DEFAULT_WIDTH, minWidth: PAGE_SIDEBAR_DEFAULT_WIDTH }}>
            <div className="px-3">
              {searchOpen ? (
                <SettingsSearchBox autoFocus onCollapse={() => setSearchOpen(false)} />
              ) : (
                <div className="flex items-center gap-1">
                  <button
                    type="button"
                    data-ui="settings.back"
                    aria-label={t('settings.backToApp')}
                    onClick={leaveSettings}
                    className="flex h-9 min-w-0 flex-1 items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] text-[#3f3f3f] transition-colors outline-none hover:bg-[#f7f7f7] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
                    <ArrowLeft className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                    <span className="truncate">{t('settings.backToApp')}</span>
                  </button>
                  <button
                    type="button"
                    data-ui="settings.search"
                    aria-label={t('settings.search.placeholder')}
                    onClick={() => setSearchOpen(true)}
                    className="flex size-9 shrink-0 items-center justify-center rounded-[8px] text-[#3f3f3f] transition-colors outline-none hover:bg-[#f7f7f7] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10">
                    <Search className="size-4 text-current" strokeWidth={1.7} aria-hidden />
                  </button>
                </div>
              )}
            </div>
            <Scrollbar className="min-h-0 flex-1 select-none">
              <MenuList className={settingsSubmenuListClassName}>
                {settingsNav.map((item, index) => {
                  const startsNewGroup =
                    Boolean(item.groupKey) && (index === 0 || item.groupKey !== settingsNav[index - 1]?.groupKey)
                  return (
                    <Fragment key={item.id}>
                      {startsNewGroup && item.groupKey && (
                        <div className={settingsSubmenuSectionTitleClassName}>{t(item.groupKey)}</div>
                      )}
                      <MenuItem
                        className={settingsSubmenuItemClassName}
                        labelClassName={settingsSubmenuItemLabelClassName}
                        icon={item.icon}
                        label={t(item.titleKey)}
                        active={isActive(item)}
                        onClick={() => openNavItem(item)}
                      />
                    </Fragment>
                  )
                })}
              </MenuList>
            </Scrollbar>
          </div>
          <div className="flex h-full min-h-0 min-w-0 flex-1 flex-col bg-[var(--settings-content-background)]">
            {activeItem && activeItem.id !== 'about' && (
              <div
                data-ui="settings.section-title"
                className="flex shrink-0 flex-wrap items-start justify-between gap-3 px-6 pt-4 pb-2">
                <div className="min-w-0">
                  <h1 className={settingsContentHeaderTitleClassName}>{t(activeItem.titleKey)}</h1>
                  {activeItem.descriptionKey ? (
                    <p className={settingsContentHeaderDescriptionClassName}>{t(activeItem.descriptionKey)}</p>
                  ) : null}
                </div>
                <div className="ml-auto flex shrink-0 items-center gap-2" data-ui="settings.header-action" />
              </div>
            )}
            <div
              ref={contentRef}
              data-ui="settings.content"
              className="flex min-h-0 min-w-0 flex-1 overflow-hidden text-foreground">
              <Outlet />
              <SettingsFocusUrl />
              <SettingsFocusScroll scopeRef={contentRef} />
            </div>
          </div>
        </div>
      </div>
    </SettingsSearchDomIdsProvider>
  )
}

export default SettingsPage
