import { type ReactNode, useEffect, useId, useMemo, useRef, useState } from 'react'

import { loggerService } from '@logger'
import type { OpenPluginScope, WorkbenchPluginScope } from '@renderer/services/plugins'
import type { PluginScopeKind, PluginServiceSnapshot } from '@renderer/types/workbenchPlugin'

import { PluginScopeProvider, usePluginScope, usePluginScopeSnapshot } from './PluginScopeProvider'

const logger = loggerService.withContext('PluginScopeRuntime')

/** Owns one UI child scope. Native Code runs require their own admitted owner. */
export function PluginScopeRuntime({
  kind,
  services,
  plugins,
  enabled = true,
  children
}: {
  kind: Exclude<PluginScopeKind, 'window'>
  services?: PluginServiceSnapshot
  plugins?: readonly string[]
  enabled?: boolean
  children: ReactNode
}) {
  const parent = usePluginScope()
  usePluginScopeSnapshot()
  const activeParent = enabled && parent?.isOpen() ? parent : null
  const ownerId = useId().replace(/[^a-zA-Z0-9_.-]/g, '_')
  const sequence = useRef(0)
  const ownerKey = JSON.stringify({
    services: {
      workspace: services?.workspace && { id: services.workspace.id, root: services.workspace.root },
      document: services?.document && { uri: services.document.uri, version: services.document.version },
      conversation: services?.conversation && { sessionId: services.conversation.sessionId },
      run: services?.run && {
        coreSessionId: services.run.coreSessionId,
        runId: services.run.runId,
        promptId: services.run.promptId
      },
      turn: services?.turn && { modelTurn: services.turn.modelTurn },
      harness: services?.harness && { components: services.harness.components }
    },
    plugins: plugins && [...plugins].sort()
  })
  const captured = useMemo<Pick<OpenPluginScope, 'services' | 'plugins'>>(() => JSON.parse(ownerKey), [ownerKey])
  const [owned, setOwned] = useState<{
    scope: WorkbenchPluginScope
    parent: WorkbenchPluginScope
    ownerKey: string
  } | null>(null)

  useEffect(() => {
    if (!activeParent) return undefined
    const cancellation = new globalThis.AbortController()
    let scope: WorkbenchPluginScope | undefined
    const close = (target: WorkbenchPluginScope) => {
      void target.close().then((report) => {
        if (report.failed.length || report.timedOut.length) logger.error('Plugin child cleanup incomplete', report)
      })
    }
    void activeParent.host
      .openScope({
        id: `owner-${ownerId}-${++sequence.current}`,
        kind,
        parent: activeParent,
        ...captured,
        signal: cancellation.signal
      })
      .then((next) => {
        scope = next
        if (cancellation.signal.aborted) close(next)
        else setOwned({ scope: next, parent: activeParent, ownerKey })
      })
      .catch((error: unknown) => {
        if (!cancellation.signal.aborted)
          logger.error('Plugin child activation failed', error instanceof Error ? error : new Error(String(error)))
      })
    return () => {
      cancellation.abort()
      if (scope) close(scope)
    }
  }, [activeParent, captured, kind, ownerId, ownerKey])

  const current =
    owned?.parent === activeParent &&
    owned?.ownerKey === ownerKey &&
    owned.scope.options.kind === kind &&
    owned.scope.isOpen()
      ? owned.scope
      : null
  const inherited = (!enabled || kind === 'run' || kind === 'turn') && parent?.isOpen() ? parent : null
  return <PluginScopeProvider scope={current ?? inherited}>{children}</PluginScopeProvider>
}
