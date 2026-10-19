import { useEffect, useMemo, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import {
  AppletLaunchpad,
  appletGlyph,
  INBOX_APPLET_ID,
  launchpadMotionReduced,
  type LaunchpadApplet
} from '@renderer/pages/agents/WorkbenchApplets'

import {
  DEFAULT_ORG_FAVORITES,
  isOrgHomeApp,
  openOrgHomeApp,
  ORG_APPLICATIONS,
  readOrgFavorites,
  subscribeOrgFavorites,
  writeOrgFavorites,
  type OrgHomeApp
} from './orgWorkbenchCatalog'

export type LaunchpadPhase = 'closed' | 'open' | 'closing'

export function closeAppletLaunchpad(current: LaunchpadPhase): LaunchpadPhase {
  if (current !== 'open') return current
  return launchpadMotionReduced() ? 'closed' : 'closing'
}

/** Fullscreen mini-app board shared by the team workbench and the activity-bar button. */
export function OrgAppletLaunchpad({
  orgId,
  phase,
  origin,
  onClose,
  onExited
}: {
  orgId: string
  phase: LaunchpadPhase
  origin: string
  onClose: () => void
  onExited: () => void
}) {
  const { t } = useTranslation()
  const favorites = useSyncExternalStore(
    subscribeOrgFavorites,
    () => readOrgFavorites(orgId),
    () => DEFAULT_ORG_FAVORITES
  )
  const [query, setQuery] = useState('')
  const catalog = useMemo<OrgHomeApp[]>(() => [INBOX_APPLET_ID, ...ORG_APPLICATIONS], [])
  const rows = useMemo<LaunchpadApplet[]>(() => {
    const needle = query.trim().toLocaleLowerCase()
    const ordered = [...favorites, ...catalog.filter((id) => !favorites.includes(id))]
    return ordered.flatMap((id) => {
      const label = id === INBOX_APPLET_ID ? t('work.inbox.box.todo') : t(`workbench.applet.${id}`)
      if (needle && !`${label} ${id}`.toLocaleLowerCase().includes(needle)) return []
      return [{ id, Icon: appletGlyph(id), label }]
    })
  }, [catalog, favorites, query, t])

  useEffect(() => {
    if (phase === 'closed') setQuery('')
  }, [phase])

  if (phase === 'closed') return null
  return (
    <AppletLaunchpad
      phase={phase}
      origin={origin}
      rows={rows}
      pins={favorites}
      query={query}
      catalogReady
      pinLimit={catalog.length}
      onQuery={setQuery}
      onClose={onClose}
      onExited={onExited}
      onOpen={(id) => {
        if (!isOrgHomeApp(id)) return
        const title = id === INBOX_APPLET_ID ? t('work.inbox.box.todo') : t(`workbench.applet.${id}`)
        onClose()
        void openOrgHomeApp(id, title, t)
      }}
      onToggle={(id) => {
        if (!isOrgHomeApp(id)) return
        if (favorites.includes(id)) {
          writeOrgFavorites(
            orgId,
            favorites.filter((item) => item !== id)
          )
          return
        }
        if (favorites.length >= catalog.length) return
        writeOrgFavorites(orgId, [...favorites, id])
      }}
      onReorder={(next) => {
        const ids = next.filter(isOrgHomeApp)
        if (ids.length === next.length) writeOrgFavorites(orgId, ids)
      }}
    />
  )
}
