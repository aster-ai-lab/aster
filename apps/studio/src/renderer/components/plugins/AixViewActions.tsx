import { Button, ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@a3s-lab/ui'
import { type ReactNode, useLayoutEffect, useRef } from 'react'

import { loggerService } from '@logger'
import { useCommandRuntime, useCommandRuntimeRevision } from '@renderer/hooks/command'
import { type AixViewSessions, matchesWorkbenchCondition } from '@renderer/services/plugins'
import { extensionCommandId } from '@renderer/services/rendererCommandService'
import type { ExtensionCommandRegistration } from '@renderer/types/command'
import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import type { AixWorkbenchPackage, AixWorkbenchView } from '@shared/types/aixWorkbench'

import { usePluginContributionContext } from './PluginContributionContext'
import { usePluginScope } from './PluginScopeProvider'

const logger = loggerService.withContext('AixViewActions')

/** Placements share command behavior while retaining their own visibility checks. */
export function AixViewActions({
  item,
  view,
  context,
  session,
  children
}: {
  item: AixWorkbenchPackage
  view: AixWorkbenchView
  context: PluginActivationContext
  session: ReturnType<AixViewSessions['open']>
  children: ReactNode
}) {
  const runtime = useCommandRuntime()
  useCommandRuntimeRevision()
  const visibility = usePluginContributionContext(usePluginScope())
  const latestVisibility = useRef(visibility)
  latestVisibility.current = visibility
  const registrations = useRef<ExtensionCommandRegistration[]>([])
  const provider = session.commands
  const identity = (command: string) => extensionCommandId(`aix.${item.slug}.${view.id}`, session.documentId, command)

  useLayoutEffect(() => {
    if (!provider) return
    const owned: ExtensionCommandRegistration[] = []
    const unsubscribe = provider.subscribe(() => owned.forEach((registration) => registration.refresh()))
    try {
      for (const command of view.commands ?? [])
        owned.push(
          runtime.registerCommand(
            {
              id: extensionCommandId(`aix.${item.slug}.${view.id}`, session.documentId, command.id),
              title: command.title,
              category: item.slug,
              ...(command.icon ? { iconKey: command.icon } : {})
            },
            provider,
            {
              enabled: () =>
                provider.canExecute(command.id) &&
                matchesWorkbenchCondition(view.when, latestVisibility.current) &&
                matchesWorkbenchCondition(command.enablement, latestVisibility.current),
              execute: (signal) => provider.execute(command.id, signal)
            }
          )
        )
      registrations.current = owned
    } catch (error) {
      owned.forEach((registration) => registration.dispose())
      provider.close()
      logger.warn('AIX command registration refused', error as Error)
    }
    return () => {
      unsubscribe()
      owned.forEach((registration) => registration.dispose())
      registrations.current = []
    }
  }, [runtime, provider, item.slug, session.documentId, view])

  useLayoutEffect(() => {
    registrations.current.forEach((registration) => registration.refresh())
  }, [visibility])

  const menus = [...(view.menus ?? [])]
    .filter((menu) => matchesWorkbenchCondition(menu.when, visibility))
    .sort(
      (left, right) =>
        left.order - right.order || (left.command < right.command ? -1 : left.command > right.command ? 1 : 0)
    )
    .map((menu) => {
      const command = runtime.resolveCommand(identity(menu.command))
      return {
        ...menu,
        title: command?.title ?? view.commands?.find((item) => item.id === menu.command)?.title ?? menu.command,
        enabled: command?.enabled ?? false,
        execute: () => {
          if (
            context.isCurrent() &&
            provider?.isCurrent() &&
            matchesWorkbenchCondition(menu.when, latestVisibility.current)
          )
            void command?.execute()
        }
      }
    })
  const titleActions = menus.filter((menu) => menu.location === 'view.title')
  const contextActions = menus.filter((menu) => menu.location === 'view.context')
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild disabled={!contextActions.length}>
        <div className="flex min-h-0 flex-1 flex-col">
          <div role="toolbar" aria-label={view.title || view.id} className="flex items-center gap-1 px-2 py-1">
            <h2 className="flex-1 text-xs text-muted-foreground">{view.title || view.id}</h2>
            {titleActions.map((menu) => (
              <Button key={menu.command} variant="ghost" size="sm" disabled={!menu.enabled} onClick={menu.execute}>
                {menu.title}
              </Button>
            ))}
          </div>
          {children}
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent>
        {contextActions.map((menu) => (
          <ContextMenuItem key={menu.command} disabled={!menu.enabled} onSelect={menu.execute}>
            {menu.title}
          </ContextMenuItem>
        ))}
      </ContextMenuContent>
    </ContextMenu>
  )
}
