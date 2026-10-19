import { Button, SearchSelect } from '@a3s-lab/ui'
import { ArrowDown, ArrowLeft, ArrowUp, Check, Download, Plus, RotateCcw, Trash2, X } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { PDF_TOOL_BYTE_LIMIT, PDF_TOOL_FILE_LIMIT, fileKindOf, intakeAccept, toolLabelKey } from '../pdfToolsJob'
import type { FilterName } from '../pdfToolsRaster'
import type { ToolOutput } from '../pdfToolsRun'
import type { DocumentTaskApplication, DocumentWriter } from './application'
import { LongImageOptionsPanel } from './AssemblyOptions'
import { FilterEditor, type FilterHistory } from './FilterEditor'
import { DEFAULT_LONG_IMAGE, type LongImageOptions } from './longImage'
import { OfficeExportOptions } from './OfficeExportOptions'
import { DEFAULT_OFFICE_EXPORT, type OfficeExportOptions as OfficeOptions } from './officeSvg'
import { OutputActions } from './OutputActions'
import { OutputPreview } from './OutputPreview'
import {
  DEFAULT_PDF_EXPORT,
  PDF_EXPORT_DPI,
  PDF_IMAGE_FORMATS,
  parsePdfExportDpi,
  parsePdfImageFormat,
  type PdfExportOptions
} from './pdfExport'
import { PdfRotationEditor, RasterEditor, rasterEditsEqual, type RasterEditorHistory } from './RasterEditor'
import { ORIGINAL_FILTER_DOCUMENT, type FilterDocumentEdit, type FilterEdit } from './rasterFilter'
import { ORIGINAL_RASTER_EDIT, type RasterEdit } from './rasterGeometry'
import type { DocumentInputFile } from './sourceFiles'
import { documentTaskProgress } from './task'
import type { DocumentTaskItem } from './task'
import { isDocumentConversion, isOfficeConversion, type DocumentArtifact } from './toolEngine'

const PREFIX = 'workbench.feature.pdf.tools.workspace.'
const BUTTON =
  'inline-flex h-9 items-center justify-center gap-1.5 rounded-[8px] border border-[var(--spec-button-border,#e5e5e5)] bg-white px-3 text-[13px] text-[var(--spec-ink,#171717)] hover:bg-[var(--spec-quiet-button-hover,#fafafa)] disabled:opacity-40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10'
const SOURCE_ACTION =
  'inline-flex size-6 p-0 shrink-0 items-center justify-center rounded-[6px] border border-[var(--spec-button-border,#e5e5e5)] bg-white text-[var(--spec-body,#3f3f3f)] hover:bg-[var(--spec-selected-row,#f2f3f5)] disabled:opacity-40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10'
const FILTERS: readonly FilterName[] = ['document', 'grayscale', 'contrast']

export function DocumentWorkspace({
  application,
  onBack,
  onFilter,
  onAddSources,
  onRemoveSources,
  onPdfExport,
  onOfficeExport,
  onLongImage,
  onGeometry,
  getRasterEdit,
  onApplyFilter,
  getFilterDocument,
  writer
}: {
  application: DocumentTaskApplication<DocumentArtifact>
  onBack: () => void
  onFilter: (filter: FilterName) => void
  writer?: DocumentWriter<DocumentArtifact>
  onAddSources?: (files: readonly DocumentInputFile[]) => void
  onRemoveSources?: (ids: readonly string[]) => void
  onPdfExport?: (options: PdfExportOptions) => void
  onOfficeExport?: (options: OfficeOptions) => void
  onLongImage?: (options: LongImageOptions) => void
  onGeometry?: (sourceId: string, edit: RasterEdit, ids: readonly string[]) => Promise<void>
  getRasterEdit?: (sourceId: string) => RasterEdit
  onApplyFilter?: (
    sourceId: string,
    edit: FilterEdit,
    pages: readonly number[],
    ids: readonly string[]
  ) => Promise<void>
  getFilterDocument?: (sourceId: string) => FilterDocumentEdit
}) {
  const { t } = useTranslation()
  const task = useSyncExternalStore(application.subscribe, application.getSnapshot, application.getSnapshot)
  const batch = task.items.length > 1
  const conversion = isDocumentConversion(task.kind)
  const office = isOfficeConversion(task.kind)
  const progress = documentTaskProgress(task)
  const saving = task.items.some((item) => item.saveState === 'saving')
  const busy = Boolean(task.attempt) || saving
  const [editorDrafts, setEditorDrafts] = useState<Readonly<Record<string, RasterEdit>>>({})
  const filterHistories = useRef(new Map<string, FilterHistory>())
  const [filterDrafts, setFilterDrafts] = useState<Readonly<Record<string, boolean>>>({})
  const editorHistories = useRef(new Map<string, RasterEditorHistory>())
  const inputRef = useRef<InstanceType<typeof globalThis.HTMLInputElement>>(null)
  const addButtonRef = useRef<InstanceType<typeof globalThis.HTMLButtonElement>>(null)
  const sourceListRef = useRef<InstanceType<typeof globalThis.HTMLUListElement>>(null)
  const knownIds = useRef(new Set(task.items.filter((item) => item.state !== 'rejected').map((item) => item.source.id)))
  const restoreSourceFocus = useRef(false)
  const [selectedId, setSelectedId] = useState(
    task.items.find((item) => item.state !== 'rejected')?.source.id ?? task.items[0]?.source.id
  )
  const [selected, setSelected] = useState(
    () => new Set(task.items.filter((item) => item.state !== 'rejected').map((item) => item.source.id))
  )
  const onFilterDraft = useCallback(
    (dirty: boolean) => {
      if (!selectedId) return
      setFilterDrafts((previous) => (previous[selectedId] === dirty ? previous : { ...previous, [selectedId]: dirty }))
    },
    [selectedId]
  )
  const [filter, setFilter] = useState<FilterName>('document')
  const [error, setError] = useState(false)
  const [pdfExport, setPdfExport] = useState(DEFAULT_PDF_EXPORT)
  const [officeExport, setOfficeExport] = useState(DEFAULT_OFFICE_EXPORT)
  const [longImage, setLongImage] = useState(DEFAULT_LONG_IMAGE)
  const changeOfficeExport = (options: OfficeOptions) => {
    if (busy || !onOfficeExport) return
    try {
      onOfficeExport(options)
      setOfficeExport(options)
      setError(false)
    } catch {
      setError(true)
    }
  }
  const changePdfExport = (options: Partial<PdfExportOptions>) => {
    if (busy || !onPdfExport) return
    try {
      const next = { ...pdfExport, ...options }
      onPdfExport(next)
      setPdfExport(next)
      setError(false)
    } catch {
      setError(true)
    }
  }
  useEffect(() => {
    for (const id of filterHistories.current.keys()) {
      if (!task.items.some((item) => item.source.id === id)) filterHistories.current.delete(id)
    }
    setFilterDrafts((previous) => {
      const entries = Object.entries(previous).filter(([id]) => task.items.some((item) => item.source.id === id))
      return entries.length === Object.keys(previous).length ? previous : Object.fromEntries(entries)
    })
    for (const id of editorHistories.current.keys()) {
      if (!task.items.some((item) => item.source.id === id)) editorHistories.current.delete(id)
    }
    setEditorDrafts((previous) => {
      const entries = Object.entries(previous).filter(([id]) => task.items.some((item) => item.source.id === id))
      return entries.length === Object.keys(previous).length ? previous : Object.fromEntries(entries)
    })
    const previousKnown = knownIds.current
    setSelected(
      (previous) =>
        new Set(
          task.items
            .filter(
              (item) =>
                item.state !== 'rejected' && (previous.has(item.source.id) || !previousKnown.has(item.source.id))
            )
            .map((item) => item.source.id)
        )
    )
    knownIds.current = new Set(task.items.filter((item) => item.state !== 'rejected').map((item) => item.source.id))
    if (!task.items.some((item) => item.source.id === selectedId)) {
      setSelectedId(task.items.find((item) => item.state !== 'rejected')?.source.id ?? task.items[0]?.source.id)
    }
  }, [task.items, selectedId])
  useEffect(() => {
    if (!restoreSourceFocus.current) return
    const target = [
      ...(sourceListRef.current?.querySelectorAll<InstanceType<typeof globalThis.HTMLButtonElement>>(
        '[data-source-id]'
      ) ?? [])
    ].find((button) => button.dataset.sourceId === selectedId)
    const focusTarget = target ?? addButtonRef.current
    focusTarget?.focus()
    restoreSourceFocus.current = false
  }, [selectedId, task.items])
  const onEditorDraft = useCallback(
    (edit: RasterEdit) => {
      if (!selectedId) return
      setEditorDrafts((old) =>
        rasterEditsEqual(old[selectedId] ?? ORIGINAL_RASTER_EDIT, edit) ? old : { ...old, [selectedId]: edit }
      )
    },
    [selectedId]
  )
  const admittedIds = task.items.filter((item) => item.state !== 'rejected').map((item) => item.source.id)
  const allSelected = admittedIds.length > 0 && admittedIds.every((id) => selected.has(id))
  const addSources = (files: readonly DocumentInputFile[]) => {
    if (busy || files.length === 0 || !onAddSources) return
    try {
      onAddSources(files)
      setError(false)
    } catch {
      setError(true)
    }
  }
  const removeSources = (ids: readonly string[]) => {
    if (busy || ids.length === 0 || !onRemoveSources) return
    try {
      const remaining = task.items.filter((item) => !ids.includes(item.source.id))
      onRemoveSources(ids)
      restoreSourceFocus.current = true
      if (ids.includes(selectedId ?? ''))
        setSelectedId(remaining.find((item) => item.state !== 'rejected')?.source.id ?? remaining[0]?.source.id)
      setError(false)
    } catch {
      setError(true)
    }
  }
  const moveSource = (id: string, offset: number) => {
    const order = task.items.map((item) => item.source.id)
    const index = order.indexOf(id)
    if (busy || index < 0 || index + offset < 0 || index + offset >= order.length) return
    ;[order[index], order[index + offset]] = [order[index + offset], order[index]]
    try {
      application.reorder(order)
      setError(false)
    } catch {
      setError(true)
    }
  }
  const current = task.items.find((item) => item.source.id === selectedId)
  const retryIds = task.items
    .filter((item) => selected.has(item.source.id) && ['ready', 'failed', 'cancelled'].includes(item.state))
    .map((item) => item.source.id)
  const run = () => {
    setError(false)
    void application.run(retryIds).catch(() => setError(true))
  }

  return (
    <section
      data-ui="document-tools.workspace"
      data-state={task.attempt ? 'running' : 'idle'}
      aria-label={t(toolLabelKey(task.kind))}
      onDragOver={(event) => {
        if (event.dataTransfer.types.includes('Files')) event.preventDefault()
      }}
      onDrop={(event) => {
        event.preventDefault()
        addSources([...event.dataTransfer.files])
      }}
      onPaste={(event) => {
        const target = event.target
        if (target instanceof globalThis.Element && target.closest('input, textarea, [contenteditable="true"]')) return
        const files = [...event.clipboardData.files]
        if (files.length > 0) {
          event.preventDefault()
          addSources(files)
        }
      }}
      className="mx-auto flex min-h-full w-full max-w-[920px] flex-col gap-4 p-4 text-[var(--spec-ink,#171717)] sm:p-6">
      <header className="flex flex-wrap items-center gap-3">
        <Button variant="ghost" type="button" className={BUTTON} onClick={onBack} aria-label={t(PREFIX + 'back')}>
          <ArrowLeft size={16} strokeWidth={1.7} aria-hidden />
        </Button>
        <h2 className="min-w-0 flex-1 text-[24px] leading-8 font-bold">{t(toolLabelKey(task.kind))}</h2>
        {onAddSources ? (
          <Button
            variant="ghost"
            ref={addButtonRef}
            type="button"
            className={BUTTON}
            disabled={busy}
            onClick={() => inputRef.current?.click()}>
            <Plus size={16} strokeWidth={1.7} aria-hidden />
            {t(PREFIX + 'add_sources')}
          </Button>
        ) : null}
        {task.attempt ? (
          <Button
            variant="ghost"
            type="button"
            className={BUTTON}
            onClick={() => application.cancel()}
            disabled={progress.cancelling > 0}>
            <X size={16} strokeWidth={1.7} aria-hidden />
            {t(PREFIX + (progress.cancelling ? 'cancelling' : 'cancel'))}
          </Button>
        ) : retryIds.length > 0 ? (
          <Button variant="ghost" type="button" className={BUTTON} disabled={saving} onClick={run}>
            <RotateCcw size={16} strokeWidth={1.7} aria-hidden />
            {t(PREFIX + (batch ? 'retry_selected' : current?.state === 'failed' ? 'retry' : 'process'))}
          </Button>
        ) : null}
      </header>
      {onAddSources ? (
        <input
          ref={inputRef}
          type="file"
          multiple
          accept={intakeAccept(task.kind).accept}
          className="sr-only"
          data-ui="document-tools.add-input"
          onChange={(event) => {
            const files = [...(event.target.files ?? [])]
            event.target.value = ''
            addSources(files)
          }}
        />
      ) : null}
      <p
        role="status"
        aria-live="polite"
        className="text-[13px] text-[var(--spec-body,#3f3f3f)]"
        data-ui="document-tools.progress">
        {task.attempt || batch
          ? t(PREFIX + 'progress', {
              completed: progress.completed,
              total: progress.total,
              failed: progress.failed
            })
          : current?.state === 'accepted'
            ? t(PREFIX + 'completed')
            : null}
      </p>
      {error ? <p role="alert">{t('workbench.feature.pdf.tools.job.failed')}</p> : null}
      {task.attempt ? (
        <progress
          className="h-1.5 w-full accent-[var(--spec-ink,#171717)]"
          aria-label={t(PREFIX + 'processing')}
          value={batch ? progress.completed : undefined}
          max={Math.max(1, progress.total)}
        />
      ) : null}
      <div className="flex min-h-0 flex-col gap-4">
        {batch ? (
          <details open className="rounded-[8px] border border-[var(--spec-card-border,#ececec)]">
            <summary className="cursor-pointer px-4 py-3 text-[13px]">
              {t(PREFIX + 'sources_count', { count: task.items.length })}
            </summary>
            <aside
              className="min-w-0 rounded-[8px] border border-[var(--spec-card-border,#ececec)] p-2"
              aria-label={t(PREFIX + 'sources')}>
              <h3 className="px-2 py-2 text-[14px] font-medium">{t(PREFIX + 'sources')}</h3>
              <p className="px-2 py-2 text-[12px] text-[var(--spec-body,#3f3f3f)]">
                {t(PREFIX + 'limits', {
                  count: PDF_TOOL_FILE_LIMIT,
                  size: PDF_TOOL_BYTE_LIMIT / (1024 * 1024)
                })}
              </p>
              <div className="mb-2 flex flex-wrap items-center gap-2 px-2">
                <label className="flex min-h-9 items-center gap-2 text-[13px]">
                  <input
                    type="checkbox"
                    className="size-4 accent-[var(--spec-ink,#171717)]"
                    checked={allSelected}
                    disabled={busy || admittedIds.length === 0}
                    onChange={() => setSelected(allSelected ? new Set() : new Set(admittedIds))}
                  />
                  {t(PREFIX + 'select_all')}
                </label>
                {onRemoveSources ? (
                  <Button
                    variant="ghost"
                    type="button"
                    className={BUTTON}
                    disabled={busy || selected.size === 0}
                    onClick={() => removeSources([...selected])}>
                    {t(PREFIX + 'remove_selected')}
                  </Button>
                ) : null}
              </div>
              <ul ref={sourceListRef} className="flex flex-col gap-1">
                {task.items.map((item, index) => (
                  <li
                    key={item.source.id}
                    data-source-state={item.state}
                    className={`flex min-w-0 items-start gap-2 rounded-[8px] p-2 ${selectedId === item.source.id ? 'bg-[var(--spec-selected-row,#f2f3f5)]' : ''}`}>
                    <input
                      type="checkbox"
                      className="mt-1 size-4 accent-[var(--spec-ink,#171717)]"
                      aria-label={t(PREFIX + 'select_source', { name: item.source.name })}
                      checked={selected.has(item.source.id)}
                      disabled={busy || item.state === 'rejected'}
                      onChange={(event) => {
                        const next = new Set(selected)
                        if (event.target.checked) next.add(item.source.id)
                        else next.delete(item.source.id)
                        setSelected(next)
                      }}
                    />
                    <div className="min-w-0 flex-1">
                      <Button
                        variant="ghost"
                        type="button"
                        className="min-h-9 w-full min-w-0 flex-1 rounded-[4px] text-left focus-visible:outline-2 focus-visible:outline-offset-2"
                        data-source-id={item.source.id}
                        aria-pressed={selectedId === item.source.id}
                        aria-label={`${item.source.name} ${t(PREFIX + 'state.' + item.state)}`}
                        onClick={() => setSelectedId(item.source.id)}>
                        <span className="block text-[13px] break-words">{item.source.name}</span>
                        <span className="mt-0.5 block text-[12px] text-[var(--spec-body,#3f3f3f)]">
                          {t(PREFIX + 'state.' + item.state)}
                        </span>
                      </Button>
                      <div className="mt-1 flex gap-1">
                        <Button
                          variant="ghost"
                          type="button"
                          className={SOURCE_ACTION}
                          aria-label={t(PREFIX + 'move_up', { name: item.source.name })}
                          data-action="move-up"
                          disabled={busy || index === 0}
                          onClick={() => moveSource(item.source.id, -1)}>
                          <ArrowUp size={14} strokeWidth={1.7} aria-hidden />
                        </Button>
                        <Button
                          variant="ghost"
                          type="button"
                          className={SOURCE_ACTION}
                          aria-label={t(PREFIX + 'move_down', { name: item.source.name })}
                          data-action="move-down"
                          disabled={busy || index === task.items.length - 1}
                          onClick={() => moveSource(item.source.id, 1)}>
                          <ArrowDown size={14} strokeWidth={1.7} aria-hidden />
                        </Button>
                        {onRemoveSources ? (
                          <Button
                            variant="ghost"
                            type="button"
                            className={SOURCE_ACTION}
                            aria-label={t(PREFIX + 'remove_source', { name: item.source.name })}
                            data-action="remove"
                            disabled={busy}
                            onClick={() => removeSources([item.source.id])}>
                            <Trash2 size={14} strokeWidth={1.7} aria-hidden />
                          </Button>
                        ) : null}
                      </div>
                    </div>
                  </li>
                ))}
              </ul>
            </aside>
          </details>
        ) : null}
        <main
          data-source-state={batch ? undefined : current?.state}
          className="min-w-0"
          aria-label={t(PREFIX + 'preview')}>
          {current ? (
            <CandidateReview
              item={current}
              application={application}
              writer={writer}
              conversion={conversion}
              editor={
                task.kind === 'rotate' && onGeometry && (current.candidate ?? current.accepted) ? (
                  (() => {
                    const source = (current.candidate ?? current.accepted)?.value.preview?.sourceFiles?.[0]
                    const apply = (edit: RasterEdit, anglesToSelected: boolean) =>
                      onGeometry(
                        current.source.id,
                        edit,
                        anglesToSelected
                          ? [
                              ...new Set([
                                current.source.id,
                                ...task.items
                                  .filter(
                                    (item) =>
                                      selected.has(item.source.id) &&
                                      item.state !== 'rejected' &&
                                      fileKindOf(item.source) === 'image'
                                  )
                                  .map((item) => item.source.id)
                              ])
                            ]
                          : [current.source.id]
                      )
                    return source ? (
                      <RasterEditor
                        key={current.source.id}
                        id={current.source.id}
                        source={source}
                        histories={editorHistories.current}
                        busy={busy}
                        batch={batch}
                        committed={getRasterEdit?.(current.source.id) ?? ORIGINAL_RASTER_EDIT}
                        accepted={current.state === 'accepted'}
                        onApply={apply}
                        onDraft={onEditorDraft}
                      />
                    ) : fileKindOf(current.source) === 'pdf' ? (
                      <PdfRotationEditor
                        key={current.source.id}
                        id={current.source.id}
                        histories={editorHistories.current}
                        busy={busy}
                        onApply={apply}
                        onDraft={onEditorDraft}
                      />
                    ) : undefined
                  })()
                ) : task.kind === 'filter' &&
                  onApplyFilter &&
                  (current.candidate ?? current.accepted)?.value.preview?.sourceFiles?.length ? (
                  <FilterEditor
                    key={current.source.id}
                    id={current.source.id}
                    sources={(current.candidate ?? current.accepted)?.value.preview?.sourceFiles ?? []}
                    committed={getFilterDocument?.(current.source.id) ?? ORIGINAL_FILTER_DOCUMENT}
                    accepted={current.state === 'accepted'}
                    histories={filterHistories.current}
                    busy={busy}
                    batch={batch}
                    onDraft={onFilterDraft}
                    onApply={async (edit, pages, selectedFiles) => {
                      const ids = selectedFiles
                        ? [
                            ...new Set([
                              current.source.id,
                              ...task.items
                                .filter((item) => selected.has(item.source.id) && item.state !== 'rejected')
                                .map((item) => item.source.id)
                            ])
                          ]
                        : [current.source.id]
                      await onApplyFilter(current.source.id, edit, pages, ids)
                      for (const id of ids) if (id !== current.source.id) filterHistories.current.delete(id)
                      setFilterDrafts((previous) => ({
                        ...previous,
                        ...Object.fromEntries(ids.filter((id) => id !== current.source.id).map((id) => [id, false]))
                      }))
                    }}
                  />
                ) : undefined
              }
              hideOutput={
                (task.kind === 'rotate' && fileKindOf(current.source) === 'image') ||
                (task.kind === 'filter' && Boolean(onApplyFilter))
              }
              hideAcceptedActions={
                (task.kind === 'filter' && Boolean(filterDrafts[current.source.id])) ||
                (task.kind === 'rotate' &&
                  Boolean(editorDrafts[current.source.id]) &&
                  !rasterEditsEqual(
                    editorDrafts[current.source.id],
                    getRasterEdit?.(current.source.id) ?? ORIGINAL_RASTER_EDIT
                  ))
              }
            />
          ) : (
            <p>{t(PREFIX + 'empty')}</p>
          )}
        </main>
        {(task.kind === 'filter' && !onApplyFilter) ||
        (onPdfExport && (task.kind === 'pdf-to-image' || task.kind === 'pdf-to-long')) ||
        (office && onOfficeExport) ? (
          <details
            open={task.kind === 'filter'}
            className="rounded-[8px] border border-[var(--spec-card-border,#ececec)] p-4">
            <summary className="cursor-pointer text-[13px] font-medium">{t(PREFIX + 'options')}</summary>
            {task.kind === 'pdf-to-long' && onLongImage ? (
              <LongImageOptionsPanel
                value={longImage}
                busy={busy}
                format={false}
                onChange={(options) => {
                  if (!busy) {
                    onLongImage(options)
                    setLongImage(options)
                  }
                }}
              />
            ) : null}
            {office && onOfficeExport ? (
              <OfficeExportOptions
                value={officeExport}
                printLayout={task.kind === 'word-to-pdf'}
                spreadsheet={task.kind === 'excel-to-pdf' || task.kind === 'excel-to-image'}
                image={task.kind === 'excel-to-image' || task.kind === 'word-to-image'}
                busy={busy}
                onChange={changeOfficeExport}
              />
            ) : null}
            {onPdfExport && (task.kind === 'pdf-to-image' || task.kind === 'pdf-to-long') ? (
              <fieldset className="mt-4 flex flex-col gap-4 text-[13px]" disabled={busy}>
                <legend className="mb-2 font-medium">{t(PREFIX + 'pdf.export')}</legend>
                <label className="flex flex-col gap-1.5">
                  {t(PREFIX + 'pdf.pages')}
                  <input
                    type="text"
                    maxLength={1024}
                    value={pdfExport.pageRange}
                    placeholder={t(PREFIX + 'pdf.pages_hint')}
                    data-ui="document-tools.pdf-pages"
                    className="h-9 min-w-0 rounded-[8px] border border-[var(--spec-button-border,#e5e5e5)] px-2 focus-visible:outline-2"
                    onChange={(event) => changePdfExport({ pageRange: event.target.value })}
                  />
                </label>
                <p className="text-[12px]">{t(PREFIX + 'pdf.order_hint')}</p>
                <label className="flex flex-col gap-1.5">
                  {t(PREFIX + 'pdf.resolution')}
                  <SearchSelect
                    searchable={false}
                    allowClear={false}
                    data-ui="document-tools.pdf-dpi"
                    aria-label={t(PREFIX + 'pdf.resolution')}
                    value={String(pdfExport.dpi)}
                    options={PDF_EXPORT_DPI.map((dpi) => ({ value: String(dpi), label: `${dpi} DPI` }))}
                    onChange={(next) => changePdfExport({ dpi: parsePdfExportDpi(Number(next)) })}
                  />
                </label>
                <label className="flex flex-col gap-1.5">
                  {t(PREFIX + 'pdf.format')}
                  <SearchSelect
                    searchable={false}
                    allowClear={false}
                    data-ui="document-tools.pdf-format"
                    aria-label={t(PREFIX + 'pdf.format')}
                    value={pdfExport.imageFormat}
                    options={PDF_IMAGE_FORMATS.map((format) => ({
                      value: format,
                      label: format === 'jpeg' ? 'JPEG' : 'PNG'
                    }))}
                    onChange={(next) => changePdfExport({ imageFormat: parsePdfImageFormat(next) })}
                  />
                </label>
                <p className="text-[12px]">{t(PREFIX + 'pdf.apply_hint')}</p>
              </fieldset>
            ) : null}
            {task.kind === 'filter' ? (
              <fieldset className="mt-4" disabled={Boolean(task.attempt) || saving}>
                <legend className="mb-2 text-[13px]">{t(PREFIX + 'filter_all')}</legend>
                <div className="flex flex-col gap-2">
                  {FILTERS.map((value) => (
                    <label key={value} className="flex min-h-9 items-center gap-2 text-[13px]">
                      <input
                        type="radio"
                        className="size-4 accent-[var(--spec-ink,#171717)]"
                        name="document-filter"
                        value={value}
                        checked={filter === value}
                        onChange={() => {
                          setFilter(value)
                          onFilter(value)
                        }}
                      />
                      {t(PREFIX + 'filter.' + value)}
                    </label>
                  ))}
                </div>
              </fieldset>
            ) : null}
          </details>
        ) : null}
      </div>
    </section>
  )
}

function CandidateReview({
  item,
  application,
  writer,
  conversion,
  editor,
  hideOutput,
  hideAcceptedActions
}: {
  conversion: boolean
  editor?: ReactNode
  hideOutput?: boolean
  hideAcceptedActions?: boolean
  item: DocumentTaskItem<DocumentArtifact>
  application: DocumentTaskApplication<DocumentArtifact>
  writer?: DocumentWriter<DocumentArtifact>
}) {
  const { t } = useTranslation()
  const candidate = item.candidate ?? item.accepted
  const files = candidate?.value.preview?.files ?? candidate?.value.output.files ?? []
  const links = useOutputUrls(candidate?.value.output.files ?? [])
  const previews = useOutputUrls(files)
  const [previewIndex, setPreviewIndex] = useState(0)
  useEffect(() => {
    setPreviewIndex(0)
  }, [candidate])
  const shown = previews[Math.min(previewIndex, Math.max(0, previews.length - 1))]
  return (
    <div className="flex min-w-0 flex-col gap-4" data-ui="document-tools.review" data-source={item.source.id}>
      <h3 className="text-[14px] font-medium break-words">{item.source.name}</h3>
      {candidate && candidate.revision !== application.getSnapshot().revision ? (
        <p role="status" className="text-[13px]">
          {t(PREFIX + 'previous_result')}
        </p>
      ) : null}
      {!(conversion && item.state === 'accepted') ? (
        <p
          data-ui="document-tools.review.reason"
          role={item.failure ? 'alert' : 'status'}
          className="text-[13px] text-[var(--spec-body,#3f3f3f)]">
          {item.refusal
            ? t(PREFIX + 'refusal.' + item.refusal)
            : item.failure
              ? t(item.failure)
              : hideAcceptedActions
                ? t(PREFIX + 'geometry.unapplied')
                : t(PREFIX + (conversion && item.state === 'accepted' ? 'completed' : 'state.' + item.state))}
        </p>
      ) : null}
      {item.state === 'accepted' && !hideAcceptedActions ? (
        <div className="flex flex-wrap items-center gap-2">
          {item.saveState !== 'unsaved' || !conversion ? (
            <p role="status" className="text-[13px]" data-ui="document-tools.save-state">
              {t(PREFIX + 'save.' + item.saveState)}
            </p>
          ) : null}
          {writer && !conversion ? (
            <Button
              variant="ghost"
              type="button"
              className={`${BUTTON} w-fit`}
              disabled={item.saveState === 'saving'}
              onClick={() => {
                void application.save(item.source.id, writer)
              }}>
              <Download size={16} strokeWidth={1.7} aria-hidden />
              {t(PREFIX + 'save_as')}
            </Button>
          ) : null}
          {links.map(({ file, url }) => (
            <OutputActions key={file.name} file={file} url={url} />
          ))}
          {item.receipt ? <p className="text-[12px] break-words">{item.receipt.destination}</p> : null}
        </div>
      ) : null}
      {candidate?.value.output.note ? (
        <p className="text-[12px]" data-ui="document-tools.output-note">
          {t(candidate.value.output.note)}
        </p>
      ) : null}
      {editor}
      {!hideOutput && previews.length > 1 ? (
        <nav aria-label={t(PREFIX + 'outputs')} className="flex flex-wrap gap-2">
          {previews.map(({ file }, index) => (
            <Button
              variant="ghost"
              key={file.name}
              type="button"
              className={BUTTON}
              aria-pressed={previewIndex === index}
              onClick={() => setPreviewIndex(index)}>
              {file.name}
            </Button>
          ))}
        </nav>
      ) : null}
      {(!hideOutput && shown ? [shown] : []).map(({ file, url }) => (
        <div key={file.name} className="min-w-0">
          <p className="mb-2 text-[12px] break-words" data-ui="document-tools.output-details">
            {file.name}
            {file.width && file.height ? ` · ${file.width} × ${file.height} px` : ''}
            {file.sourcePage ? ` · ${t(PREFIX + 'source_page', { page: file.sourcePage })}` : ''}
            {file.previewPage
              ? ` · ${t('workbench.feature.pdf.tools.workspace.office.preview_page', { page: file.previewPage })}`
              : ''}
            {file.sourceSheet
              ? ` · ${t('workbench.feature.pdf.tools.workspace.office.source_sheet', { sheet: file.sourceSheet })}`
              : ''}
          </p>
          <OutputPreview file={file} url={url} />
        </div>
      ))}
      {item.state === 'review' && item.candidate && !conversion && !editor ? (
        <Button
          variant="ghost"
          type="button"
          data-ui="document-tools.accept"
          className={`${BUTTON} w-fit`}
          onClick={() => {
            if (item.candidate) application.accept(item.source.id, item.candidate)
          }}>
          <Check size={16} strokeWidth={1.7} aria-hidden />
          {t(PREFIX + 'accept')}
        </Button>
      ) : null}
    </div>
  )
}

function useOutputUrls(files: readonly ToolOutput[]) {
  const links = useMemo(
    () =>
      files.map((file) => ({
        file,
        url: globalThis.URL.createObjectURL(new globalThis.Blob([file.bytes.slice()], { type: file.mediaType }))
      })),
    [files]
  )
  useEffect(
    () => () => {
      for (const link of links) globalThis.URL.revokeObjectURL(link.url)
    },
    [links]
  )
  return links
}
