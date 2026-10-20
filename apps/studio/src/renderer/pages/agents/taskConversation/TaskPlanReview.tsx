import { Button } from '@a3s-lab/ui'
import { ListChecks, Play } from 'lucide-react'
import { useTranslation } from 'react-i18next'

export function TaskPlanReview({
  disabled,
  blockedReason,
  hasDraft,
  busy,
  onStart,
  onRevise
}: {
  disabled: boolean
  blockedReason?: string
  hasDraft: boolean
  busy: boolean
  onStart: () => void
  onRevise: () => void
}) {
  const { t } = useTranslation()
  return (
    <section
      aria-label={t('workbench.plan.ready')}
      className="mx-3 mb-2 flex flex-wrap items-center gap-3 border-b border-[#ececec] bg-white pt-1 pb-3">
      <ListChecks size={16} strokeWidth={1.7} aria-hidden className="text-[#8a8a8a]" />
      <div className="min-w-0 flex-1">
        <p role="status" className="text-[13px] font-medium text-[#171717]">
          {t('workbench.plan.ready')}
        </p>
        <p className="mt-0.5 text-[12px] text-[#8a8a8a]">
          {hasDraft ? t('workbench.plan.unsent_draft') : (blockedReason ?? t('workbench.plan.review_hint'))}
        </p>
      </div>
      <Button variant="ghost" size="sm" disabled={disabled || busy} onClick={onRevise}>
        {t('workbench.plan.revise')}
      </Button>
      <Button
        size="sm"
        className="gap-1.5 bg-[#171717] text-white hover:bg-[#3a3a3a]"
        disabled={disabled || hasDraft || busy}
        onClick={onStart}>
        <Play size={14} strokeWidth={1.7} aria-hidden />
        {t(busy ? 'workbench.plan.starting' : 'workbench.plan.start')}
      </Button>
    </section>
  )
}
