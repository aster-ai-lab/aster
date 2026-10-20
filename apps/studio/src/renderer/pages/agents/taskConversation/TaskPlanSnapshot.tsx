import { Button } from '@a3s-lab/ui'
import { Check, ChevronDown, Circle, CircleAlert, ListChecks, Loader2 } from 'lucide-react'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

import type { TaskPlanStep } from './taskPlanProjection'

const STATUS_KEY: Record<TaskPlanStep['status'], string> = {
  pending: 'workbench.run.plan.pending',
  in_progress: 'workbench.run.plan.in_progress',
  completed: 'workbench.run.plan.completed',
  failed: 'workbench.run.plan.failed',
  skipped: 'workbench.run.plan.skipped',
  cancelled: 'workbench.run.plan.cancelled'
}

const STATUS_TONE: Record<TaskPlanStep['status'], string> = {
  pending: 'text-muted-foreground',
  in_progress: 'text-warning-subtle-foreground',
  completed: 'text-success-subtle-foreground',
  failed: 'text-error-subtle-foreground',
  skipped: 'text-muted-foreground',
  cancelled: 'text-warning-subtle-foreground'
}

/** A historical plan snapshot stays in its assistant turn after composer controls clear. */
export function TaskPlanSnapshot({ steps }: { steps: readonly TaskPlanStep[] }) {
  const { t } = useTranslation()
  const listId = useId()
  const [userExpanded, setUserExpanded] = useState<boolean | null>(null)
  const completed = steps.filter((step) => step.status === 'completed').length
  const expanded = userExpanded ?? completed !== steps.length
  const progress = t('agent.right_pane.status.task_count', { completed, total: steps.length })
  return (
    <section
      data-task-plan-snapshot=""
      aria-label={t('workbench.run.plan.title')}
      className="my-2 rounded-lg border border-border px-3 py-1">
      <Button
        variant="ghost"
        className="h-7 w-full justify-start gap-2 px-0 text-start hover:bg-transparent"
        aria-label={t('workbench.run.plan.toggle')}
        aria-expanded={expanded}
        aria-controls={listId}
        onClick={() => setUserExpanded(!expanded)}>
        <ListChecks size={14} aria-hidden />
        <span className="min-w-0 flex-1 text-[13px] font-medium">{t('workbench.run.plan.title')}</span>
        <span className="shrink-0 text-xs font-normal text-muted-foreground tabular-nums" aria-live="polite">
          {progress}
        </span>
        <ChevronDown size={14} aria-hidden className={cn('shrink-0', expanded && 'rotate-180')} />
      </Button>
      <ol id={listId} hidden={!expanded} className="space-y-1 pb-1">
        {steps.map((step) => {
          const Icon =
            step.status === 'completed'
              ? Check
              : step.status === 'in_progress'
                ? Loader2
                : step.status === 'failed'
                  ? CircleAlert
                  : Circle
          return (
            <li key={step.id} data-task-plan-status={step.status} className="flex items-start gap-2 text-[13px]">
              <Icon
                size={14}
                aria-hidden
                className={cn(
                  'mt-0.5 shrink-0',
                  STATUS_TONE[step.status],
                  step.status === 'in_progress' && 'animate-spin'
                )}
              />
              <span className="min-w-0 flex-1 break-words">{step.title}</span>
              <span className={cn('shrink-0 text-xs', STATUS_TONE[step.status])}>{t(STATUS_KEY[step.status])}</span>
            </li>
          )
        })}
      </ol>
    </section>
  )
}
