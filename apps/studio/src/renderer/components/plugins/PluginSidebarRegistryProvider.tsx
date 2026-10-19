import { type ReactNode, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import { useTabs } from '@renderer/hooks/tab'
import { DATA_ACTIVITY_PLUGIN_ID } from '@renderer/pages/data/dataPackageCatalogue'
import type { WorkbenchPlugin } from '@renderer/types/workbenchPlugin'
import { createSidebarShortcutTarget, SIDEBAR_SHORTCUT_PROVIDER_IDS } from '@renderer/utils/sidebar'
import {
  createSidebarShortcutId,
  type SidebarShortcutItem,
  type SidebarShortcutTarget
} from '@shared/data/preference/preferenceTypes'

import {
  SidebarShortcutRegistry,
  SidebarShortcutRegistryProvider,
  type SidebarShortcutProvider
} from '../app/sidebarShortcuts'
import { usePluginContributionContext } from './PluginContributionContext'
import { usePluginContributions, usePluginScope, usePluginScopeSnapshot } from './PluginScopeProvider'
import { createPluginSidebarProviders, PLUGIN_SIDEBAR_PROVIDER_ID } from './pluginSidebarProviders'
import { usePluginContainerSelection } from './PluginWorkbenchLayout'

export function PluginSidebarRegistryProvider({
  providers,
  builtins,
  children
}: {
  providers: readonly SidebarShortcutProvider[]
  builtins: readonly WorkbenchPlugin[]
  children: ReactNode
}) {
  const { t } = useTranslation()
  const scope = usePluginScope()
  const containers = usePluginContainerSelection()
  const snapshot = usePluginScopeSnapshot()
  const visibility = usePluginContributionContext(scope)
  const { activeTab } = useTabs()
  const context = useRef(visibility)
  const workspaceUrl = useRef(activeTab?.url)
  workspaceUrl.current = activeTab?.url
  const registry = useMemo(() => {
    context.current = visibility
    return new SidebarShortcutRegistry(
      createPluginSidebarProviders(
        providers,
        builtins,
        () => scope?.getSnapshot() ?? snapshot,
        t,
        containers,
        () => context.current,
        () => workspaceUrl.current
      )
    )
  }, [providers, builtins, scope, snapshot, t, containers, visibility])
  return <SidebarShortcutRegistryProvider registry={registry}>{children}</SidebarShortcutRegistryProvider>
}

function activityBarChoice(choices: Readonly<Record<string, boolean>> | null | undefined, pluginId: string): boolean {
  return choices?.[pluginId] === true
}

const KNOWLEDGE_ACTIVITY_ID = createSidebarShortcutId(
  createSidebarShortcutTarget(SIDEBAR_SHORTCUT_PROVIDER_IDS.APP, 'knowledge')
)

function insertAfterKnowledge(
  items: readonly SidebarShortcutItem[],
  extra: readonly SidebarShortcutItem[]
): SidebarShortcutItem[] {
  if (extra.length === 0) return [...items]
  const index = items.findIndex((item) => item.id === KNOWLEDGE_ACTIVITY_ID)
  if (index < 0) return [...items, ...extra]
  return [...items.slice(0, index + 1), ...extra, ...items.slice(index + 1)]
}

export function usePluginActivityShortcuts(configured: readonly SidebarShortcutItem[]): readonly SidebarShortcutItem[] {
  const activities = usePluginContributions('activity')
  const [choices] = usePreference('plugins.activity_bar')
  const { t } = useTranslation()
  return useMemo(() => {
    const retained = configured.filter((item) => {
      if (item.target.locator.providerId !== PLUGIN_SIDEBAR_PROVIDER_ID) return true
      const pluginId = item.target.locator.resourceId.split('/')[0]
      return choices?.[pluginId] !== false
    })
    const ids = new Set(retained.map((item) => item.id))
    const added = activities.flatMap((item): SidebarShortcutItem[] => {
      if (item.contribution.slot !== 'activity' || item.contribution.applicationId) return []
      if (!activityBarChoice(choices, item.pluginId)) return []
      const target: SidebarShortcutTarget = {
        kind: 'resource',
        locator: { providerId: PLUGIN_SIDEBAR_PROVIDER_ID, resourceId: item.key }
      }
      const id = createSidebarShortcutId(target)
      if (ids.has(id)) return []
      ids.add(id)
      return [{ type: 'shortcut', id, target, fallbackLabel: item.contribution.title ?? t(item.contribution.labelKey) }]
    })
    const data = added.filter((item) => item.target.locator.resourceId.startsWith(`${DATA_ACTIVITY_PLUGIN_ID}/`))
    const rest = added.filter((item) => !item.target.locator.resourceId.startsWith(`${DATA_ACTIVITY_PLUGIN_ID}/`))
    const withData = insertAfterKnowledge(retained, data)
    return rest.length ? [...withData, ...rest] : withData
  }, [configured, activities, choices, t])
}
