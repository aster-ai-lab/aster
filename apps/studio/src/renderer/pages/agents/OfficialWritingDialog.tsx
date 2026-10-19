import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogPortalContainerProvider,
  DialogTitle
} from '@a3s-lab/ui'
import { useEffect, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import {
  OFFICIAL_WRITING_CATEGORIES,
  officialWritingTaskText,
  type OfficialWritingHandoff,
  officialWritingType,
  officialWritingTypesIn,
  writingChoiceOn,
  writingOptionsFor,
  type OfficialWritingTypeId,
  type WritingChoices,
  type WritingField,
  type WritingFields,
  type WritingOption
} from './officialWriting'
import { OfficialWritingWordPreview } from './OfficialWritingWordPreview'

type WritingStep = { name: 'hub' } | { name: 'brief'; typeId: OfficialWritingTypeId }

const HEADER_OPTIONS: readonly WritingOption[] = ['copyNo', 'secret', 'banner', 'urgent', 'docNo', 'signer']
const BODY_OPTIONS: readonly WritingOption[] = ['attachment', 'amount', 'attendance']
const SIGN_OPTIONS: readonly WritingOption[] = ['seal', 'note']
const FOOTER_OPTIONS: readonly WritingOption[] = ['copyTo', 'imprint']

const OPTION_FIELDS: Partial<Record<WritingOption, readonly WritingField[]>> = {
  copyNo: ['copyNoText'],
  secret: ['secretText'],
  urgent: ['urgency'],
  docNo: ['docNumber'],
  signer: ['signerName'],
  attachment: ['attachmentText'],
  amount: ['amountText'],
  attendance: ['present', 'absent', 'observer'],
  note: ['noteText'],
  copyTo: ['copyToText'],
  imprint: ['printOrg', 'printDate']
}

const FIELD_LABEL: Record<WritingField, string> = {
  org: 'workbench.writing.field.org',
  copyNoText: 'workbench.writing.field.copyNo',
  secretText: 'workbench.writing.field.secret',
  urgency: 'workbench.writing.field.urgency',
  docNumber: 'workbench.writing.field.docNumber',
  signerName: 'workbench.writing.field.signer',
  subject: 'workbench.writing.field.subject',
  mainTo: 'workbench.writing.field.mainTo',
  attachmentText: 'workbench.writing.field.attachment',
  amountText: 'workbench.writing.field.amount',
  signature: 'workbench.writing.field.signature',
  issueDate: 'workbench.writing.field.issueDate',
  noteText: 'workbench.writing.field.note',
  present: 'workbench.writing.field.present',
  absent: 'workbench.writing.field.absent',
  observer: 'workbench.writing.field.observer',
  copyToText: 'workbench.writing.field.copyTo',
  printOrg: 'workbench.writing.field.printOrg',
  printDate: 'workbench.writing.field.printDate'
}

const FIELD_PLACEHOLDER: Partial<Record<WritingField, string>> = {
  copyNoText: 'workbench.writing.placeholder.copyNo',
  secretText: 'workbench.writing.placeholder.secret',
  urgency: 'workbench.writing.placeholder.urgency',
  docNumber: 'workbench.writing.placeholder.docNumber',
  subject: 'workbench.writing.placeholder.subject',
  signature: 'workbench.writing.placeholder.signature',
  issueDate: 'workbench.writing.placeholder.issueDate',
  printDate: 'workbench.writing.placeholder.printDate'
}

function WritingSwitch({
  option,
  on,
  label,
  onToggle
}: {
  option: WritingOption
  on: boolean
  label: string
  onToggle: () => void
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      data-ui="workbench.writing.option"
      data-option={option}
      onClick={onToggle}
      className="flex h-8 w-full items-center justify-between gap-3 rounded-[8px] bg-[#f5f5f5] px-2.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-[#171717]/15">
      <span className="text-[13px] text-[#171717]">{label}</span>
      <span className="writing-switch" data-on={on ? 'true' : 'false'} />
    </button>
  )
}

function WritingField({
  field,
  label,
  value,
  placeholder,
  onChange
}: {
  field: WritingField
  label: string
  value: string
  placeholder?: string
  onChange: (value: string) => void
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[12px] leading-4 text-[#8a8a8a]">{label}</span>
      <input
        aria-label={label}
        data-writing-field={field}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        className="h-8 w-full rounded-[8px] border border-[#efefef] bg-white px-2 text-[13px] text-[#171717] outline-none placeholder:text-[#a3a3a3] focus-visible:border-[#171717]"
      />
    </label>
  )
}

export function OfficialWritingDialog({
  open,
  typeId,
  seed,
  onOpenChange,
  onCommit
}: {
  open: boolean
  typeId: OfficialWritingTypeId | null
  seed: string
  onOpenChange: (open: boolean) => void
  onCommit: (handoff: OfficialWritingHandoff) => void
}) {
  const { t } = useTranslation()
  const [step, setStep] = useState<WritingStep>({ name: 'hub' })
  const [brief, setBrief] = useState('')
  const [choices, setChoices] = useState<WritingChoices>({})
  const [fields, setFields] = useState<WritingFields>({})
  const openKey = open ? `${typeId ?? 'hub'}:${seed}` : ''

  useEffect(() => {
    if (!openKey) return
    setBrief(seed)
    setChoices({})
    setFields({})
    setStep(typeId ? { name: 'brief', typeId } : { name: 'hub' })
  }, [openKey, seed, typeId])

  const chosen = step.name === 'hub' ? null : officialWritingType(step.typeId)
  const portal = typeof document === 'undefined' ? null : document.body
  const wide = step.name !== 'hub'

  const toggle = (option: WritingOption) => {
    if (!chosen) return
    setChoices((prev) => ({ ...prev, [option]: !writingChoiceOn(chosen.id, option, prev) }))
  }

  const renderOption = (option: WritingOption) => {
    if (!chosen || !writingOptionsFor(chosen.id).some((item) => item.id === option)) return null
    const on = writingChoiceOn(chosen.id, option, choices)
    const linked = OPTION_FIELDS[option] ?? []
    return (
      <div key={option} className="flex flex-col gap-1">
        <WritingSwitch
          option={option}
          on={on}
          label={t(`workbench.writing.option.${option}`)}
          onToggle={() => toggle(option)}
        />
        {on
          ? linked.map((field) => (
              <WritingField
                key={field}
                field={field}
                label={t(FIELD_LABEL[field])}
                value={fields[field] ?? ''}
                placeholder={FIELD_PLACEHOLDER[field] ? t(FIELD_PLACEHOLDER[field]) : undefined}
                onChange={(value) => setFields((prev) => ({ ...prev, [field]: value }))}
              />
            ))
          : null}
      </div>
    )
  }

  const renderPart = (
    part: 'header' | 'body' | 'footer',
    options: readonly WritingOption[],
    leading?: ReactNode,
    trailing?: ReactNode
  ) => (
    <section data-ui="workbench.writing.part" data-part={part} className="flex flex-col gap-1.5">
      <h3 className="m-0 text-[12px] font-medium tracking-wide text-[#8a8a8a]">
        {t(`workbench.writing.part.${part}`)}
      </h3>
      {leading}
      {options.map((option) => renderOption(option))}
      {trailing}
    </section>
  )

  return (
    <DialogPortalContainerProvider container={portal}>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent
          size="xl"
          closeLabel={t('common.close')}
          overlayClassName="z-[120]"
          data-writing-stage={step.name}
          style={
            wide
              ? {
                  width: 'min(1080px, calc(100vw - 2rem))',
                  maxWidth: 'min(1080px, calc(100vw - 2rem))',
                  height: 'min(860px, calc(100dvh - 2rem))',
                  maxHeight: 'min(860px, calc(100dvh - 2rem))'
                }
              : undefined
          }
          className="z-[120] flex max-h-[min(720px,calc(100dvh-2rem))] w-[min(640px,calc(100vw-2rem))] flex-col gap-0 overflow-hidden bg-white p-0 text-[#171717]">
          <DialogHeader className="shrink-0 border-b border-[#efefef] px-5 py-4 pr-12">
            <DialogTitle className="text-[16px] text-[#171717]">{t('workbench.writing.hub.title')}</DialogTitle>
            <DialogDescription className="text-[13px] text-[#8a8a8a]">
              {chosen ? chosen.title : t('workbench.writing.hub.lead')}
            </DialogDescription>
          </DialogHeader>
          {step.name === 'hub' ? (
            <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
              <div className="flex flex-col gap-5" data-ui="workbench.writing.hub">
                {OFFICIAL_WRITING_CATEGORIES.map((category) => (
                  <section key={category.id} aria-labelledby={`writing-cat-${category.id}`}>
                    <h3 id={`writing-cat-${category.id}`} className="m-0 text-[14px] font-medium text-[#171717]">
                      {t(category.titleKey)}
                    </h3>
                    <p className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">{t(category.summaryKey)}</p>
                    <div className="mt-2 grid grid-cols-1 gap-2 sm:grid-cols-2">
                      {officialWritingTypesIn(category.id).map((type) => (
                        <button
                          key={type.id}
                          type="button"
                          data-official-type={type.id}
                          onClick={() => {
                            setBrief('')
                            setChoices({})
                            setFields({})
                            setStep({ name: 'brief', typeId: type.id })
                          }}
                          className="rounded-[12px] bg-[#f4f5f7] px-3 py-2.5 text-left hover:bg-[#eef0f3]">
                          <span className="block text-[14px] font-medium text-[#171717]">{type.title}</span>
                          <span className="mt-1 block text-[12px] leading-5 text-[#8a8a8a]">{t(type.summaryKey)}</span>
                        </button>
                      ))}
                    </div>
                  </section>
                ))}
              </div>
            </div>
          ) : null}
          {step.name === 'brief' && chosen ? (
            <div
              data-ui="workbench.writing.layout"
              className="grid min-h-0 flex-1 grid-cols-1 min-[900px]:grid-cols-[minmax(0,1.15fr)_minmax(360px,0.85fr)]">
              <div className="flex min-h-0 flex-col overflow-hidden bg-[#f4f5f7] max-[899px]:max-h-[42%] min-[900px]:border-r min-[900px]:border-[#efefef]">
                <OfficialWritingWordPreview typeId={chosen.id} brief={brief} choices={choices} fields={fields} />
              </div>
              <div data-ui="workbench.writing.brief" className="min-h-0 overflow-y-auto px-3 py-3">
                <div className="flex flex-col gap-3">
                  {renderPart(
                    'header',
                    HEADER_OPTIONS,
                    <WritingField
                      field="org"
                      label={t(FIELD_LABEL.org)}
                      value={fields.org ?? ''}
                      onChange={(value) => setFields((prev) => ({ ...prev, org: value }))}
                    />
                  )}
                  {renderPart(
                    'body',
                    BODY_OPTIONS,
                    <>
                      <WritingField
                        field="subject"
                        label={t(FIELD_LABEL.subject)}
                        value={fields.subject ?? ''}
                        placeholder={t(FIELD_PLACEHOLDER.subject!)}
                        onChange={(value) => setFields((prev) => ({ ...prev, subject: value }))}
                      />
                      <WritingField
                        field="mainTo"
                        label={t(FIELD_LABEL.mainTo)}
                        value={fields.mainTo ?? ''}
                        onChange={(value) => setFields((prev) => ({ ...prev, mainTo: value }))}
                      />
                      <label className="flex flex-col gap-1">
                        <span className="text-[12px] leading-4 text-[#8a8a8a]">{t('workbench.writing.hub.brief')}</span>
                        <textarea
                          aria-label={t('workbench.writing.hub.brief')}
                          value={brief}
                          rows={3}
                          placeholder={t('workbench.writing.hub.brief_placeholder')}
                          onChange={(event) => setBrief(event.target.value)}
                          className="min-h-16 w-full resize-none rounded-[8px] border border-[#efefef] bg-white px-2 py-1.5 text-[13px] leading-5 text-[#171717] outline-none placeholder:text-[#a3a3a3] focus-visible:border-[#171717]"
                        />
                      </label>
                    </>,
                    <>
                      <WritingField
                        field="signature"
                        label={t(FIELD_LABEL.signature)}
                        value={fields.signature ?? ''}
                        placeholder={t(FIELD_PLACEHOLDER.signature!)}
                        onChange={(value) => setFields((prev) => ({ ...prev, signature: value }))}
                      />
                      <WritingField
                        field="issueDate"
                        label={t(FIELD_LABEL.issueDate)}
                        value={fields.issueDate ?? ''}
                        placeholder={t(FIELD_PLACEHOLDER.issueDate!)}
                        onChange={(value) => setFields((prev) => ({ ...prev, issueDate: value }))}
                      />
                      {SIGN_OPTIONS.map((option) => renderOption(option))}
                    </>
                  )}
                  {renderPart('footer', FOOTER_OPTIONS)}
                </div>
              </div>
            </div>
          ) : null}
          {step.name === 'brief' && chosen ? (
            <div className="flex shrink-0 items-center justify-end gap-3 border-t border-[#efefef] px-5 py-2.5">
              <Button
                type="button"
                variant="emphasis"
                disabled={brief.trim() === ''}
                data-ui="workbench.writing.commit"
                onClick={() => {
                  const note = brief.trim()
                  const text = officialWritingTaskText(chosen.id, note, choices, fields)
                  if (!text) return
                  onCommit({
                    text,
                    typeId: chosen.id,
                    brief: note,
                    choices: { ...choices },
                    fields: { ...fields }
                  })
                }}>
                {t('workbench.writing.hub.commit')}
              </Button>
            </div>
          ) : null}
        </DialogContent>
      </Dialog>
    </DialogPortalContainerProvider>
  )
}
