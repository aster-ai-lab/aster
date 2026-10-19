import { Badge, type BadgeTone } from '@a3s-lab/ui'
import { useTranslation } from 'react-i18next'

import type { WorkspaceDiagnosticObservation, WorkspaceDocumentDiagnostic } from '@shared/types/workspaceDocument'

const SEVERITIES: Record<WorkspaceDocumentDiagnostic['severity'], { key: string; tone: BadgeTone }> = {
  error: { key: 'documents.diagnostics.error', tone: 'danger' },
  warning: { key: 'documents.diagnostics.warning', tone: 'warn' },
  information: { key: 'documents.diagnostics.information', tone: 'neutral' },
  hint: { key: 'documents.diagnostics.hint', tone: 'neutral' }
}

export function DocumentDiagnostics({
  observation,
  unavailable
}: {
  observation: WorkspaceDiagnosticObservation | null
  unavailable: boolean
}) {
  const { t } = useTranslation()
  if (!unavailable && !observation?.diagnostics.length) return null
  const rows =
    observation?.diagnostics.flatMap((publication) =>
      publication.items.map((item, index) => ({ key: `${publication.publisherId}:${index}`, item }))
    ) ?? []
  return (
    <section aria-label={t('documents.diagnostics.title')} className="shrink-0 border-t border-[#efefef] bg-white">
      <div className="flex items-center gap-2 px-3 py-2 text-[13px] font-medium text-[#3f3f3f]">
        {t('documents.diagnostics.title')}
        {unavailable ? null : <Badge>{rows.length}</Badge>}
      </div>
      <p className="m-0 px-3 pb-2 text-[12px] text-[#8a8a8a]">
        {t(unavailable ? 'documents.diagnostics.unavailable' : 'documents.diagnostics.incomplete')}
      </p>
      {rows.length ? (
        <ul className="m-0 max-h-40 list-none overflow-auto px-3 pb-2">
          {rows.map(({ key, item }) => (
            <li key={key} className="flex items-start gap-2 py-1 text-[13px] text-[#3f3f3f]">
              <Badge tone={SEVERITIES[item.severity].tone}>{t(SEVERITIES[item.severity].key)}</Badge>
              <span className="break-words whitespace-pre-wrap">{item.message}</span>
              <span className="ml-auto shrink-0 font-mono text-[12px] text-[#8a8a8a]">
                {t('documents.diagnostics.position', { line: item.start.line + 1, column: item.start.character + 1 })}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  )
}
