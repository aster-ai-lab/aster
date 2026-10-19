import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { type QuickPanelInputAdapter, useOptionalQuickPanel } from '@renderer/components/QuickPanel'
import { AbsoluteFilePathSchema } from '@shared/types/file'

import { isDocxName, reviewComposerDraft } from './reviewHandoff'

/** One source for this draft. Choosing a file never submits or opens a plugin window. */
export function ReviewComposerSource({ adapter }: { adapter: () => QuickPanelInputAdapter | null }) {
  const { t } = useTranslation()
  const panel = useOptionalQuickPanel()
  const [path, setPath] = useState('')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const pending = useRef(false)
  const active = useRef(true)
  useEffect(() => {
    active.current = true
    return () => {
      active.current = false
    }
  }, [])
  const commit = (value: string, name?: string) => {
    const parsed = AbsoluteFilePathSchema.safeParse(value.trim())
    const fileName = name || value.trim().split(/[/\\]/u).pop() || ''
    if (!parsed.success || !parsed.data.toLowerCase().endsWith('.docx') || !isDocxName(fileName)) {
      setError(t('workbench.review.composer.invalid_path'))
      return
    }
    const input = adapter()
    if (!active.current || !input?.insertToken) return
    input.insertText('\n', { tokenizeVariables: false })
    input.insertToken(reviewComposerDraft('', parsed.data, fileName).tokens[1])
    input.focus()
    panel?.close()
  }
  const select = async () => {
    if (pending.current) return
    pending.current = true
    setBusy(true)
    setError('')
    const owner = adapter()
    try {
      const files = await window.api.file.select({
        properties: ['openFile'],
        filters: [{ name: 'Word', extensions: ['docx'] }]
      })
      if (!files?.length || !active.current || owner !== adapter()) return
      if (files.length !== 1 || files[0].size < 1 || files[0].size > 8 * 1024 * 1024) {
        setError(t('workbench.review.composer.invalid_file'))
        return
      }
      commit(files[0].path, isDocxName(files[0].origin_name) ? files[0].origin_name : files[0].name)
    } catch {
      setError(t('workbench.review.composer.pick_failed'))
    } finally {
      pending.current = false
      setBusy(false)
    }
  }
  return (
    <section
      data-review-source-picker=""
      aria-label={t('workbench.review.composer.source')}
      className="m-3 rounded-lg border border-[#e5e5e5] p-3 text-[13px]">
      <p className="mb-2">{t('workbench.review.composer.choose')}</p>
      <div className="flex flex-wrap items-center gap-2">
        <button type="button" disabled={busy} onClick={() => void select()} className="rounded-md border px-3 py-1.5">
          {t('workbench.review.composer.pick')}
        </button>
        <input
          aria-label={t('workbench.review.composer.path')}
          value={path}
          disabled={busy}
          placeholder={t('workbench.review.composer.path_hint')}
          className="min-w-36 flex-1 rounded-md border px-2 py-1.5"
          onChange={(event) => {
            setPath(event.target.value)
            setError('')
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && !event.nativeEvent.isComposing) {
              event.preventDefault()
              commit(path)
            }
          }}
        />
        <button
          type="button"
          disabled={busy || !path.trim()}
          className="rounded-md border px-3 py-1.5"
          onClick={() => commit(path)}>
          {t('workbench.review.composer.use_path')}
        </button>
      </div>
      {error ? (
        <p role="alert" className="mt-2 text-[#b42318]">
          {error}
        </p>
      ) : null}
    </section>
  )
}
