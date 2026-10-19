import { Button } from '@a3s-lab/ui'
import { X } from 'lucide-react'
import { createContext, type ReactNode, use, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { usePluginContributions } from './PluginScopeProvider'
import { PluginSlot } from './PluginSlot'

interface ContainerSelection {
  readonly active: string | null
  reveal(key: string): void
}
const ContainerContext = createContext<ContainerSelection | null>(null)

export function PluginWorkbenchLayoutProvider({ children }: { children: ReactNode }) {
  const [active, setActive] = useState<string | null>(null)
  const sidebars = usePluginContributions('sidebar')
  useEffect(() => {
    if (active && !sidebars.some((item) => item.key === active && item.context.isCurrent())) setActive(null)
  }, [active, sidebars])
  const value = useMemo(
    () => ({ active, reveal: (key: string) => setActive((previous) => (previous === key ? null : key)) }),
    [active]
  )
  return <ContainerContext value={value}>{children}</ContainerContext>
}

export function usePluginContainerSelection() {
  return use(ContainerContext)
}

export function PluginWorkbenchSidebar() {
  const { t } = useTranslation()
  const selection = usePluginContainerSelection()
  const sidebars = usePluginContributions('sidebar')
  const current = sidebars.find((item) => item.key === selection?.active && item.context.isCurrent())
  if (!current) return null
  return (
    <aside
      aria-label={current.contribution.title ?? t(current.contribution.labelKey)}
      className="flex w-72 shrink-0 flex-col overflow-hidden border-r border-border bg-background">
      <header className="flex items-center justify-between border-b border-border p-2 text-sm">
        <span>{current.contribution.title ?? t(current.contribution.labelKey)}</span>
        <Button variant="ghost" size="sm" onClick={() => selection?.reveal(current.key)} aria-label={t('common.close')}>
          <X size={14} />
        </Button>
      </header>
      <div className="flex min-h-0 flex-1 flex-col overflow-auto">
        <PluginSlot slot="sidebar" contributionKey={current.key} />
      </div>
    </aside>
  )
}

export function PluginWorkbenchPanels() {
  const { t } = useTranslation()
  const panels = usePluginContributions('panel')
  const [selected, setSelected] = useState<string | null>(null)
  useEffect(() => {
    if (selected && !panels.some((item) => item.key === selected && item.context.isCurrent())) setSelected(null)
  }, [selected, panels])
  const current = panels.find((item) => item.key === selected && item.context.isCurrent())
  if (!panels.length) return null
  return (
    <section aria-label={t('plugins.title')} className="shrink-0 border-t border-border bg-background">
      <div className="flex gap-1 p-1" role="toolbar" aria-label={t('plugins.title')}>
        {panels.map((item) => (
          <Button
            key={item.key}
            variant="ghost"
            size="sm"
            aria-pressed={current?.key === item.key}
            onClick={() => setSelected((previous) => (previous === item.key ? null : item.key))}>
            {item.contribution.title ?? t(item.contribution.labelKey)}
          </Button>
        ))}
      </div>
      {current && (
        <div className="flex h-64 min-h-0 flex-col overflow-auto">
          <PluginSlot slot="panel" contributionKey={current.key} />
        </div>
      )}
    </section>
  )
}
