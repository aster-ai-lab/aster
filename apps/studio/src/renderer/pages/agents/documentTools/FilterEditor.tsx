import { SearchSelect } from '@a3s-lab/ui'
import { Undo2 } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { decodePng, encodePng } from '../pdfToolsPng'
import type { ToolOutput } from '../pdfToolsRun'
import {
  applyFilterEdit,
  FILTER_KINDS,
  filterEditsEqual,
  filterPageEdit,
  filterThumbnail,
  ORIGINAL_FILTER,
  parseFilterComparison,
  type FilterComparison,
  type FilterDocumentEdit,
  type FilterEdit
} from './rasterFilter'

const P = 'workbench.feature.pdf.tools.workspace.filters.'
const LABEL = 'workbench.feature.pdf.tools.workspace.filter.'
const BUTTON =
  'inline-flex min-h-9 items-center justify-center gap-1.5 rounded-[8px] border border-[var(--spec-button-border,#e5e5e5)] bg-white px-3 text-[13px] hover:bg-[var(--spec-quiet-button-hover,#fafafa)] disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-offset-2'
export type FilterHistory = Readonly<{
  present: Readonly<Record<number, FilterEdit>>
  undo: readonly Readonly<Record<number, FilterEdit>>[]
  page?: number
  selectedPages?: readonly number[]
  applyPages?: boolean
  applyFiles?: boolean
  comparison?: FilterComparison
  split?: number
  showOriginal?: boolean
}>

export function FilterEditor({
  id,
  sources,
  committed,
  accepted,
  histories,
  busy,
  batch,
  onApply,
  onDraft
}: {
  id: string
  sources: readonly ToolOutput[]
  committed: FilterDocumentEdit
  accepted: boolean
  histories: Map<string, FilterHistory>
  busy: boolean
  batch: boolean
  onApply: (edit: FilterEdit, pages: readonly number[], selectedFiles: boolean) => Promise<void>
  onDraft: (dirty: boolean) => void
}) {
  const { t } = useTranslation()
  const [history, setHistory] = useState<FilterHistory>(
    () =>
      histories.get(id) ?? {
        present: Object.fromEntries(sources.map((_, index) => [index + 1, filterPageEdit(committed, index + 1)])),
        undo: []
      }
  )
  const [page, setPage] = useState(() => Math.min(sources.length, histories.get(id)?.page ?? 1))
  const [selectedPages, setSelectedPages] = useState<ReadonlySet<number>>(
    () => new Set(histories.get(id)?.selectedPages ?? [1])
  )
  const [applyPages, setApplyPages] = useState(histories.get(id)?.applyPages ?? false)
  const [applyFiles, setApplyFiles] = useState(histories.get(id)?.applyFiles ?? false)
  const [comparison, setComparison] = useState<FilterComparison>(histories.get(id)?.comparison ?? 'side-by-side')
  const [split, setSplit] = useState(histories.get(id)?.split ?? 50)
  const [showOriginal, setShowOriginal] = useState(histories.get(id)?.showOriginal ?? false)
  useEffect(() => {
    histories.set(id, {
      // Preference effects may run after an Apply publishes a newer draft.
      // Only the synchronous edit commands own present/undo.
      ...(histories.get(id) ?? history),
      page,
      selectedPages: [...selectedPages],
      applyPages,
      applyFiles,
      comparison,
      split,
      showOriginal
    })
  }, [histories, id, history, page, selectedPages, applyPages, applyFiles, comparison, split, showOriginal])
  const [applying, setApplying] = useState(false)
  const [failure, setFailure] = useState(false)
  const [previewResult, setPreviewResult] = useState<{ source: ToolOutput; edit: FilterEdit; url: string }>()
  const [thumbnails, setThumbnails] = useState<Readonly<Record<string, string>>>({})
  const source = sources[page - 1]
  const edit = history.present[page] ?? filterPageEdit(committed, page)
  const preview =
    previewResult?.source === source && filterEditsEqual(previewResult.edit, edit) ? previewResult.url : undefined
  const isDirty = (present: Readonly<Record<number, FilterEdit>>) =>
    sources.some(
      (_, index) =>
        !filterEditsEqual(
          present[index + 1] ?? filterPageEdit(committed, index + 1),
          filterPageEdit(committed, index + 1)
        )
    )
  const dirty = isDirty(history.present)
  // Apply may replace the committed document without an intermediate render
  // of dirty=true. Reconcile each draft/commit, even when both renders are clean.
  useEffect(() => onDraft(dirty), [dirty, onDraft, history.present, committed])
  const sourceUrl = useMemo(
    () =>
      source
        ? globalThis.URL.createObjectURL(new globalThis.Blob([source.bytes.slice()], { type: source.mediaType }))
        : undefined,
    [source]
  )
  useEffect(
    () => () => {
      if (sourceUrl) globalThis.URL.revokeObjectURL(sourceUrl)
    },
    [sourceUrl]
  )
  useEffect(() => {
    const controller = new globalThis.AbortController()
    let url: string | undefined
    setPreviewResult(undefined)
    setFailure(false)
    if (source)
      void (async () => {
        try {
          const pixels = await applyFilterEdit(await decodePng(source.bytes), edit, controller.signal)
          const bytes = await encodePng(pixels)
          if (controller.signal.aborted) return
          url = globalThis.URL.createObjectURL(new globalThis.Blob([bytes.slice()], { type: 'image/png' }))
          setPreviewResult({ source, edit, url })
        } catch {
          if (!controller.signal.aborted) setFailure(true)
        }
      })()
    return () => {
      controller.abort()
      if (url) globalThis.URL.revokeObjectURL(url)
    }
  }, [source, edit])
  useEffect(() => {
    const controller = new globalThis.AbortController()
    const urls: Record<string, string> = {}
    setThumbnails({})
    if (source)
      void (async () => {
        try {
          const small = filterThumbnail(await decodePng(source.bytes))
          for (const name of FILTER_KINDS) {
            const bytes = await encodePng(
              await applyFilterEdit(small, { name, intensity: edit.intensity }, controller.signal)
            )
            if (controller.signal.aborted) return
            urls[name] = globalThis.URL.createObjectURL(new globalThis.Blob([bytes.slice()], { type: 'image/png' }))
          }
          setThumbnails({ ...urls })
        } catch {
          if (!controller.signal.aborted) setFailure(true)
        }
      })()
    return () => {
      controller.abort()
      for (const url of Object.values(urls)) globalThis.URL.revokeObjectURL(url)
    }
  }, [source, edit.intensity])
  const locked = busy || applying
  const update = (present: Readonly<Record<number, FilterEdit>>) => {
    const old = histories.get(id) ?? history
    const next = { ...old, present, undo: [...old.undo, old.present].slice(-32) }
    // Apply can immediately unmount this view. Publish the owned draft before
    // dispatching the task; a queued React updater may never run after unmount.
    histories.set(id, next)
    // Hide the previous export in the same event as a local edit. Waiting for
    // an effect leaves the old accepted bytes briefly downloadable.
    onDraft(isDirty(present))
    setHistory(next)
  }
  const change = (next: Partial<FilterEdit>) => update({ ...history.present, [page]: { ...edit, ...next } })
  const undo = () => {
    const old = histories.get(id) ?? history
    const previous = old.undo.at(-1)
    if (!previous) return
    const next = { ...old, present: previous, undo: old.undo.slice(0, -1) }
    histories.set(id, next)
    onDraft(isDirty(previous))
    setHistory(next)
  }
  const targets = applyFiles ? sources.map((_, index) => index + 1) : applyPages ? [...selectedPages] : [page]
  const needsApply =
    !accepted || targets.some((target) => !filterEditsEqual(edit, filterPageEdit(committed, target))) || applyFiles
  const apply = async () => {
    if (locked || !preview || failure || targets.length === 0) return
    if (applyPages || applyFiles)
      update({ ...history.present, ...Object.fromEntries(targets.map((target) => [target, edit])) })
    setApplying(true)
    try {
      await onApply(edit, targets, applyFiles)
    } catch {
      setFailure(true)
    } finally {
      setApplying(false)
    }
  }
  return (
    <section
      data-ui="document-tools.filter-editor"
      aria-label={t(P + 'editor')}
      className="flex min-w-0 flex-col gap-4"
      onKeyDown={(event) => {
        if (locked || (event.target instanceof globalThis.Element && event.target.closest('input,select,textarea')))
          return
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
          event.preventDefault()
          undo()
        }
      }}>
      {sources.length > 1 ? (
        <nav aria-label={t(P + 'pages')} className="flex flex-wrap gap-2">
          {sources.map((_, index) => (
            <div key={index} className="flex items-center gap-1">
              <input
                type="checkbox"
                aria-label={t(P + 'select_page', { page: index + 1 })}
                disabled={locked}
                checked={selectedPages.has(index + 1)}
                onChange={(event) => {
                  const next = new Set(selectedPages)
                  if (event.target.checked) next.add(index + 1)
                  else next.delete(index + 1)
                  setSelectedPages(next)
                }}
              />
              <button
                type="button"
                className={BUTTON}
                disabled={locked}
                aria-pressed={page === index + 1}
                onClick={() => {
                  setPage(index + 1)
                  setShowOriginal(false)
                }}>
                {t(P + 'page', { page: index + 1 })}
              </button>
            </div>
          ))}
        </nav>
      ) : null}
      <fieldset disabled={locked} className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <legend className="mb-2 text-[13px]">{t(P + 'choose')}</legend>
        {FILTER_KINDS.map((name) => (
          <label
            key={name}
            className={`flex min-w-0 cursor-pointer flex-col items-center gap-2 rounded-[8px] border p-2 ${edit.name === name ? 'border-[var(--spec-ink,#171717)]' : 'border-[var(--spec-card-border,#ececec)]'}`}>
            {thumbnails[name] ? (
              <img
                src={thumbnails[name]}
                alt={t(P + 'thumbnail', { filter: t(LABEL + name) })}
                className="h-16 w-full object-contain"
              />
            ) : (
              <span className="h-16" />
            )}
            <span className="flex items-center gap-2 text-[13px]">
              <input
                type="radio"
                name="document-filter"
                aria-label={t(LABEL + name)}
                value={name}
                checked={edit.name === name}
                onChange={() => change({ name })}
              />
              {t(LABEL + name)}
            </span>
          </label>
        ))}
      </fieldset>
      <div className="flex flex-wrap items-center gap-3">
        <label className="flex min-h-9 items-center gap-2 text-[13px]">
          {t(P + 'intensity')}
          <input
            type="range"
            aria-label={t(P + 'intensity')}
            min={0}
            max={100}
            step={1}
            disabled={locked || edit.name === 'original'}
            value={edit.intensity}
            onChange={(event) => change({ intensity: Number(event.target.value) })}
          />
          <span data-ui="document-tools.filter-strength">{edit.intensity}%</span>
        </label>
        <button type="button" className={BUTTON} disabled={locked || history.undo.length === 0} onClick={undo}>
          <Undo2 size={16} strokeWidth={1.7} aria-hidden />
          {t(P + 'undo')}
        </button>
        <button type="button" className={BUTTON} disabled={locked} onClick={() => change(ORIGINAL_FILTER)}>
          {t(P + 'restore')}
        </button>
      </div>
      <div className="flex flex-wrap gap-2">
        <label className="flex min-h-9 items-center gap-2 text-[13px]">
          {t(P + 'comparison')}
          <SearchSelect
            size="sm"
            searchable={false}
            allowClear={false}
            aria-label={t(P + 'comparison')}
            value={comparison}
            options={[
              { value: 'side-by-side', label: t(P + 'side_by_side') },
              { value: 'split', label: t(P + 'split') }
            ]}
            onChange={(next) => setComparison(parseFilterComparison(next))}
          />
        </label>
        <button
          type="button"
          className={BUTTON}
          aria-pressed={showOriginal}
          onClick={() => setShowOriginal((value) => !value)}>
          {t(P + 'show_original')}
        </button>
      </div>
      {!preview ? (
        <p role="status" className="text-[13px]">
          {t(P + (failure ? 'failed' : 'preparing'))}
        </p>
      ) : comparison === 'side-by-side' ? (
        <div className="grid min-w-0 gap-3 sm:grid-cols-2">
          <figure className="min-w-0">
            <figcaption className="mb-2 text-[13px]">{t(P + 'original')}</figcaption>
            <img
              src={sourceUrl}
              alt={t(P + 'source_image')}
              className="block max-h-[420px] w-full rounded-[8px] border border-[var(--spec-card-border,#ececec)] object-contain"
            />
          </figure>
          <figure className="min-w-0" data-ui="document-tools.filter-preview">
            <figcaption className="mb-2 text-[13px]">{t(P + (showOriginal ? 'original' : 'result'))}</figcaption>
            <img
              src={showOriginal ? sourceUrl : preview}
              alt={t(P + (showOriginal ? 'source_image' : 'result_image'))}
              className="block max-h-[420px] w-full rounded-[8px] border border-[var(--spec-card-border,#ececec)] object-contain"
            />
          </figure>
        </div>
      ) : (
        <figure className="min-w-0" data-ui="document-tools.filter-preview">
          <figcaption className="mb-2 text-[13px]">{t(P + 'split_hint')}</figcaption>
          <div
            className="relative mx-auto overflow-hidden rounded-[8px] border border-[var(--spec-card-border,#ececec)]"
            style={{ maxWidth: source?.width && source?.height ? (420 * source.width) / source.height : undefined }}>
            <img src={sourceUrl} alt={t(P + 'source_image')} className="block w-full" />
            {!showOriginal ? (
              <>
                <img
                  src={preview}
                  alt={t(P + 'result_image')}
                  className="absolute inset-0 h-full w-full"
                  style={{ clipPath: `inset(0 0 0 ${split}%)` }}
                />
                <span
                  className="pointer-events-none absolute top-0 bottom-0 w-0.5 bg-white"
                  style={{ left: `${split}%` }}
                  aria-hidden
                />
              </>
            ) : null}
          </div>
          <label className="mt-2 flex min-h-9 items-center gap-2 text-[13px]">
            {t(P + 'split_position')}
            <input
              type="range"
              min={0}
              max={100}
              value={split}
              onChange={(event) => setSplit(Number(event.target.value))}
            />
          </label>
        </figure>
      )}
      {sources.length > 1 ? (
        <label className="flex min-h-9 items-center gap-2 text-[13px]">
          <input
            type="checkbox"
            disabled={locked}
            checked={applyPages}
            onChange={(event) => setApplyPages(event.target.checked)}
          />
          {t(P + 'selected_pages', { count: selectedPages.size })}
        </label>
      ) : null}
      {batch ? (
        <label className="flex min-h-9 items-center gap-2 text-[13px]">
          <input
            type="checkbox"
            disabled={locked}
            checked={applyFiles}
            onChange={(event) => setApplyFiles(event.target.checked)}
          />
          {t(P + 'selected_files')}
        </label>
      ) : null}
      <button
        type="button"
        data-ui="document-tools.filter-apply"
        className={`${BUTTON} w-fit !bg-[var(--spec-ink,#171717)] !text-white`}
        disabled={locked || !preview || failure || targets.length === 0 || !needsApply}
        onClick={() => void apply()}>
        {t(P + 'apply')}
      </button>
      {failure ? (
        <p role="alert" className="text-[13px]">
          {t(P + 'failed')}
        </p>
      ) : null}
    </section>
  )
}
