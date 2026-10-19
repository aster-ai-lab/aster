import { Popover, PopoverContent, PopoverTrigger } from '@a3s-lab/ui'
import { ChevronLeft, FilePlus } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { DocumentFileIcon } from '@renderer/components/icons/DocumentFileIcon'

import {
  OFFICE_CREATE_ICON,
  OFFICE_CREATE_KINDS,
  OFFICE_CREATE_LABEL_KEY,
  OFFICE_CREATE_TITLE_KEY,
  type OfficeCreateKind
} from './blankOfficeFile'
import {
  officeCreateFileName,
  officeCreateIssue,
  officeExtension,
  uniqueOfficeStem,
  type OfficeCreateOutcome,
  type OfficeNameIssue
} from './officeCreateName'

const ISSUE_KEY: Record<OfficeNameIssue, string> = {
  empty: 'documents.create.name_empty',
  invalid: 'documents.create.name_invalid',
  reserved: 'documents.create.name_reserved',
  too_long: 'documents.create.name_too_long',
  taken: 'documents.create.name_taken',
  failed: 'documents.create.failed'
}

/** Chooser for the five office file kinds. A kind click names the file before anything is written. */
export function OfficeCreateMenu({
  busy = false,
  existingNames = [],
  onCreate
}: {
  busy?: boolean
  existingNames?: readonly string[]
  onCreate: (kind: OfficeCreateKind, fileName: string) => Promise<OfficeCreateOutcome>
}) {
  const { t } = useTranslation()
  const headingId = useId()
  const extensionId = useId()
  const errorId = useId()
  const inputRef = useRef<HTMLInputElement>(null)
  const [open, setOpen] = useState(false)
  const [kind, setKind] = useState<OfficeCreateKind | null>(null)
  const [stem, setStem] = useState('')
  const [error, setError] = useState<OfficeNameIssue | null>(null)
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    if (open) return
    setKind(null)
    setStem('')
    setError(null)
    setSubmitting(false)
  }, [open])

  useEffect(() => {
    if (!kind) return
    const node = inputRef.current
    if (!node) return
    node.focus()
    node.select()
  }, [kind])

  const choose = (next: OfficeCreateKind) => {
    const extension = officeExtension(next)
    setStem(uniqueOfficeStem(existingNames, t(OFFICE_CREATE_TITLE_KEY[next]), extension))
    setError(null)
    setKind(next)
  }

  const back = () => {
    setKind(null)
    setError(null)
  }

  const submit = async () => {
    if (!kind || submitting || busy) return
    const extension = officeExtension(kind)
    const shown = inputRef.current?.value ?? stem
    const issue = officeCreateIssue(shown, extension, existingNames)
    if (issue) {
      setError(issue)
      inputRef.current?.focus()
      return
    }
    setSubmitting(true)
    setError(null)
    try {
      const outcome = await onCreate(kind, officeCreateFileName(shown, extension))
      if (outcome === 'ok') {
        setOpen(false)
        return
      }
      setError(outcome === 'taken' ? 'taken' : 'failed')
    } catch {
      setError('failed')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          data-workspace-files-action="new-file"
          disabled={busy}
          className="flex h-[30px] items-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none hover:bg-[#2a2a2a] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
          <FilePlus className="size-4 shrink-0 text-white" color="#ffffff" strokeWidth={2} aria-hidden />
          {t('workbench.workspace.new_file', '新建文件')}
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        side="bottom"
        sideOffset={8}
        data-ui="documents.create-menu"
        onEscapeKeyDown={(event) => {
          if (!kind) return
          event.preventDefault()
          back()
        }}
        className="w-[312px] rounded-[12px] border-[1px] border-[#e5e5e5] bg-white p-3 text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
        <div role="dialog" aria-labelledby={headingId}>
          {kind ? (
            <form
              data-documents-create-step="name"
              onKeyDown={(event) => {
                if (event.key !== 'Escape') return
                event.preventDefault()
                back()
              }}
              onSubmit={(event) => {
                event.preventDefault()
                void submit()
              }}>
              <div className="flex items-center gap-1 px-1 pb-2">
                <button
                  type="button"
                  aria-label={t('documents.create.back')}
                  className="flex size-6 items-center justify-center rounded-[6px] text-[#171717] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10"
                  onClick={back}>
                  <ChevronLeft className="size-4" strokeWidth={1.7} aria-hidden />
                </button>
                <p id={headingId} className="m-0 text-[12px] leading-4 text-[#8a8a8a]">
                  {t('documents.create.group')}
                </p>
              </div>
              <div className="flex items-center gap-2 px-1">
                <DocumentFileIcon type={OFFICE_CREATE_ICON[kind]} className="h-8 w-6" />
                <span className="text-[13px] text-[#171717]">{t(OFFICE_CREATE_LABEL_KEY[kind])}</span>
              </div>
              <label className="mt-3 block">
                <span className="sr-only">{t('documents.create.name')}</span>
                <span className="flex h-8 items-center rounded-[8px] border border-[#e6e6e6] focus-within:border-[#171717]">
                  <input
                    ref={inputRef}
                    aria-label={t('documents.create.name')}
                    aria-invalid={error ? true : undefined}
                    aria-describedby={error ? `${extensionId} ${errorId}` : extensionId}
                    autoCapitalize="off"
                    autoComplete="off"
                    autoCorrect="off"
                    spellCheck={false}
                    value={stem}
                    disabled={submitting || busy}
                    className="min-w-0 flex-1 bg-transparent px-2 text-[13px] text-[#171717] outline-none disabled:opacity-50"
                    onChange={(event) => {
                      setStem(event.currentTarget.value)
                      setError(null)
                    }}
                  />
                  <span id={extensionId} className="shrink-0 pr-2 text-[13px] text-[#8a8a8a]">
                    .{officeExtension(kind)}
                  </span>
                </span>
              </label>
              {error ? (
                <p id={errorId} role="alert" className="m-0 mt-1.5 px-1 text-[12px] leading-4 text-[#E23B3B]">
                  {t(ISSUE_KEY[error])}
                </p>
              ) : null}
              <div className="mt-3 flex justify-end">
                <button
                  type="submit"
                  disabled={submitting || busy}
                  className="h-8 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-50">
                  {t('documents.create.confirm')}
                </button>
              </div>
            </form>
          ) : (
            <>
              <p id={headingId} className="m-0 px-1 pb-2 text-[12px] leading-4 text-[#8a8a8a]">
                {t('documents.create.group')}
              </p>
              <div className="grid grid-cols-4">
                {OFFICE_CREATE_KINDS.map((item) => (
                  <button
                    key={item}
                    type="button"
                    data-documents-create={item}
                    disabled={busy}
                    className="flex flex-col items-center gap-1.5 rounded-[8px] px-1 py-2 text-[12px] leading-4 text-[#171717] outline-none hover:bg-[#f7f7f7] focus-visible:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-50"
                    onClick={() => choose(item)}>
                    <DocumentFileIcon type={OFFICE_CREATE_ICON[item]} className="h-10 w-8" />
                    <span>{t(OFFICE_CREATE_LABEL_KEY[item])}</span>
                  </button>
                ))}
              </div>
            </>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}
