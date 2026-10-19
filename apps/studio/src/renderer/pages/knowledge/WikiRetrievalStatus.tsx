import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { useWikiRetrievalPhase, type WikiRetrievalPhase } from './wikiRetrievalPhase'

const SHOW_PREPARING_AFTER_MS = 400

/** Quiet meta line. A fast index never flashes, and a published index leaves the line blank. */
export function WikiRetrievalStatus() {
  const { t } = useTranslation()
  const phase = useWikiRetrievalPhase()
  const visible = useDelayedPhase(phase)
  if (visible === 'idle') return null
  const text = visible === 'failed' ? t('knowledge.notes.retrieval_failed') : t('knowledge.notes.retrieval_preparing')
  return (
    <p
      role="status"
      aria-live="polite"
      data-ui="knowledge.notes.retrieval"
      className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">
      {text}
    </p>
  )
}

function useDelayedPhase(phase: WikiRetrievalPhase): WikiRetrievalPhase {
  const [visible, setVisible] = useState<WikiRetrievalPhase>('idle')
  useEffect(() => {
    if (phase === 'idle') {
      setVisible('idle')
      return
    }
    if (phase === 'failed') {
      setVisible('failed')
      return
    }
    const timer = window.setTimeout(() => setVisible('preparing'), SHOW_PREPARING_AFTER_MS)
    return () => window.clearTimeout(timer)
  }, [phase])
  return visible
}
