import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@a3s-lab/ui'
import { Upload } from 'lucide-react'
import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { MemberOrgError } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'

import { SKILL_ARCHIVE_BYTES, uploadSkillPackage } from './capabilitiesData'

const ACCEPT = '.zip,.skill,application/zip'

type UploadRow = {
  id: string
  file: File
  error?: string
  installedName?: string
}

function packageError(file: File): string | null {
  const name = file.name.toLowerCase()
  if (!name.endsWith('.zip') && !name.endsWith('.skill')) return 'bad_type'
  if (file.size <= 0 || file.size > SKILL_ARCHIVE_BYTES) return 'too_large'
  return null
}

function errorKey(error: unknown): string {
  const code = error instanceof MemberOrgError ? error.message : ''
  const known = [
    'bad_format',
    'no_skill_md',
    'ambiguous',
    'nested',
    'invalid_frontmatter',
    'invalid_name',
    'invalid_description',
    'too_many_entries',
    'unpacks_too_large',
    'escapes',
    'symlink',
    'skill_exists',
    'skill_md_too_large',
    'too_large',
    'failed'
  ]
  return known.includes(code) ? code : 'failed'
}

export function SkillUploadDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const { t } = useTranslation()
  const inputRef = useRef<HTMLInputElement>(null)
  const busyRef = useRef(false)
  const [rows, setRows] = useState<UploadRow[]>([])
  const [rejected, setRejected] = useState<string | null>(null)
  const [over, setOver] = useState(false)
  const [busy, setBusy] = useState(false)
  const pending = rows.some((row) => !row.installedName)

  const setBusyState = (value: boolean) => {
    busyRef.current = value
    setBusy(value)
  }

  const reset = () => {
    setRows([])
    setRejected(null)
    setOver(false)
    setBusyState(false)
  }

  const addFiles = (list: FileList | File[]) => {
    const files = Array.from(list)
    if (files.length === 0) return
    const next: UploadRow[] = []
    let refusal: string | null = null
    for (const file of files) {
      const problem = packageError(file)
      if (problem) {
        refusal = problem
        continue
      }
      next.push({ id: `${file.name}:${file.size}:${file.lastModified}`, file })
    }
    setRejected(refusal)
    if (next.length > 0) {
      setRows((current) => {
        const seen = new Set(current.map((row) => row.id))
        return [...current, ...next.filter((row) => !seen.has(row.id))].slice(0, 8)
      })
    }
  }

  const install = async () => {
    if (busyRef.current || !pending) return
    setBusyState(true)
    setRejected(null)
    let installed = 0
    let failed = false
    let lastName = ''
    for (const row of rows) {
      if (row.installedName) continue
      setRows((current) => current.map((item) => (item.id === row.id ? { ...item, error: undefined } : item)))
      try {
        const result = await uploadSkillPackage(row.file)
        installed += 1
        lastName = result.name
        setRows((current) =>
          current.map((item) => (item.id === row.id ? { ...item, installedName: result.name, error: undefined } : item))
        )
      } catch (error) {
        failed = true
        const key = errorKey(error)
        setRows((current) => current.map((item) => (item.id === row.id ? { ...item, error: key } : item)))
      }
    }
    setBusyState(false)
    if (!failed && installed > 0) {
      toast.success(t('capabilities.skill.upload.success', { name: lastName }))
      reset()
      onOpenChange(false)
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (busyRef.current) return
        if (!next) reset()
        onOpenChange(next)
      }}>
      {open ? (
        <DialogContent
          closeLabel={t('common.close')}
          closeOnOverlayClick={!busy}
          overlayClassName="!opacity-100"
          className="flex max-h-[min(640px,calc(100vh-4rem))] w-full max-w-[min(420px,calc(100%-2rem))] !animate-none flex-col gap-0 overflow-hidden p-0 !opacity-100 sm:max-w-[min(420px,calc(100%-2rem))]">
          <div data-ui="capabilities.skill-upload" className="flex min-h-0 flex-1 flex-col px-6 pt-5 pb-6">
            <DialogTitle className="pr-8 text-[15px] leading-6 font-medium text-[#171717]">
              {t('capabilities.skill.upload.title')}
            </DialogTitle>
            <DialogDescription className="sr-only">{t('capabilities.skill.upload.hint')}</DialogDescription>
            <button
              type="button"
              data-ui="capabilities.skill-upload.drop"
              onClick={() => inputRef.current?.click()}
              onDragOver={(event) => {
                event.preventDefault()
                setOver(true)
              }}
              onDragLeave={() => setOver(false)}
              onDrop={(event) => {
                event.preventDefault()
                setOver(false)
                addFiles(event.dataTransfer.files)
              }}
              className={`mt-4 flex h-[140px] w-full flex-col items-center justify-center gap-2 rounded-[8px] border border-dashed border-[#e5e5e5] px-4 text-[13px] leading-5 text-[#8a8a8a] outline-none focus-visible:ring-2 focus-visible:ring-black/10 ${over ? 'bg-[#ececec]' : 'bg-[#f5f5f5] hover:bg-[#ececec]'}`}>
              <Upload className="size-4 text-current" strokeWidth={1.7} aria-hidden />
              {t('capabilities.skill.upload.hint')}
            </button>
            <input
              ref={inputRef}
              type="file"
              accept={ACCEPT}
              multiple
              className="sr-only"
              data-ui="capabilities.skill-upload.input"
              onChange={(event) => {
                if (event.currentTarget.files) addFiles(event.currentTarget.files)
                event.currentTarget.value = ''
              }}
            />
            <p className="m-0 mt-3 text-[12px] leading-5 text-[#8a8a8a]">{t('capabilities.skill.upload.limits')}</p>
            {rejected ? (
              <p className="m-0 mt-2 text-[13px] leading-5 text-[#b42318]" role="alert">
                {t(`capabilities.skill.upload.${rejected}`)}
              </p>
            ) : null}
            {rows.length > 0 ? (
              <ul className="mt-3 flex max-h-[180px] flex-col gap-0.5 overflow-y-auto">
                {rows.map((row) => (
                  <li
                    key={row.id}
                    className="flex items-center gap-2 rounded-[8px] px-2.5 py-2 text-[13px] text-[#171717]">
                    <span className="min-w-0 flex-1 truncate">{row.file.name}</span>
                    {row.error ? (
                      <span className="shrink-0 text-[#b42318]">{t(`capabilities.skill.upload.${row.error}`)}</span>
                    ) : row.installedName ? (
                      <span className="shrink-0 text-[#8a8a8a]">{row.installedName}</span>
                    ) : (
                      <button
                        type="button"
                        disabled={busy}
                        onClick={() => setRows((current) => current.filter((item) => item.id !== row.id))}
                        className="shrink-0 text-[#8a8a8a] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                        {t('capabilities.skill.upload.remove')}
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            ) : null}
            <button
              type="button"
              disabled={!pending || busy}
              onClick={() => void install()}
              className="mt-4 inline-flex h-9 items-center justify-center self-end rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none hover:bg-[#2a2a2a] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
              {t(busy ? 'capabilities.skill.upload.installing' : 'capabilities.skill.upload.install')}
            </button>
          </div>
        </DialogContent>
      ) : null}
    </Dialog>
  )
}
