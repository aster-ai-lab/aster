import { lazy, Suspense } from 'react'
import { useTranslation } from 'react-i18next'

import type { PaperloomWorkspaceSource } from './paperloomApi'

const Workspace = lazy(() => import('./PaperloomWorkspace').then((module) => ({ default: module.PaperloomWorkspace })))

export function PaperloomPanel(props: {
  onClose: () => void
  question?: string
  closeLabel?: string
  source?: PaperloomWorkspaceSource
  onSourceReady?: () => void
}) {
  const { t } = useTranslation()
  return (
    <Suspense
      fallback={
        <p role="status" className="p-4 text-[13px] text-[#8a8a8a]">
          {t('paperloom.loading')}
        </p>
      }>
      <Workspace {...props} />
    </Suspense>
  )
}
