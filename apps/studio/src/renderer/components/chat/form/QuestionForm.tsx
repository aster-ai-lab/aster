import { Button, ConsoleTextarea, Label, SearchSelect } from '@a3s-lab/ui'
import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'

import type { AgentFormValues, AgentQuestion } from '@shared/types/agentForm'

import type { IntelligentUiSubmitOutcome, IntelligentUiValues } from '../messages/markdown/intelligentUiHost'

export function QuestionForm({
  question,
  values,
  onSubmit,
  closed = false
}: {
  question: AgentQuestion
  values?: AgentFormValues
  onSubmit: ((values: IntelligentUiValues) => Promise<IntelligentUiSubmitOutcome>) | null
  closed?: boolean
}) {
  const { t } = useTranslation()
  const answerId = useId()
  const errorId = useId()
  const [choice, setChoice] = useState('')
  const [answer, setAnswer] = useState('')
  const [status, setStatus] = useState<'idle' | 'sending' | 'sent' | 'failed' | 'required'>('idle')
  const disabled = !onSubmit || status === 'sending' || status === 'sent'
  const selected = values ? question.choices.find((option) => option.id === values.choice)?.label : undefined

  async function submit() {
    if (!onSubmit || disabled) return
    if ((!choice && !answer.trim()) || (choice && answer.trim())) {
      setStatus('required')
      return
    }
    setStatus('sending')
    try {
      const outcome = await onSubmit({ choice, answer })
      setStatus(outcome === 'sent' ? 'sent' : 'failed')
    } catch {
      setStatus('failed')
    }
  }

  return (
    <section
      className="w-full max-w-[520px] space-y-3 rounded-[14px] border border-[#ececec] bg-white p-5"
      data-intelligent-ui-form={values ? 'answered' : closed ? 'closed' : 'question'}>
      <h3 className="text-[14px] leading-5 font-medium text-[#1a1a1a]">{question.question}</h3>
      {values ? (
        <>
          <p className="text-[12px] text-[#8a8a8a]">{t('agent.form.answered')}</p>
          <p className="text-[13px] break-words whitespace-pre-wrap">
            {selected ?? values.answer ?? t('agent.form.empty')}
          </p>
        </>
      ) : closed ? (
        <p role="status" className="text-[13px] text-[#8a8a8a]">
          {t('agent.form.closed')}
        </p>
      ) : (
        <>
          {question.choices.length > 0 && (
            <SearchSelect
              aria-label={question.question}
              aria-invalid={status === 'required'}
              aria-describedby={status === 'required' ? errorId : undefined}
              value={choice}
              options={question.choices.map((option) => ({ value: option.id, label: option.label }))}
              placeholder={t('common.select')}
              searchable={false}
              disabled={disabled}
              onChange={(value) => {
                setChoice(value)
                setAnswer('')
                setStatus('idle')
              }}
            />
          )}
          {question.allowFreeText && (
            <div className="space-y-2">
              <Label htmlFor={answerId}>
                {question.choices.length ? t('common.other') : t('agent.askUserQuestion.customPlaceholder')}
              </Label>
              <ConsoleTextarea
                id={answerId}
                aria-invalid={status === 'required'}
                aria-describedby={status === 'required' ? errorId : undefined}
                value={answer}
                maxLength={4096}
                disabled={disabled}
                placeholder={t('agent.askUserQuestion.customPlaceholder')}
                onChange={(event) => {
                  setAnswer(event.target.value)
                  setChoice('')
                  setStatus('idle')
                }}
              />
            </div>
          )}
          <Button type="button" disabled={disabled} onClick={() => void submit()}>
            {status === 'sending'
              ? t('agent.form.submitting')
              : status === 'sent'
                ? t('agent.form.submitted')
                : t('agent.askUserQuestion.submit')}
          </Button>
          {status === 'failed' || status === 'required' ? (
            <p id={errorId} role="status" className="text-[13px] text-[#8a8a8a]">
              {t(status === 'required' ? 'agent.form.required' : 'agent.form.sendFailed')}
            </p>
          ) : null}
        </>
      )}
    </section>
  )
}
