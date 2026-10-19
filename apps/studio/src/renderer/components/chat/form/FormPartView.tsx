import { describeOpenUiForm, type FormField } from '@a3s-lab/intelligent-ui/openui'
import { CircleCheck } from 'lucide-react'
import { use, useCallback, useEffect, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'

import type { FormPartData } from '@shared/data/types/uiParts'
import {
  AGENT_FORM_SPEC_MAX_BYTES,
  agentQuestionSchema,
  agentFormBindingSchema,
  agentFormValuesSchema,
  type AgentFormBinding,
  type AgentQuestion
} from '@shared/types/agentForm'

import type { IntelligentUiValues, IntelligentUiSubmitOutcome } from '../messages/markdown/intelligentUiHost'
import { OpenUiMessage } from '../messages/markdown/OpenUiMessage'
import { AgentFormSubmitContext, type AgentFormSubmit } from './agentFormSubmit'
import { QuestionForm } from './QuestionForm'

function PendingForm({
  binding,
  spec,
  channel,
  question
}: {
  binding: AgentFormBinding
  spec: string
  channel?: AgentFormSubmit
  question?: AgentQuestion
}) {
  const lifetime = useRef<globalThis.AbortController | null>(null)
  useEffect(() => {
    const abort = new globalThis.AbortController()
    lifetime.current = abort
    return () => abort.abort()
  }, [])
  const submit = useCallback(
    async (values: IntelligentUiValues): Promise<IntelligentUiSubmitOutcome> => {
      const abort = lifetime.current
      const parsed = agentFormValuesSchema.safeParse(values)
      if (!channel || !abort || abort.signal.aborted || !parsed.success) return 'failed'
      const digest = await globalThis.crypto.subtle.digest('SHA-256', new globalThis.TextEncoder().encode(spec))
      const revision = `sha256:${[...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('')}`
      if (abort.signal.aborted || revision !== binding.revision) return 'failed'
      const outcome = await channel({ binding, values: parsed.data }, abort.signal)
      return abort.signal.aborted ? 'failed' : outcome
    },
    [binding, spec, channel]
  )
  return question ? (
    <QuestionForm question={question} onSubmit={channel ? submit : null} />
  ) : (
    <OpenUiMessage source={spec} streaming={false} onSubmit={channel ? submit : null} />
  )
}

type AnswerRow = { name: string; label: string; value: string | number | boolean | undefined; kind?: string }

function AnsweredForm({ spec, values }: { spec: string; values: Record<string, string | number | boolean> }) {
  const { t } = useTranslation()
  const outline = useMemo(() => describeOpenUiForm(spec), [spec])
  const rows: AnswerRow[] = useMemo(() => {
    const fields: FormField[] = outline?.fields ?? []
    const known = new Set(fields.map((field) => field.name))
    return [
      ...fields.map((field) => ({ ...field, value: values[field.name] })),
      ...Object.keys(values)
        .filter((name) => !known.has(name))
        .map((name) => ({ name, label: name, value: values[name] }))
    ]
  }, [outline, values])
  const display = (row: AnswerRow) => {
    if (typeof row.value === 'boolean') return row.value ? t('agent.form.yes') : t('agent.form.no')
    if (row.value === undefined || (typeof row.value === 'string' && !row.value.trim())) return null
    return String(row.value)
  }
  const title = outline?.title.trim()

  return (
    <section
      className="my-2 w-full max-w-[520px] rounded-[14px] border border-[#ececec] bg-white px-5 py-4"
      data-intelligent-ui-form="answered">
      <header className="flex items-center gap-2">
        <CircleCheck className="size-4 shrink-0 text-[#3f3f3f]" strokeWidth={1.7} aria-hidden />
        <h3 className="min-w-0 flex-1 truncate text-[14px] leading-5 font-medium text-[#1a1a1a]">
          {title || t('agent.form.answered')}
        </h3>
        {title ? (
          <span className="shrink-0 text-[12px] leading-5 text-[#8a8a8a]">{t('agent.form.answered')}</span>
        ) : null}
      </header>
      {rows.length ? (
        <dl className="mt-3.5 grid grid-cols-[minmax(0,auto)_minmax(0,1fr)] gap-x-6 gap-y-2 border-t border-[#efefef] pt-3 text-[13px] leading-5">
          {rows.map((row) => {
            const shown = display(row)
            return (
              <div key={row.name} className="contents">
                <dt className="max-w-[16rem] truncate text-[#8a8a8a]">{row.label}</dt>
                <dd className="min-w-0 break-words whitespace-pre-wrap text-[#171717]">
                  {shown ?? (
                    <>
                      <span className="text-[#8a8a8a]" aria-hidden>
                        —
                      </span>
                      <span className="sr-only">{t('agent.form.empty')}</span>
                    </>
                  )}
                </dd>
              </div>
            )
          })}
        </dl>
      ) : null}
    </section>
  )
}

export function FormPartView({ part }: { part: { data: FormPartData } }) {
  const channel = use(AgentFormSubmitContext)
  const { t } = useTranslation()
  let question: AgentQuestion | undefined
  const bounded =
    !!part.data.spec &&
    part.data.spec.length <= AGENT_FORM_SPEC_MAX_BYTES &&
    new globalThis.TextEncoder().encode(part.data.spec).byteLength <= AGENT_FORM_SPEC_MAX_BYTES
  if (part.data.format === 'question' && bounded) {
    try {
      const parsed = agentQuestionSchema.safeParse(JSON.parse(part.data.spec))
      if (parsed.success) question = parsed.data
    } catch {
      /* Malformed historical content stays read-only. */
    }
  }
  if (part.data.format === 'question' && !question) return <p role="status">{t('agent.form.invalid')}</p>

  if (part.data.state === 'closed') {
    return question ? (
      <QuestionForm question={question} onSubmit={null} closed />
    ) : (
      <p role="status">{t('agent.form.closed')}</p>
    )
  }

  if (part.data.state === 'answered') {
    if (question) return <QuestionForm question={question} values={part.data.values ?? {}} onSubmit={null} />
    return <AnsweredForm spec={part.data.spec} values={part.data.values ?? {}} />
  }
  const binding = agentFormBindingSchema.safeParse(part.data.binding)
  if (
    (part.data.format !== 'openui' && !question) ||
    !binding.success ||
    binding.data.requestId !== part.data.id ||
    !part.data.spec ||
    new globalThis.TextEncoder().encode(part.data.spec).byteLength > AGENT_FORM_SPEC_MAX_BYTES
  ) {
    return (
      <pre className="overflow-auto" data-intelligent-ui-form="legacy-source">
        <code>{part.data.spec}</code>
      </pre>
    )
  }
  return (
    <div className="my-2">
      <PendingForm
        key={`${JSON.stringify(binding.data)}\0${part.data.spec}`}
        binding={binding.data}
        spec={part.data.spec}
        channel={channel}
        question={question}
      />
    </div>
  )
}
