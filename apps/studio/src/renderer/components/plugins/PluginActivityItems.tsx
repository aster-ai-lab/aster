import { Button } from '@a3s-lab/ui'
import { useTranslation } from 'react-i18next'

import { usePluginContributions } from './PluginScopeProvider'

export function PluginActivityItems({ onOpen, activePage }: { onOpen: (key: string) => void; activePage?: string }) {
  const { t } = useTranslation()
  return usePluginContributions('activity').map((item) => {
    const contribution = item.contribution
    if (contribution.slot !== 'activity') return null
    const key = `${item.pluginId}/${contribution.pageId}`
    return (
      <Button
        key={`${item.key}@${item.pluginGeneration}`}
        type="button"
        variant="ghost"
        size="sm"
        aria-pressed={activePage === key}
        onClick={() => {
          if (item.context.isCurrent()) onOpen(key)
        }}>
        {contribution.title ?? t(contribution.labelKey)}
      </Button>
    )
  })
}
