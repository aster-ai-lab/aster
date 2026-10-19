import { Button } from '@a3s-lab/ui'
import { Package } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'

import { useTabs } from '@renderer/hooks/tab'
import { aixAppletForPage, openAixAppletWindow, readAixCatalogue } from '@renderer/services/plugins'
import type { PublishedPluginContribution } from '@renderer/types/workbenchPlugin'
import { pluginPageUrl } from '@renderer/utils/pluginNavigation'

import { usePluginContributions } from './PluginScopeProvider'

export function usePluginMiniApps(search = '') {
  const items = usePluginContributions('applet')
  const { t } = useTranslation()
  return useMemo(
    () =>
      items.filter((item) => {
        if (item.contribution.slot !== 'applet' || !item.context.isCurrent()) return false
        const query = search.trim().toLocaleLowerCase()
        return (
          !query ||
          `${item.contribution.title ?? t(item.contribution.labelKey)} ${item.contribution.description ?? ''}`
            .toLocaleLowerCase()
            .includes(query)
        )
      }),
    [items, search, t]
  )
}

/** An AIX mini app opens in its own system window; elsewhere, and outside the shell, it opens as a tab. */
async function openMiniApp(pluginId: string, pageId: string, title: string, openTab: (url: string) => void) {
  const applet = aixAppletForPage(readAixCatalogue(), pluginId, pageId)
  if (applet && (await openAixAppletWindow(applet.item.slug, applet.view.id, title))) return
  openTab(pluginPageUrl(`${pluginId}/${pageId}`))
}

export function PluginMiniAppItems({ items }: { items: readonly PublishedPluginContribution[] }) {
  const { openTab } = useTabs()
  const { t } = useTranslation()
  return items.map((item) => {
    const contribution = item.contribution
    if (contribution.slot !== 'applet') return null
    const Icon = contribution.icon ?? Package
    const title = contribution.title ?? t(contribution.labelKey)
    return (
      <Button
        key={item.key}
        type="button"
        variant="ghost"
        className="h-auto min-h-[104px] w-[92px] flex-col gap-2 px-0 pt-1"
        onClick={() => {
          if (!item.context.isCurrent()) return
          void openMiniApp(item.pluginId, contribution.pageId, title, (url) => openTab(url, { title }))
        }}
        title={contribution.description || title}>
        <span className="flex size-14 items-center justify-center rounded-[14px] border border-border bg-background">
          <Icon size={24} strokeWidth={1.7} />
        </span>
        <span className="max-w-[92px] text-center text-[13px] leading-[18px] whitespace-normal">{title}</span>
      </Button>
    )
  })
}
