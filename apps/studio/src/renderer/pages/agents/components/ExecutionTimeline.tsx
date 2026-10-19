import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

/** One step in an agent execution timeline. */
export type ExecutionStep = {
  sequence: number
  kind: 'requested' | 'started' | 'text_delta' | 'tool_call' | 'tool_result' | 'approval' | 'completed' | 'failed'
  label: string
  detail?: string
  timestamp?: string
}

/** Fetch the execution trajectory from the member plane. */
async function fetchTrajectory(
  executionId: string,
  origin: string
): Promise<ExecutionStep[]> {
  const response = await fetch(
    `${origin}/api/cloud/agent-runtimes/${executionId}/logs?limit=100`
  )
  if (!response.ok) return []
  const body = (await response.json()) as {
    data?: Array<{
      sequence?: number
      kind?: string
      content?: { text?: string; toolName?: string; status?: string }
      recordedAt?: string
    }>
  }
  return (body.data ?? []).map((event) => {
    const kind = (event.kind ?? 'unknown') as ExecutionStep['kind']
    const label =
      kind === 'tool_call'
        ? `Tool: ${event.content?.toolName ?? 'unknown'}`
        : kind === 'text_delta'
          ? (event.content?.text ?? '').slice(0, 80)
          : kind === 'approval'
            ? 'Approval checkpoint'
            : kind
    return {
      sequence: event.sequence ?? 0,
      kind,
      label,
      detail: event.content?.text,
      timestamp: event.recordedAt
    }
  })
}

/** Visual timeline of one cloud agent execution. */
export function ExecutionTimeline({
  executionId,
  apiOrigin
}: {
  executionId: string
  apiOrigin: string
}) {
  const { t } = useTranslation()
  const [steps, setSteps] = useState<ExecutionStep[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!executionId) return
    const load = () =>
      void fetchTrajectory(executionId, apiOrigin)
        .then(setSteps)
        .catch(() => setSteps([]))
        .finally(() => setLoading(false))
    load()
    const interval = setInterval(load, 5_000)
    return () => clearInterval(interval)
  }, [executionId, apiOrigin])

  if (loading) {
    return <p className="text-sm text-muted-foreground">{t('agents.executionTimeline.loading')}</p>
  }
  if (steps.length === 0) {
    return <p className="text-sm text-muted-foreground">{t('agents.executionTimeline.empty')}</p>
  }

  return (
    <div className="flex flex-col gap-0">
      <p className="mb-2 text-xs font-medium text-muted-foreground">
        {t('agents.executionTimeline.title')} ({steps.length} events)
      </p>
      <div className="relative border-l-2 border-border pl-4">
        {steps.map((step) => (
          <div key={step.sequence} className="relative mb-3">
            <span
              className={`absolute -left-[21px] top-1 h-3 w-3 rounded-full border-2 ${
                step.kind === 'completed'
                  ? 'border-green-500 bg-green-100 dark:bg-green-900'
                  : step.kind === 'failed'
                    ? 'border-red-500 bg-red-100 dark:bg-red-900'
                    : step.kind === 'tool_call' || step.kind === 'approval'
                      ? 'border-blue-500 bg-blue-100 dark:bg-blue-900'
                      : 'border-border bg-background'
              }`}
            />
            <p className="text-sm font-medium">{step.label}</p>
            {step.detail && step.kind !== 'text_delta' && (
              <p className="text-xs text-muted-foreground">{step.detail.slice(0, 120)}</p>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}
