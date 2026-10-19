import { IntelligentUI } from '@a3s-lab/intelligent-ui'
import { compileOpenUi } from '@a3s-lab/intelligent-ui/openui'
import { useTranslation } from 'react-i18next'

import type { IntelligentUiSubmitOutcome, IntelligentUiValues } from './intelligentUiHost'

/** Paint closed OpenUI Lang nodes in the session tree. Invalid source stays text. */
export function OpenUiMessage({
  source,
  streaming,
  onSubmit
}: {
  source: string
  streaming: boolean
  onSubmit?: ((values: IntelligentUiValues) => Promise<IntelligentUiSubmitOutcome>) | null
}) {
  const { t } = useTranslation()
  const compiled = compileOpenUi(source, streaming)
  if (!compiled.ok || !compiled.document.nodes.length) {
    return (
      <pre data-openui="source">
        <code>{source}</code>
      </pre>
    )
  }
  return (
    <div data-openui="ready">
      <IntelligentUI
        source={source}
        streaming={streaming || compiled.pending}
        labels={{
          invalid: t('agent.form.invalid'),
          pending: t('common.loading'),
          unavailable: t('agent.form.unavailable'),
          sent: t('agent.form.submitted'),
          busy: t('agent.form.busy'),
          failed: t('agent.form.sendFailed'),
          required: t('agent.form.required'),
          submitting: t('agent.form.submitting')
        }}
        submit={onSubmit !== undefined ? (onSubmit ?? undefined) : undefined}
      />
    </div>
  )
}
