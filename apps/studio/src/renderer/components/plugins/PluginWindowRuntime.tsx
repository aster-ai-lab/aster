import { type ReactNode, useEffect, useRef, useState } from 'react'

import { loggerService } from '@logger'
import { WorkbenchPluginHost, type WorkbenchPluginScope } from '@renderer/services/plugins'
import type { WorkbenchPlugin } from '@renderer/types/workbenchPlugin'

import { PluginContributionContextProvider } from './PluginContributionContext'
import { PluginScopeProvider } from './PluginScopeProvider'

const logger = loggerService.withContext('PluginWindowRuntime')

interface WindowPluginOwner {
  readonly host: WorkbenchPluginHost
  opening?: Promise<WorkbenchPluginScope>
}

export function PluginWindowRuntime({
  namespace,
  windowId,
  plugins,
  children
}: {
  namespace: string
  windowId: string
  plugins: readonly WorkbenchPlugin[]
  children: ReactNode
}) {
  const [scope, setScope] = useState<WorkbenchPluginScope | null>(null)
  const owner = useRef<WindowPluginOwner | null>(null)
  useEffect(() => {
    const host = new WorkbenchPluginHost(namespace)
    owner.current = { host }
    return () => {
      if (owner.current?.host === host) owner.current = null
      void host.close().then((report) => {
        if (report.failed.length || report.timedOut.length) logger.error('Plugin window cleanup incomplete', report)
      })
    }
  }, [namespace, windowId])

  useEffect(() => {
    const runtime = owner.current
    if (!runtime) return undefined
    let disposed = false
    void runtime.host
      .setPlugins(plugins)
      .then(() => {
        runtime.opening ??= runtime.host.openScope({ id: windowId, kind: 'window' }).catch((error: unknown) => {
          runtime.opening = undefined
          throw error
        })
        return runtime.opening
      })
      .then((next) => {
        if (!disposed && owner.current === runtime) setScope(next)
      })
      .catch((error: unknown) => {
        if (!disposed)
          logger.error('Plugin window activation failed', error instanceof Error ? error : new Error(String(error)))
      })
    return () => {
      disposed = true
    }
  }, [namespace, plugins, windowId])

  const current =
    scope?.host === owner.current?.host &&
    scope?.host.tenantId === namespace &&
    scope?.options.id === windowId &&
    scope?.isOpen()
      ? scope
      : null
  return (
    <PluginContributionContextProvider>
      <PluginScopeProvider scope={current}>{children}</PluginScopeProvider>
    </PluginContributionContextProvider>
  )
}
