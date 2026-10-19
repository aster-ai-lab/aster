import { Package } from 'lucide-react'

import { matchesWorkbenchCondition, type WorkbenchContributionContext } from '@renderer/services/plugins'
import type { PluginScopeSnapshot, WorkbenchPlugin } from '@renderer/types/workbenchPlugin'
import { isPluginPageKey, pluginPageUrl } from '@renderer/utils/pluginNavigation'
import { SIDEBAR_SHORTCUT_PROVIDER_IDS } from '@renderer/utils/sidebar'
import { createSidebarShortcutId, type SidebarShortcutTarget } from '@shared/data/preference/preferenceTypes'

import type { ResolvedShortcut, SidebarShortcutProvider } from '../app/sidebarShortcuts'

export const PLUGIN_SIDEBAR_PROVIDER_ID = 'studio.plugin-page'

function isPluginContributionPage(url: string, pageKey: string): boolean {
  try {
    const parsed = new globalThis.URL(url, 'http://studio.invalid')
    return parsed.pathname === '/app/plugin' && parsed.searchParams.get('page') === pageKey
  } catch {
    return false
  }
}

export function createPluginSidebarProviders(
  base: readonly SidebarShortcutProvider[],
  builtins: readonly WorkbenchPlugin[],
  snapshot: () => PluginScopeSnapshot,
  translate: (key: string) => string,
  containers?: { readonly active: string | null; reveal(key: string): void } | null,
  conditionContext?: () => WorkbenchContributionContext,
  workspaceUrl?: () => string | undefined
): readonly SidebarShortcutProvider[] {
  const claimed = new Set<string>(
    builtins.flatMap((plugin) =>
      plugin.contributions.flatMap((item) =>
        item.slot === 'activity' && item.applicationId ? [item.applicationId] : []
      )
    )
  )
  const activities = () =>
    snapshot().contributions.filter(
      (item) =>
        (item.contribution.slot === 'activity' || item.contribution.slot === 'applet') &&
        item.context.isCurrent() &&
        (!item.contribution.when ||
          (!!conditionContext && matchesWorkbenchCondition(item.contribution.when, conditionContext())))
    )
  const find = (target: SidebarShortcutTarget, application = false) =>
    activities().find(
      (item) =>
        (item.contribution.slot === 'activity' || item.contribution.slot === 'applet') &&
        (application
          ? item.contribution.applicationId === target.locator.resourceId
          : item.contribution.applicationId === undefined && item.key === target.locator.resourceId)
    )
  const resolved = (item: NonNullable<ReturnType<typeof find>>) => {
    const Icon =
      item.contribution.slot === 'activity' || item.contribution.slot === 'applet'
        ? (item.contribution.icon ?? Package)
        : Package
    return {
      label: item.contribution.title ?? translate(item.contribution.labelKey),
      renderIcon: ({ glyphSize }: { glyphSize: number }) => <Icon size={glyphSize} strokeWidth={1.6} />,
      supportsNewTab:
        (item.contribution.slot === 'activity' || item.contribution.slot === 'applet') && !item.contribution.sidebarId
    }
  }
  const dynamic: SidebarShortcutProvider = {
    id: PLUGIN_SIDEBAR_PROVIDER_ID,
    validate: (target) =>
      target.kind === 'resource' &&
      target.locator.providerId === PLUGIN_SIDEBAR_PROVIDER_ID &&
      isPluginPageKey(target.locator.resourceId) &&
      (target.activationId === undefined || target.activationId === 'reveal'),
    async resolveMany(targets) {
      const result = new Map<string, ResolvedShortcut>()
      for (const target of targets) {
        if (!this.validate(target)) continue
        const item = find(target)
        if (item) result.set(createSidebarShortcutId(target), resolved(item))
      }
      return result
    },
    activate(target, gateway) {
      if (!this.validate(target)) return
      const item = find(target)
      if (!item || (item.contribution.slot !== 'activity' && item.contribution.slot !== 'applet')) return
      const pageKey = `${item.pluginId}/${item.contribution.pageId}`
      const title = item.contribution.title ?? translate(item.contribution.labelKey)
      const openPage = () =>
        gateway.openWorkspace({
          url: pluginPageUrl(pageKey),
          title,
          matchesCurrent: (current) => !!this.isActive?.(target, { url: current })
        })
      if (item.contribution.sidebarId && containers) {
        const sidebarKey = `${item.pluginId}/${item.contribution.sidebarId}`
        const current = workspaceUrl?.()
        // Callers without a workspace URL dock the rail beside an editor. A real page opens the
        // plugin across the content column. A 18rem rail leaves the previous page beside it.
        if (current === undefined) {
          containers.reveal(sidebarKey)
          return
        }
        if (containers.active === sidebarKey) containers.reveal(sidebarKey)
        if (!isPluginContributionPage(current, pageKey)) openPage()
        return
      }
      openPage()
    },
    isActive(target, navigation) {
      const item = this.validate(target) ? find(target) : undefined
      if (!item || (item.contribution.slot !== 'activity' && item.contribution.slot !== 'applet')) return false
      const pageKey = `${item.pluginId}/${item.contribution.pageId}`
      if (item.contribution.sidebarId && containers?.active === `${item.pluginId}/${item.contribution.sidebarId}`)
        return true
      return isPluginContributionPage(navigation.url, pageKey)
    }
  }
  return [
    ...base.map(
      (provider): SidebarShortcutProvider =>
        provider.id !== SIDEBAR_SHORTCUT_PROVIDER_IDS.APP
          ? provider
          : {
              ...provider,
              async resolveMany(targets) {
                const result = await provider.resolveMany(
                  targets.filter((target) => !claimed.has(target.locator.resourceId))
                )
                for (const target of targets) {
                  if (!claimed.has(target.locator.resourceId) || !provider.validate(target)) continue
                  const item = find(target, true)
                  if (item) result.set(createSidebarShortcutId(target), resolved(item))
                }
                return result
              },
              activate(target, gateway) {
                if (!claimed.has(target.locator.resourceId)) return provider.activate(target, gateway)
                const item = provider.validate(target) ? find(target, true) : undefined
                if (!item) return undefined
                return provider.activate(target, {
                  openWorkspace: (destination) => {
                    if (find(target, true)?.context === item.context && item.context.isCurrent())
                      gateway.openWorkspace(destination)
                  }
                })
              },
              isActive(target, navigation) {
                const sidebarOpen = activities().some(
                  (item) =>
                    item.contribution.slot === 'activity' &&
                    item.contribution.sidebarId !== undefined &&
                    containers?.active === `${item.pluginId}/${item.contribution.sidebarId}`
                )
                if (sidebarOpen) return false
                return (
                  (!claimed.has(target.locator.resourceId) || !!find(target, true)) &&
                  !!provider.isActive?.(target, navigation)
                )
              }
            }
    ),
    dynamic
  ]
}
