import type { AgentParameterField } from '@a3s-lab/intelligent-ui/openui'
import { SearchSelect } from '@a3s-lab/ui'
import { useEffect, useRef, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import {
  acceptParameterPrefill,
  agentParameterDraftSnapshot,
  editAgentParameter,
  subscribeAgentParameterDraft
} from './agentParameterDraft'
import { prefillAgentParameters } from './agentParameterPrefill'

/** The composer send button submits this form. There is no second submit control. */
export function AgentParameterForm({
  mentionId,
  modelId,
  text
}: {
  mentionId: string | null
  modelId?: string
  text: string
}) {
  const { t } = useTranslation()
  const draft = useSyncExternalStore(
    subscribeAgentParameterDraft,
    agentParameterDraftSnapshot,
    agentParameterDraftSnapshot
  )
  const textRef = useRef(text)
  textRef.current = text

  useEffect(() => {
    if (!draft || !modelId || draft.agentId !== mentionId) return
    const capturedGeneration = draft.generation
    const agentId = draft.agentId
    const schema = draft.schema
    const fields = draft.fields
    let cancelled = false
    void prefillAgentParameters({ modelId, text: textRef.current, schema, fields }).then((extracted) => {
      if (!cancelled) acceptParameterPrefill(agentId, capturedGeneration, extracted)
    })
    return () => {
      cancelled = true
    }
  }, [draft?.agentId, draft?.generation, mentionId, modelId])

  if (!draft || draft.agentId !== mentionId) return null

  return (
    <section className="mb-2 rounded-lg border border-[#e5e5e5] bg-white px-3 py-2" data-agent-parameter-form="">
      <p className="mb-2 text-[12px] leading-4 text-[#666]">{t('workbench.composer.agent_parameters', '调用参数')}</p>
      <div className="flex flex-col gap-2">
        {draft.fields.map((field) => (
          <ParameterControl
            key={field.name}
            field={field}
            value={draft.values[field.name]}
            onChange={(value) => editAgentParameter(field.name, value)}
          />
        ))}
      </div>
    </section>
  )
}

function ParameterControl({
  field,
  value,
  onChange
}: {
  field: AgentParameterField
  value: string | number | boolean | undefined
  onChange: (value: string | number | boolean | null) => void
}) {
  const label = field.required ? `${field.label} *` : field.label
  if (field.enumValues && field.enumValues.every((item) => typeof item === 'string')) {
    return (
      <label className="flex flex-col gap-1 text-[13px] text-[#171717]">
        {label}
        <SearchSelect
          size="sm"
          searchable={field.enumValues.length > 8}
          allowClear={!field.required}
          aria-label={field.label}
          value={typeof value === 'string' ? value : ''}
          options={field.enumValues.map((item) => ({ value: String(item), label: String(item) }))}
          onChange={(next) => onChange(next || null)}
        />
      </label>
    )
  }
  if (field.kind === 'boolean') {
    return (
      <label className="flex items-center gap-2 text-[13px] text-[#171717]">
        <input
          type="checkbox"
          aria-label={field.label}
          checked={value === true}
          onChange={(event) => onChange(event.target.checked)}
        />
        {label}
      </label>
    )
  }
  if (field.slider && field.minimum !== undefined && field.maximum !== undefined) {
    return (
      <label className="flex flex-col gap-1 text-[13px] text-[#171717]">
        {label}
        <input
          type="range"
          aria-label={field.label}
          min={field.minimum}
          max={field.maximum}
          step={1}
          value={typeof value === 'number' ? value : field.minimum}
          onChange={(event) => onChange(Number(event.target.value))}
        />
      </label>
    )
  }
  if (field.longText) {
    return (
      <label className="flex flex-col gap-1 text-[13px] text-[#171717]">
        {label}
        <textarea
          aria-label={field.label}
          className="min-h-16 rounded-md border border-[#e5e5e5] px-2 py-1"
          value={typeof value === 'string' ? value : ''}
          onChange={(event) => onChange(event.target.value)}
        />
      </label>
    )
  }
  return (
    <label className="flex flex-col gap-1 text-[13px] text-[#171717]">
      {label}
      <input
        aria-label={field.label}
        className="h-8 rounded-md border border-[#e5e5e5] px-2"
        type={field.kind === 'string' ? 'text' : 'number'}
        value={value ?? ''}
        onChange={(event) => {
          if (field.kind === 'string') onChange(event.target.value)
          else onChange(event.target.value === '' ? null : Number(event.target.value))
        }}
      />
    </label>
  )
}
