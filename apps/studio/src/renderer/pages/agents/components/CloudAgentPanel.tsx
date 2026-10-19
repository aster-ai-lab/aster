import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { cloudListRuns, type AixCloudRunSummary } from '../aixAgentRuntime'
import { CloudModelPicker } from './CloudModelPicker'

export type CloudAgentPanelRun = {
  executionId: string
  status: string
}

/**
 * Self-contained panel for cloud agent runs: model selection, run history,
 * and streaming text. Drop into any agent page layout that hosts cloud agents.
 */
export function CloudAgentPanel({
  conversationId,
  gatewayOrigin,
  cloudEndpoint,
  bearerToken,
  modelAlias,
  onModelAliasChange,
  streamingText,
  isStreaming
}: {
  conversationId: string
  gatewayOrigin: string
  cloudEndpoint: string
  bearerToken?: string
  modelAlias?: string
  onModelAliasChange?: (alias: string | undefined) => void
  streamingText?: string
  isStreaming?: boolean
}) {
  const { t } = useTranslation()
  const [runs, setRuns] = useState<AixCloudRunSummary[]>([])
  const textRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!conversationId || !cloudEndpoint) return
    const fetchImpl = async (
      url: string,
      init: { method: string; headers?: Record<string, string> }
    ) => fetch(url, init)
    const load = () =>
      void cloudListRuns({ cloud: cloudEndpoint }, fetchImpl, conversationId, bearerToken)
        .then(setRuns)
        .catch(() => setRuns([]))
    load()
    const interval = setInterval(load, 10_000)
    return () => clearInterval(interval)
  }, [conversationId, cloudEndpoint, bearerToken])

  useEffect(() => {
    if (textRef.current && isStreaming) {
      textRef.current.scrollTop = textRef.current.scrollHeight
    }
  }, [streamingText, isStreaming])

  return (
    <div className="flex h-full flex-col gap-3 p-2 text-sm">
      {onModelAliasChange && (
        <CloudModelPicker
          gatewayOrigin={gatewayOrigin}
          bearerToken={bearerToken}
          value={modelAlias}
          onChange={onModelAliasChange}
        />
      )}

      {streamingText !== undefined && (
        <div
          ref={textRef}
          className="min-h-[3rem] flex-1 overflow-auto rounded-md border px-3 py-2 whitespace-pre-wrap"
        >
          {streamingText}
          {isStreaming && (
            <span className="ml-0.5 inline-block h-4 w-1 animate-pulse bg-current align-text-bottom" />
          )}
        </div>
      )}

      <div className="shrink-0">
        <p className="mb-1 text-xs font-medium text-muted-foreground">
          {t('agents.cloudRunHistory.title')} ({runs.length})
        </p>
        <div className="flex flex-col gap-1">
          {runs.length === 0 ? (
            <p className="text-xs text-muted-foreground">{t('agents.cloudRunHistory.empty')}</p>
          ) : (
            runs.map((run) => (
              <div
                key={run.agentExecutionId}
                className="flex items-center justify-between gap-2 rounded-md border px-2 py-1 text-xs"
              >
                <span
                  className={
                    run.status === 'completed'
                      ? 'font-medium text-green-600 dark:text-green-400'
                      : run.status === 'failed' || run.status === 'cancelled'
                        ? 'font-medium text-red-600 dark:text-red-400'
                        : 'font-medium text-blue-600 dark:text-blue-400'
                  }
                >
                  {run.status}
                </span>
                <span className="text-muted-foreground">
                  {run.agentExecutionId.slice(0, 8)}…
                </span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  )
}
