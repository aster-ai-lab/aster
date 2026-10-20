import { Check, Circle, CircleAlert, Loader2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import type { TaskPlanStep } from './taskPlanProjection'

const STATUS_KEY: Record<TaskPlanStep['status'], string> = {
  pending: 'workbench.run.plan.pending',
  in_progress: 'workbench.run.plan.in_progress',
  completed: 'workbench.run.plan.completed',
  failed: 'workbench.run.plan.failed',
  skipped: 'workbench.run.plan.skipped',
  cancelled: 'workbench.run.plan.cancelled'
}

/** A historical plan snapshot stays in its assistant turn after composer controls clear. */
export function TaskPlanSnapshot({ steps }: { steps: readonly TaskPlanStep[] }) {
  const { t } = useTranslation()
  return (
    <section
      data-task-plan-snapshot=""
      aria-label={t('workbench.run.plan.title')}
      className="my-2 rounded-lg border border-border px-3 py-2">
      <p className="mb-1 text-[13px] font-medium">{t('workbench.run.plan.title')}</p>
      <ol className="space-y-1">
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
                className={step.status === 'in_progress' ? 'mt-0.5 shrink-0 animate-spin' : 'mt-0.5 shrink-0'}
              />
              <span className="min-w-0 flex-1 break-words">{step.title}</span>
              <span className="shrink-0 text-xs text-muted-foreground">{t(STATUS_KEY[step.status])}</span>
            </li>
          )
        })}
      </ol>
    </section>
  )
}
