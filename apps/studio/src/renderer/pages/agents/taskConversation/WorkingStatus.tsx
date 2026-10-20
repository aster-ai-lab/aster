import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

const VERBS = [
  'agent.working.checking',
  'agent.working.comparing',
  'agent.working.confirming',
  'agent.working.drafting',
  'agent.working.looking',
  'agent.working.mulling',
  'agent.working.organizing',
  'agent.working.polishing',
  'agent.working.preparing',
  'agent.working.reading',
  'agent.working.searching',
  'agent.working.sketching',
  'agent.working.sorting',
  'agent.working.summarizing',
  'agent.working.thinking',
  'agent.working.untangling',
  'agent.working.weighing',
  'agent.working.writing'
] as const

const HOLD_MS = 2800

/**
 * A live turn keeps one status row after the latest content. Finished tools
 * and a paragraph that looks complete are steps, not the end of the turn.
 * A form or an approval is waiting on the person, so that row stays quiet.
 */
export function turnShowsWorkingStatus(
  streaming: boolean,
  last: { kind: string; state?: string; tools?: { state?: string }[]; quiet?: boolean } | undefined
): boolean {
  if (!streaming) return false
  if (!last || last.quiet) return !last?.quiet
  if (last.kind === 'tool' && last.state === 'pending') return false
  if (last.kind === 'tools' && last.tools?.at(-1)?.state === 'pending') return false
  return true
}

function nextVerb(current: number): number {
  const pick = Math.floor(Math.random() * VERBS.length)
  return pick === current ? (current + 1) % VERBS.length : pick
}

/** Cycling working verbs, in the latest-message slot. Reduced motion keeps one verb. */
export function WorkingStatus({ label }: { label?: string }) {
  const { t } = useTranslation()
  const [index, setIndex] = useState(() => Math.floor(Math.random() * VERBS.length))

  useEffect(() => {
    if (label) return
    if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return
    const timer = window.setInterval(() => setIndex(nextVerb), HOLD_MS)
    return () => window.clearInterval(timer)
  }, [label])

  return (
    <p className="task-pending" data-testid="task-pending" role="status" aria-label={label ?? t('agent.working.label')}>
      <span className="task-pending__mark" aria-hidden="true" data-tool-spinner="" />
      <span className="task-pending__line" aria-hidden="true">
        <span className="task-pending__verb" data-testid="task-pending-verb">
          {label ?? t(VERBS[index])}
        </span>
        <span className="task-pending__dots">...</span>
      </span>
    </p>
  )
}
