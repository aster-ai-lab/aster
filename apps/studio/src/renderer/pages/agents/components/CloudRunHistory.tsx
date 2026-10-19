import { useCallback, useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { cloudListRuns, type AixCloudRunSummary } from '../aixAgentRuntime'

/** One row in the cloud run history list. */
function RunRow({ run }: { run: AixCloudRunSummary }) {
  const statusColor =
    run.status === 'completed'
      ? 'text-green-600 dark:text-green-400'
      : run.status === 'failed' || run.status === 'cancelled'
        ? 'text-red-600 dark:text-red-400'
        : 'text-blue-600 dark:text-blue-400'
  return (
    <div className="flex items-center justify-between gap-2 rounded-md border px-3 py-2 text-sm">
      <div className="flex min-w-0 items-center gap-2">
        <span className={`shrink-0 font-medium ${statusColor}`}>{run.status}</span>
        <span className="truncate text-muted-foreground">
          {run.fullyBound ? 'bound' : 'binding…'}
        </span>
      </div>
      <span className="shrink-0 font-mono text-xs text-muted-foreground">
        {run.agentExecutionId.slice(0, 8)}…
      </span>
    </div>
  )
}

/** Cloud agent run history for one conversation. */
export function CloudRunHistory({
  conversationId,
  cloudEndpoint,
  bearerToken
}: {
  conversationId: string
  cloudEndpoint: string
  bearerToken?: string
}) {
  const { t } = useTranslation()
  const [runs, setRuns] = useState<AixCloudRunSummary[]>([])
  const [state, setState] = useState<'loading' | 'ready' | 'error'>('loading')

  const refresh = useCallback(async () => {
    if (!conversationId || !cloudEndpoint) return
    try {
      const fetchImpl = async (
        url: string,
        init: { method: string; headers?: Record<string, string> }
      ) => fetch(url, init)
      const result = await cloudListRuns(
        { cloud: cloudEndpoint },
        fetchImpl,
        conversationId,
        bearerToken
      )
      setRuns(result)
      setState('ready')
    } catch {
      setState('error')
    }
  }, [conversationId, cloudEndpoint, bearerToken])

  useEffect(() => {
    void refresh()
    const interval = setInterval(() => void refresh(), 10_000)
    return () => clearInterval(interval)
  }, [refresh])

  if (state === 'loading') {
    return (
      <p className="px-2 py-1 text-sm text-muted-foreground">
        {t('agents.cloudRunHistory.loading')}
      </p>
    )
  }
  if (state === 'error') {
    return (
      <p className="px-2 py-1 text-sm text-muted-foreground">
        {t('agents.cloudRunHistory.error')}
      </p>
    )
  }
  if (runs.length === 0) {
    return (
      <p className="px-2 py-1 text-sm text-muted-foreground">
        {t('agents.cloudRunHistory.empty')}
      </p>
    )
  }

  return (
    <div className="flex flex-col gap-1">
      <p className="px-2 text-xs font-medium text-muted-foreground">
        {t('agents.cloudRunHistory.title')} ({runs.length})
      </p>
      {runs.map((run: AixCloudRunSummary) => (
        <RunRow key={run.agentExecutionId} run={run} />
      ))}
    </div>
  )
}
