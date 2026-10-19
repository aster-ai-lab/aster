import { useState } from 'react'
import { useTranslation } from 'react-i18next'

/** Why a typed upload name cannot be stored. `null` means the name can be uploaded. */
export function uploadNameProblem(name: string): 'empty' | 'invalid' | 'reserved' | null {
  const trimmed = name.trim()
  if (!trimmed) return 'empty'
  if (trimmed.toLowerCase() === 'wiki') return 'reserved'
  if (trimmed.startsWith('.') || trimmed.includes('/') || trimmed.includes('\\') || trimmed.length > 255) {
    return 'invalid'
  }
  return null
}

const NAME_PROBLEM = {
  empty: 'workbench.workspace.name_empty',
  invalid: 'workbench.workspace.name_invalid',
  reserved: 'workbench.workspace.name_reserved'
} as const

export function DriveFileCheck({ name, checked, onToggle }: { name: string; checked: boolean; onToggle: () => void }) {
  const { t } = useTranslation()
  return (
    <input
      type="checkbox"
      className="size-4 shrink-0 accent-[#171717]"
      aria-label={t('workbench.workspace.select_file', { name, defaultValue: '选择 {{name}}' })}
      checked={checked}
      onClick={(event) => event.stopPropagation()}
      onChange={onToggle}
    />
  )
}

export function DriveUploadButton({ onUpload }: { onUpload: (file: File) => Promise<void> }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [fileName, setFileName] = useState('')
  const [body, setBody] = useState('')
  const [problem, setProblem] = useState('')
  const [busy, setBusy] = useState(false)

  const submit = () => {
    const found = uploadNameProblem(fileName)
    if (found) {
      setProblem(t(NAME_PROBLEM[found]))
      return
    }
    const name = fileName.trim()
    setBusy(true)
    setProblem('')
    void onUpload(new File([body], name, { type: 'text/plain' })).then(
      () => {
        setBusy(false)
        setOpen(false)
        setFileName('')
        setBody('')
      },
      () => {
        setBusy(false)
        setProblem(t('workbench.workspace.mutate_failed'))
      }
    )
  }

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        data-workspace-files-action="upload"
        className="flex h-[30px] items-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}>
        {t('documents.upload')}
      </button>
      {open ? (
        <form
          className="flex items-center gap-2"
          onSubmit={(event) => {
            event.preventDefault()
            if (!busy) submit()
          }}>
          <input
            aria-label={t('workbench.workspace.upload_name', { defaultValue: '上传文件名' })}
            className="h-[30px] w-[160px] rounded-[8px] bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717] outline-none"
            value={fileName}
            spellCheck={false}
            onChange={(event) => setFileName(event.currentTarget.value)}
          />
          <input
            aria-label={t('workbench.workspace.upload_body', { defaultValue: '上传内容' })}
            className="h-[30px] w-[180px] rounded-[8px] bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717] outline-none"
            value={body}
            spellCheck={false}
            onChange={(event) => setBody(event.currentTarget.value)}
          />
          <button
            type="submit"
            disabled={busy}
            className="flex h-[30px] items-center rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none hover:bg-[#2a2a2a] disabled:opacity-40">
            {t('workbench.workspace.upload_submit', { defaultValue: '开始上传' })}
          </button>
        </form>
      ) : null}
      {problem ? (
        <p className="m-0 text-[13px] text-[#b42318]" role="alert">
          {problem}
        </p>
      ) : null}
    </div>
  )
}

export function DriveHandoffButton({
  count,
  names,
  onHandoff
}: {
  count: number
  names: readonly string[]
  onHandoff: (sentence: string) => void
}) {
  const { t } = useTranslation()
  const [handed, setHanded] = useState('')
  const sentence = t('workbench.workspace.handed_off', {
    names: names.join('、'),
    defaultValue: '已交给任务：{{names}}'
  })
  return (
    <div className="flex items-center gap-2">
      {count > 0 ? (
        <span className="text-[13px] text-[#3f3f3f]">{t('files.footer_selected_count', { count })}</span>
      ) : null}
      <button
        type="button"
        data-workspace-files-action="handoff"
        disabled={count === 0}
        className="flex h-[30px] items-center rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40"
        onClick={() => {
          if (count === 0) return
          setHanded(sentence)
          onHandoff(sentence)
        }}>
        {t('workbench.workspace.handoff', { defaultValue: '交给任务' })}
      </button>
      {handed ? (
        <p className="m-0 text-[13px] text-[#171717]" role="status">
          {handed}
        </p>
      ) : null}
    </div>
  )
}
