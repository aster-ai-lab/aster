import { Button } from '@a3s-lab/ui'
import {
  Check,
  ArrowUp,
  ArrowDown,
  ChevronRight,
  Crop,
  Droplet,
  Eraser,
  FilePen,
  Mic,
  PenLine,
  Plus,
  ScanLine,
  ScanText,
  SlidersHorizontal,
  StretchHorizontal
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import {
  DocumentFileIcon,
  documentFileIconType,
  type DocumentFileIconType
} from '@renderer/components/icons/DocumentFileIcon'
import { documentServiceFetch } from '@renderer/data/documentServiceTransport'
import { PERSONAL_CENTER, readCenterId } from '@renderer/data/memberOrgs'
import { osGatewayOrigin } from '@renderer/data/osGateway'
import { driveApi } from '@renderer/drive/driveTransport'

import { AppletPermissionGate } from './AppletPermissionDialog'
import { AppletWindowBar } from './AppletWindowBar'
import { DocumentTaskApplication } from './documentTools/application'
import { documentArtifactWriter } from './documentTools/artifactWriter'
import { LongImageOptionsPanel, ImagePdfOptionsPanel } from './documentTools/AssemblyOptions'
import { DocumentWorkspace } from './documentTools/DocumentWorkspace'
import { DEFAULT_IMAGE_PDF, type ImagePdfOptions } from './documentTools/imagePdf'
import { DEFAULT_LONG_IMAGE, type LongImageOptions } from './documentTools/longImage'
import { DEFAULT_OFFICE_EXPORT } from './documentTools/officeSvg'
import { OutputActions } from './documentTools/OutputActions'
import { OutputPreview } from './documentTools/OutputPreview'
import { DEFAULT_PDF_EXPORT } from './documentTools/pdfExport'
import { rasterEditsEqual } from './documentTools/RasterEditor'
import {
  ORIGINAL_FILTER_DOCUMENT,
  filterDocumentsEqual,
  validateFilterDocument,
  validateFilterEdit,
  type FilterEdit
} from './documentTools/rasterFilter'
import { ORIGINAL_RASTER_EDIT, validateRasterEdit, type RasterEdit } from './documentTools/rasterGeometry'
import { RASTER_OUTPUT_BYTE_LIMIT } from './documentTools/rasterLimits'
import { DocumentSourceFiles, type DocumentInputFile } from './documentTools/sourceFiles'
import { createDocumentTask } from './documentTools/task'
import {
  documentToolEngine,
  isDocumentConversion,
  usesDocumentBatch,
  type DocumentArtifact,
  type DocumentToolOptions
} from './documentTools/toolEngine'
import {
  failureKey,
  gapKey,
  intake,
  intakeAccept,
  isPdfToolKind,
  takePdfToolKind,
  toolLabelKey,
  type PdfToolKind,
  type ToolFile
} from './pdfToolsJob'
import type { FilterName } from './pdfToolsRaster'
import {
  pdfToolFailureReason,
  runPdfToolJob,
  type PdfToolRunResult,
  type ToolBytes,
  type ToolOutput
} from './pdfToolsRun'

const FOCUS = 'focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-black/10'

const DROP_TYPES = ['excel', 'word', 'pdf', 'image'] as const satisfies readonly DocumentFileIconType[]

const EDIT_CHIPS: readonly { key: string; kind: PdfToolKind; icon: LucideIcon }[] = [
  { key: 'workbench.feature.pdf.tools.chip.extract', kind: 'extract', icon: ScanText },
  { key: 'workbench.feature.pdf.tools.chip.rotate', kind: 'rotate', icon: Crop },
  { key: 'workbench.feature.pdf.tools.chip.filter', kind: 'filter', icon: SlidersHorizontal },
  { key: 'workbench.feature.pdf.tools.chip.handwriting', kind: 'handwriting', icon: PenLine },
  { key: 'workbench.feature.pdf.tools.chip.watermark', kind: 'watermark', icon: Droplet },
  { key: 'workbench.feature.pdf.tools.chip.erase', kind: 'erase', icon: Eraser },
  { key: 'workbench.feature.pdf.tools.chip.merge', kind: 'merge', icon: StretchHorizontal }
]

const TOOL_GROUPS: readonly {
  title: string
  items: readonly { key: string; kind: PdfToolKind; type: DocumentFileIconType }[]
}[] = [
  {
    title: 'workbench.feature.pdf.tools.pdf_group',
    items: [
      { key: 'workbench.feature.pdf.tools.item.pdf_to_word', kind: 'pdf-to-word', type: 'word' },
      { key: 'workbench.feature.pdf.tools.item.pdf_to_excel', kind: 'pdf-to-excel', type: 'excel' },
      { key: 'workbench.feature.pdf.tools.item.pdf_to_image', kind: 'pdf-to-image', type: 'image' },
      { key: 'workbench.feature.pdf.tools.item.pdf_to_long', kind: 'pdf-to-long', type: 'image' }
    ]
  },
  {
    title: 'workbench.feature.pdf.tools.office_group',
    items: [
      { key: 'workbench.feature.pdf.tools.item.word_to_pdf', kind: 'word-to-pdf', type: 'pdf' },
      { key: 'workbench.feature.pdf.tools.item.word_to_image', kind: 'word-to-image', type: 'image' },
      { key: 'workbench.feature.pdf.tools.item.excel_to_pdf', kind: 'excel-to-pdf', type: 'pdf' },
      { key: 'workbench.feature.pdf.tools.item.excel_to_image', kind: 'excel-to-image', type: 'image' }
    ]
  },
  {
    title: 'workbench.feature.pdf.tools.image_group',
    items: [
      { key: 'workbench.feature.pdf.tools.item.image_to_word', kind: 'image-to-word', type: 'word' },
      { key: 'workbench.feature.pdf.tools.item.image_to_excel', kind: 'image-to-excel', type: 'excel' },
      { key: 'workbench.feature.pdf.tools.item.image_to_pdf', kind: 'image-to-pdf', type: 'pdf' },
      { key: 'workbench.feature.pdf.tools.item.merge_long', kind: 'merge', type: 'image' }
    ]
  }
]

type SavedFile = {
  url: string
  filename: string
  bytes: Uint8Array
  mediaType: string
  width?: number
  height?: number
}

type JobSource = { name: string; size: number }

type JobView =
  | { id: number; state: 'choose'; files: File[]; kinds: PdfToolKind[] }
  | { id: number; state: 'running'; kind: PdfToolKind; partial?: string; cancelling?: boolean; sources: JobSource[] }
  | { id: number; state: 'configured'; kind: PdfToolKind; sources: JobSource[] }
  | {
      id: number
      state: 'preview'
      kind: PdfToolKind
      files: SavedFile[]
      previews?: { filter: FilterName; url: string }[]
      note?: string
      confirmation: Extract<PdfToolRunResult, { state: 'done' }>
    }
  | {
      id: number
      state: 'done'
      kind: PdfToolKind
      files: SavedFile[]
      note?: string
      label?: string
      copy?: string
      editor?: 'pdf'
      dpi?: number
    }
  | { id: number; state: 'failed'; kind: PdfToolKind | null; reason: string; sources: JobSource[] }

export function WorkbenchPdfToolsWindow({
  initialKind = null,
  initialFiles = null,
  onDismiss
}: {
  initialKind?: string | null
  initialFiles?: readonly File[] | null
  onDismiss?: () => void
}) {
  const { t } = useTranslation()
  const stagedKind = useRef(isPdfToolKind(initialKind) ? initialKind : takePdfToolKind())
  const skipPicker = useRef(Boolean(initialFiles && initialFiles.length > 0))
  const title = t('workbench.feature.pdf.tools.title')
  const [epoch, setEpoch] = useState(0)
  const [autoCrop, setAutoCrop] = useState(true)
  const [job, setJob] = useState<JobView | null>(null)
  const [documentApplication, setDocumentApplication] = useState<DocumentTaskApplication<DocumentArtifact> | null>(null)
  const documentApplicationRef = useRef<DocumentTaskApplication<DocumentArtifact> | null>(null)
  const documentFilesRef = useRef<DocumentSourceFiles | null>(null)
  const documentOptionsRef = useRef<DocumentToolOptions>({})
  const [returned, setReturned] = useState(false)
  const reload = () => setEpoch((value) => value + 1)
  const inputRef = useRef<HTMLInputElement>(null)
  const openedInitial = useRef(false)
  const pendingKind = useRef<PdfToolKind | null>(null)
  const abortRef = useRef<AbortController | null>(null)
  const urlsRef = useRef<string[]>([])
  const jobId = useRef(0)
  const filesRef = useRef<File[]>([])
  const [filterPick, setFilterPick] = useState<FilterName>('document')
  const [assemblyLong, setAssemblyLong] = useState(DEFAULT_LONG_IMAGE)
  const [assemblyPdf, setAssemblyPdf] = useState(DEFAULT_IMAGE_PDF)
  const assemblyOptionsRef = useRef({ longImage: DEFAULT_LONG_IMAGE, imagePdf: DEFAULT_IMAGE_PDF })
  const autoCropRef = useRef(autoCrop)
  autoCropRef.current = autoCrop

  useEffect(() => {
    if (!onDismiss) return
    setMainTrafficLights(true)
    return () => setMainTrafficLights(false)
  }, [onDismiss])

  useEffect(() => {
    document.getElementById('spinner')?.remove()
    const previous = document.title
    document.title = title
    return () => {
      document.title = previous
    }
  }, [title])

  useEffect(
    () => () => {
      abortRef.current?.abort()
      documentApplicationRef.current?.dispose()
      for (const url of urlsRef.current) URL.revokeObjectURL(url)
    },
    []
  )

  const armInput = (input: HTMLInputElement, kind: PdfToolKind | null) => {
    pendingKind.current = kind
    const spec = intakeAccept(kind)
    input.accept = spec.accept
    input.multiple = spec.multiple
  }

  const pick = (kind: PdfToolKind | null) => {
    const input = inputRef.current
    if (!input) return
    armInput(input, kind)
    input.click()
  }

  const bindInput = (node: HTMLInputElement | null) => {
    inputRef.current = node
    const kind = stagedKind.current
    if (!node || !kind || openedInitial.current || skipPicker.current) return
    openedInitial.current = true
    armInput(node, kind)
    node.click()
  }

  const take = (kind: PdfToolKind | null, list: File[]) => {
    const nextId = () => {
      jobId.current += 1
      return jobId.current
    }
    const sources = jobSources(list)
    documentApplicationRef.current?.dispose()
    documentApplicationRef.current = null
    documentFilesRef.current = null
    setDocumentApplication(null)
    if (kind && usesDocumentBatch(kind)) {
      abortRef.current?.abort()
      const id = nextId()
      filesRef.current = []
      const sourceFiles = new DocumentSourceFiles(async (file) => (await readToolBytes([file]))[0])
      const task = createDocumentTask<DocumentArtifact>(`document-${id}`, kind, sourceFiles.add(list))
      documentOptionsRef.current = {
        autoCrop: false,
        pdfExport: DEFAULT_PDF_EXPORT,
        longImage: DEFAULT_LONG_IMAGE,
        officeExport: DEFAULT_OFFICE_EXPORT,
        filterDocument: kind === 'filter' ? ORIGINAL_FILTER_DOCUMENT : undefined,
        rasterEdit: kind === 'rotate' ? ORIGINAL_RASTER_EDIT : undefined,
        gatewayBase: osGatewayOrigin(),
        fetchImpl: documentServiceFetch(osGatewayOrigin())
      }
      const application = new DocumentTaskApplication(
        task,
        documentToolEngine(kind, sourceFiles.load, documentOptionsRef.current),
        isDocumentConversion(kind)
      )
      documentFilesRef.current = sourceFiles
      documentApplicationRef.current = application
      setDocumentApplication(application)
      setJob(null)
      const selected = task.items.filter((item) => item.state === 'ready').map((item) => item.source.id)
      if (selected.length > 0) void application.run(selected)
      return
    }
    if (kind === null) {
      const choices = [
        ...new Set(
          list.flatMap((file) => {
            const decision = intake(null, [asToolFile(file)])
            return decision.state === 'choose' ? decision.kinds : []
          })
        )
      ]
      if (choices.length > 0) {
        setJob({ id: nextId(), state: 'choose', files: list, kinds: choices })
        return
      }
    }
    const result = intake(kind, list.map(asToolFile))
    if (result.state === 'stop') {
      setJob({ id: nextId(), state: 'failed', kind, reason: failureKey(result.failure), sources })
      return
    }
    if (result.state === 'choose') {
      setJob({ id: nextId(), state: 'choose', files: list, kinds: result.kinds })
      return
    }
    if (result.state === 'gap') {
      setJob({ id: nextId(), state: 'failed', kind: result.kind, reason: gapKey(result.engine), sources })
      return
    }
    filesRef.current = list
    assemblyOptionsRef.current = { longImage: DEFAULT_LONG_IMAGE, imagePdf: DEFAULT_IMAGE_PDF }
    setAssemblyLong(DEFAULT_LONG_IMAGE)
    setAssemblyPdf(DEFAULT_IMAGE_PDF)
    void run(result.kind, list, nextId(), false)
  }

  const setDocumentOptions = (options: Partial<DocumentToolOptions>, sourceIds?: readonly string[]) => {
    const application = documentApplicationRef.current
    const sourceFiles = documentFilesRef.current
    if (!application || !sourceFiles) return
    const next = { ...documentOptionsRef.current, ...options }
    application.revise(documentToolEngine(application.getSnapshot().kind, sourceFiles.load, next), sourceIds)
    documentOptionsRef.current = next
  }

  const applyGeometry = async (sourceId: string, edit: RasterEdit, ids: readonly string[]) => {
    const application = documentApplicationRef.current
    if (!application || application.getSnapshot().attempt) return
    const settings = validateRasterEdit(edit)
    const previous = documentOptionsRef.current.rasterEditsBySource ?? {}
    const options = { ...previous }
    const changed: string[] = []
    for (const id of ids) {
      const current = previous[id] ?? ORIGINAL_RASTER_EDIT
      const next =
        id === sourceId
          ? settings
          : validateRasterEdit({ ...current, quarterTurns: settings.quarterTurns, straighten: settings.straighten })
      const item = application.getSnapshot().items.find((item) => item.source.id === id)
      if (!item || item.state === 'rejected') throw new Error('Invalid geometry source')
      options[id] = next
      if (!rasterEditsEqual(current, next) || !['review', 'accepted'].includes(item.state)) changed.push(id)
    }
    if (changed.length > 0) {
      setDocumentOptions({ rasterEditsBySource: Object.freeze(options) }, changed)
      await application.run(changed)
    }
    const snapshot = application.getSnapshot()
    for (const item of snapshot.items) {
      if (ids.includes(item.source.id) && item.state === 'review' && item.candidate?.revision === snapshot.revision) {
        application.accept(item.source.id, item.candidate)
      }
    }
  }

  const applyFilter = async (sourceId: string, edit: FilterEdit, pages: readonly number[], ids: readonly string[]) => {
    const application = documentApplicationRef.current
    if (!application || application.getSnapshot().attempt) return
    const settings = validateFilterEdit(edit)
    const previous = documentOptionsRef.current.filterEditsBySource ?? {}
    const options = { ...previous }
    const changed: string[] = []
    for (const id of ids) {
      const item = application.getSnapshot().items.find((item) => item.source.id === id)
      if (!item || item.state === 'rejected') throw new Error('raster-filter-pages')
      const current = previous[id] ?? ORIGINAL_FILTER_DOCUMENT
      const count = (item.candidate ?? item.accepted)?.value.preview?.sourceFiles?.length
      if (id === sourceId && !count) throw new Error('raster-filter-pages')
      if (id === sourceId && (pages.length === 0 || new Set(pages).size !== pages.length))
        throw new Error('raster-filter-pages')
      const next = validateFilterDocument(
        id === sourceId
          ? { ...current, pages: { ...current.pages, ...Object.fromEntries(pages.map((page) => [page, settings])) } }
          : { default: settings, pages: {} },
        count
      )
      options[id] = next
      if (!filterDocumentsEqual(current, next) || !['review', 'accepted'].includes(item.state)) changed.push(id)
    }
    if (changed.length > 0) {
      setDocumentOptions({ filterEditsBySource: Object.freeze(options) }, changed)
      await application.run(changed)
    }
    const snapshot = application.getSnapshot()
    for (const item of snapshot.items) {
      if (ids.includes(item.source.id) && item.state === 'review' && item.candidate?.revision === snapshot.revision)
        application.accept(item.source.id, item.candidate)
    }
  }

  const appendSources = (files: readonly DocumentInputFile[]) => {
    const application = documentApplicationRef.current
    const sourceFiles = documentFilesRef.current
    if (!application || !sourceFiles) return
    try {
      application.append(sourceFiles.add(files))
    } finally {
      sourceFiles.retain(application.getSnapshot().items.map((item) => item.source.id))
    }
    if (isDocumentConversion(application.getSnapshot().kind)) {
      void application.run(
        application
          .getSnapshot()
          .items.filter((item) => item.state === 'ready')
          .map((item) => item.source.id)
      )
    }
  }

  const removeSources = (ids: readonly string[]) => {
    const application = documentApplicationRef.current
    if (!application) return
    application.remove(ids)
    documentFilesRef.current?.retain(application.getSnapshot().items.map((item) => item.source.id))
  }

  const remember = (bytes: Uint8Array, type: string) => {
    const url = URL.createObjectURL(new Blob([bytes.slice()], { type }))
    urlsRef.current.push(url)
    return url
  }

  const run = async (kind: PdfToolKind, list: File[], id: number, confirm: boolean, filter?: FilterName) => {
    abortRef.current?.abort()
    const controller = new AbortController()
    abortRef.current = controller
    const sources = jobSources(list)
    setJob({ id, state: 'running', kind, sources })
    try {
      if (
        (kind === 'merge' || kind === 'image-to-pdf') &&
        list.reduce((sum, file) => sum + file.size, 0) > RASTER_OUTPUT_BYTE_LIMIT
      )
        throw new Error('image-output-limit')
      const files = await readToolBytes(list)
      if (controller.signal.aborted || jobId.current !== id) return
      const result = await runPdfToolJob({
        kind,
        files,
        signal: controller.signal,
        autoCrop: kind === 'scan' && autoCropRef.current,
        confirm,
        ...assemblyOptionsRef.current,
        gatewayBase: osGatewayOrigin(),
        fetchImpl: documentServiceFetch(osGatewayOrigin()),
        onPartial: (text) => {
          if (controller.signal.aborted || jobId.current !== id) return
          setJob({ id, state: 'running', kind, partial: text, sources })
        },
        ...(kind === 'scan' && orgDriveReady() ? { upload: uploadScan } : {}),
        ...(filter ? { filter } : {})
      })
      if (controller.signal.aborted || jobId.current !== id) return
      const view = viewFromResult(id, kind, result, remember)
      setJob(view.state === 'failed' ? { ...view, sources } : view)
    } catch (error) {
      if (controller.signal.aborted || jobId.current !== id) return
      setJob({ id, state: 'failed', kind, reason: pdfToolFailureReason(error), sources })
    } finally {
      if (controller.signal.aborted && jobId.current === id) {
        setJob({ id, state: 'failed', kind, reason: 'workbench.feature.pdf.tools.workspace.state.cancelled', sources })
      }
    }
  }

  const confirmJob = () => {
    const current = job
    if (current?.state !== 'preview') return
    setJob(viewFromResult(current.id, current.kind, current.confirmation, remember))
  }

  const chooseFilter = (filter: FilterName) => {
    setFilterPick(filter)
    const current = job
    if (current?.state !== 'preview' || current.kind !== 'filter') return
    void run(current.kind, filesRef.current, current.id, false, filter)
  }

  const cancel = () => {
    abortRef.current?.abort()
    setJob((current) => (current?.state === 'running' ? { ...current, cancelling: true } : current))
  }

  const reviseAssembly = (
    options: { longImage?: LongImageOptions; imagePdf?: ImagePdfOptions },
    move?: { index: number; delta: number }
  ) => {
    if (
      !job ||
      job.state === 'choose' ||
      job.state === 'running' ||
      (job.kind !== 'merge' && job.kind !== 'image-to-pdf')
    )
      return
    if (move) {
      const files = [...filesRef.current]
      const target = move.index + move.delta
      if (move.index < 0 || target < 0 || move.index >= files.length || target >= files.length) return
      ;[files[move.index], files[target]] = [files[target], files[move.index]]
      filesRef.current = files
    }
    abortRef.current?.abort()
    assemblyOptionsRef.current = { ...assemblyOptionsRef.current, ...options }
    if (options.longImage) setAssemblyLong(options.longImage)
    if (options.imagePdf) setAssemblyPdf(options.imagePdf)
    for (const url of urlsRef.current) URL.revokeObjectURL(url)
    urlsRef.current = []
    const id = ++jobId.current
    setJob({ id, state: 'configured', kind: job.kind, sources: jobSources(filesRef.current) })
  }

  const takeRef = useRef(take)
  takeRef.current = take
  const seeded = !returned && Boolean(initialFiles && initialFiles.length > 0 && isPdfToolKind(stagedKind.current))
  const stage = documentApplication ? 'result' : stageOf(job, seeded)
  const goCatalog = () => {
    abortRef.current?.abort()
    documentApplicationRef.current?.dispose()
    documentApplicationRef.current = null
    documentFilesRef.current = null
    setDocumentApplication(null)
    setReturned(true)
    setJob(null)
  }

  useEffect(() => {
    const onPaste = (event: ClipboardEvent) => {
      if (documentApplicationRef.current) return
      const target = event.target
      if (target instanceof Element && target.closest('input, textarea, [contenteditable="true"]')) return
      const files = [...(event.clipboardData?.files ?? [])]
      if (files.length === 0) return
      event.preventDefault()
      takeRef.current(null, files)
    }
    window.addEventListener('paste', onPaste)
    return () => window.removeEventListener('paste', onPaste)
  }, [])

  return (
    <section
      aria-label={title}
      data-ui="workbench.applet.window"
      data-applet="pdf"
      data-presentation="window"
      className={
        onDismiss
          ? 'fixed inset-0 z-[240] flex flex-col overflow-hidden bg-white text-[#171717]'
          : 'fixed inset-0 flex flex-col overflow-hidden bg-white text-[#171717]'
      }>
      <AppletWindowBar
        title={title}
        icon={<PdfMark />}
        canBack={stage !== 'catalog'}
        onBack={goCatalog}
        onHome={() => {
          goCatalog()
          reload()
        }}
        onRefresh={reload}
        onClose={onDismiss}
      />
      <AppletPermissionGate appId="pdf" epoch={epoch} name={title} onDeny={onDismiss}>
        <div
          key={epoch}
          data-ui="workbench.pdf-tools"
          data-stage={stage}
          className={`relative min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-contain ${stage === 'catalog' ? 'bg-[#f4f5f7]' : 'bg-white'}`}>
          {seeded && isPdfToolKind(stagedKind.current) ? (
            <StartWithFiles
              kind={stagedKind.current}
              files={initialFiles ?? []}
              onStart={(kind, list) => takeRef.current(kind, list)}
            />
          ) : null}
          <input
            ref={bindInput}
            type="file"
            multiple
            className="sr-only"
            data-ui="workbench.pdf-tools.input"
            onChange={(event) => {
              const list = [...(event.target.files ?? [])]
              event.target.value = ''
              take(pendingKind.current, list)
            }}
          />
          {stage === 'catalog' ? (
            <div className="mx-auto w-full max-w-[1080px] min-w-0 px-4 py-5 sm:px-6">
              <div className="grid min-w-0 items-stretch gap-3 rounded-[16px] border border-[#ececec] bg-white p-4 shadow-[0_4px_16px_rgba(0,0,0,0.04)] min-[640px]:grid-cols-2 min-[1000px]:grid-cols-[minmax(0,1.05fr)_minmax(0,1fr)_minmax(0,220px)] sm:p-5">
                <div className="flex min-w-0 flex-col justify-center">
                  <h2 className="text-[24px] leading-8 font-bold text-[#171717]">{title}</h2>
                  <p className="mt-2 text-[13px] leading-5 text-[#8a8a8a]">
                    {t('workbench.feature.pdf.tools.subtitle')}
                    <span className="mt-0.5 block">{t('workbench.feature.pdf.tools.subtitle_batch')}</span>
                  </p>
                  <button
                    type="button"
                    onClick={() => pick(null)}
                    className={`mt-5 inline-flex h-9 w-fit items-center gap-1.5 rounded-[8px] bg-[#171717] px-3.5 text-[13px] text-white hover:bg-[#2a2a2a] disabled:opacity-40 ${FOCUS}`}>
                    <Plus size={16} strokeWidth={1.7} aria-hidden />
                    {t('workbench.feature.pdf.tools.upload')}
                  </button>
                </div>

                <div
                  data-ui="workbench.pdf-tools.drop"
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={(event) => {
                    event.preventDefault()
                    take(null, [...event.dataTransfer.files])
                  }}
                  className="flex min-h-[120px] min-w-0 flex-col items-center justify-center gap-2.5 rounded-[12px] border border-dashed border-[#e6e6e6] bg-[#f5f5f5] px-4 py-4 text-center min-[720px]:min-h-[148px]">
                  <button
                    type="button"
                    onClick={() => pick(null)}
                    className={`flex flex-col items-center gap-2.5 ${FOCUS}`}>
                    <span className="flex items-end">
                      {DROP_TYPES.map((type, index) => (
                        <span
                          key={type}
                          className={index === 0 ? 'relative' : 'relative -ml-2'}
                          style={{ zIndex: index }}>
                          <DocumentFileIcon type={type} className="h-8 w-7" />
                        </span>
                      ))}
                    </span>
                    <span className="text-[13px] text-[#3f3f3f]">{t('workbench.feature.pdf.tools.drop')}</span>
                  </button>
                  <button
                    type="button"
                    aria-pressed={autoCrop}
                    data-ui="workbench.pdf-tools.crop"
                    onClick={() => setAutoCrop((value) => !value)}
                    className={`inline-flex items-center gap-1 text-[12px] text-[#8a8a8a] ${FOCUS}`}>
                    <Check size={14} strokeWidth={1.7} aria-hidden />
                    {t('workbench.feature.pdf.tools.crop')}
                  </button>
                </div>

                <div className="flex min-w-0 flex-col gap-3 min-[640px]:col-span-2 min-[1000px]:col-span-1">
                  <SideLink
                    icon={Mic}
                    title={t('workbench.feature.pdf.tools.audio.title')}
                    body={t('workbench.feature.pdf.tools.audio.body')}
                    onClick={() => pick('audio')}
                  />
                  <SideLink
                    icon={FilePen}
                    title={t('workbench.feature.pdf.tools.edit.title')}
                    body={t('workbench.feature.pdf.tools.edit.body')}
                    onClick={() => pick('pdf-edit')}
                  />
                </div>
              </div>

              <section className="mt-6">
                <h3 className="text-[12px] leading-4 text-[#8a8a8a]">
                  {t('workbench.feature.pdf.tools.edit_section')}
                </h3>
                <div className="mt-2.5 flex flex-wrap gap-2">
                  {EDIT_CHIPS.map(({ key, kind, icon: Icon }) => (
                    <button
                      key={key}
                      type="button"
                      data-kind={kind}
                      onClick={() => pick(kind)}
                      className={`inline-flex h-8 items-center gap-1.5 rounded-[8px] border border-[#e6e6e6] bg-white px-2.5 text-[13px] text-[#171717] shadow-[0_1px_2px_rgba(20,40,80,0.04)] transition-colors hover:bg-[#fafafa] ${FOCUS}`}>
                      <Icon size={16} strokeWidth={1.7} aria-hidden />
                      {t(key)}
                    </button>
                  ))}
                </div>
              </section>

              <section className="mt-6">
                <h3 className="text-[12px] leading-4 text-[#8a8a8a]">{t('workbench.feature.pdf.tools.all')}</h3>
                <div className="mt-2.5 grid min-w-0 grid-cols-1 gap-3 min-[640px]:grid-cols-2 min-[1000px]:grid-cols-3">
                  {TOOL_GROUPS.map((group) => (
                    <div
                      key={group.title}
                      className="rounded-[14px] border border-[#ececec] bg-white p-2 shadow-[0_4px_16px_rgba(0,0,0,0.04)]">
                      <h4 className="px-2.5 pt-1.5 pb-1 text-[14px] font-medium text-[#1a1a1a]">{t(group.title)}</h4>
                      <div className="flex flex-col gap-0.5">
                        {group.items.map((item) => (
                          <button
                            key={item.key}
                            type="button"
                            aria-label={t(item.key)}
                            data-kind={item.kind}
                            onClick={() => pick(item.kind)}
                            className={`flex h-9 items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] text-[#3f3f3f] transition-colors hover:bg-[#f7f7f7] hover:text-[#171717] ${FOCUS}`}>
                            <DocumentFileIcon type={item.type} className="h-[22px] w-[18px]" />
                            <span className="min-w-0 flex-1 truncate">{t(item.key)}</span>
                            <ChevronRight size={16} strokeWidth={1.7} className="text-[#8a8a8a]" aria-hidden />
                          </button>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </section>

              <button
                type="button"
                aria-label={t('workbench.feature.pdf.tools.scan.title')}
                onClick={() => pick('scan')}
                className={`mt-3 flex w-full min-w-0 items-center gap-3 rounded-[14px] border border-[#ececec] bg-white px-3.5 py-3 text-left shadow-[0_4px_16px_rgba(0,0,0,0.04)] transition-colors hover:bg-[#fafafa] ${FOCUS}`}>
                <span className="grid size-9 shrink-0 place-items-center rounded-[8px] bg-[#f4f5f7] text-[#171717]">
                  <ScanLine size={16} strokeWidth={1.7} aria-hidden />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-[14px] font-medium text-[#1a1a1a]">
                    {t('workbench.feature.pdf.tools.scan.title')}
                  </span>
                  <span className="mt-0.5 block text-[12px] leading-5 text-[#8b8b8b]">
                    {t('workbench.feature.pdf.tools.scan.body')}
                  </span>
                </span>
                <ChevronRight size={16} strokeWidth={1.7} className="shrink-0 text-[#8a8a8a]" aria-hidden />
              </button>
            </div>
          ) : documentApplication ? (
            <DocumentWorkspace
              key={documentApplication.getSnapshot().id}
              onPdfExport={(pdfExport) => setDocumentOptions({ pdfExport })}
              onOfficeExport={(officeExport) => setDocumentOptions({ officeExport })}
              onLongImage={(longImage) => setDocumentOptions({ longImage })}
              onGeometry={applyGeometry}
              onApplyFilter={applyFilter}
              getFilterDocument={(id) =>
                documentOptionsRef.current.filterEditsBySource?.[id] ?? ORIGINAL_FILTER_DOCUMENT
              }
              getRasterEdit={(id) => documentOptionsRef.current.rasterEditsBySource?.[id] ?? ORIGINAL_RASTER_EDIT}
              application={documentApplication}
              onBack={goCatalog}
              onAddSources={appendSources}
              onRemoveSources={removeSources}
              writer={documentArtifactWriter(async (name, bytes) => window.api.file.save(name, bytes))}
              onFilter={(filter) => {
                const application = documentApplicationRef.current
                if (!application) return
                setDocumentOptions({ filter })
                void application.run(
                  application
                    .getSnapshot()
                    .items.filter((item) => item.state === 'ready')
                    .map((item) => item.source.id)
                )
              }}
            />
          ) : (
            <JobPanel
              job={job ?? runningPlaceholder(isPdfToolKind(stagedKind.current) ? stagedKind.current : 'pdf-to-word')}
              filterPick={filterPick}
              onFilter={chooseFilter}
              onConfirm={confirmJob}
              onChoose={(kind) => take(kind, job?.state === 'choose' ? job.files : [])}
              onCancel={cancel}
              assembly={
                job && job.state !== 'choose' && (job.kind === 'merge' || job.kind === 'image-to-pdf')
                  ? {
                      sources: jobSources(filesRef.current),
                      longImage: assemblyLong,
                      imagePdf: assemblyPdf,
                      onLongImage: (longImage) => reviseAssembly({ longImage }),
                      onImagePdf: (imagePdf) => reviseAssembly({ imagePdf }),
                      onMove: (index, delta) => reviseAssembly({}, { index, delta }),
                      onRun: () => {
                        if (job.state !== 'running' && (job.kind === 'merge' || job.kind === 'image-to-pdf'))
                          void run(job.kind, filesRef.current, ++jobId.current, false)
                      }
                    }
                  : undefined
              }
            />
          )}
        </div>
      </AppletPermissionGate>
    </section>
  )
}

function StartWithFiles({
  kind,
  files,
  onStart
}: {
  kind: PdfToolKind
  files: readonly File[]
  onStart: (kind: PdfToolKind, files: File[]) => void
}) {
  const started = useRef(false)
  const onStartRef = useRef(onStart)
  onStartRef.current = onStart
  useEffect(() => {
    if (started.current) return
    started.current = true
    onStartRef.current(kind, [...files])
  }, [files, kind])
  return null
}

function setMainTrafficLights(hidden: boolean) {
  const invoke = (
    globalThis as typeof globalThis & {
      __TAURI_INTERNALS__?: { invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown> }
    }
  ).__TAURI_INTERNALS__?.invoke
  if (!invoke) return
  void invoke('set_main_traffic_lights_hidden', { hidden }).catch(() => undefined)
}

function stageOf(job: JobView | null, seeded: boolean): 'catalog' | 'work' | 'result' {
  if (!job && !seeded) return 'catalog'
  if (job && job.state !== 'running') return 'result'
  return 'work'
}

function runningPlaceholder(kind: PdfToolKind): JobView {
  return { id: 0, state: 'running', kind, sources: [] }
}

function jobSources(list: readonly { name: string; size: number }[]): JobSource[] {
  return list.map((file) => ({ name: file.name, size: file.size }))
}

const PRIMARY =
  'inline-flex h-9 items-center rounded-[8px] bg-[#171717] px-3.5 text-[13px] text-white hover:bg-[#2a2a2a]'

const QUIET =
  'inline-flex h-[30px] shrink-0 items-center rounded-[8px] border border-[#e5e5e5] bg-white px-2.5 text-[13px] leading-none text-[#171717] hover:bg-[#fafafa]'

type JobFileRow = { key: string; name: string; size: number; url?: string }

function JobPanel({
  job,
  filterPick,
  onFilter,
  onConfirm,
  onChoose,
  onCancel,
  assembly
}: {
  job: JobView
  filterPick: FilterName
  onFilter: (filter: FilterName) => void
  onConfirm: () => void
  onChoose: (kind: PdfToolKind) => void
  onCancel: () => void
  assembly?: {
    sources: JobSource[]
    longImage: LongImageOptions
    imagePdf: ImagePdfOptions
    onLongImage: (value: LongImageOptions) => void
    onImagePdf: (value: ImagePdfOptions) => void
    onMove: (index: number, delta: number) => void
    onRun: () => void
  }
}) {
  const { t } = useTranslation()
  const reason =
    job.state === 'failed'
      ? t(job.reason)
      : job.state === 'running'
        ? t(
            job.cancelling
              ? 'workbench.feature.pdf.tools.workspace.cancelling'
              : 'workbench.feature.pdf.tools.job.running'
          )
        : job.state === 'done'
          ? t('workbench.feature.pdf.tools.job.done')
          : job.state === 'preview'
            ? t('workbench.feature.pdf.tools.job.preview')
            : null
  const note = job.state === 'done' || job.state === 'preview' ? job.note : undefined
  const openLabel = t(
    assembly ? 'workbench.feature.pdf.tools.workspace.assembly.download' : 'workbench.feature.pdf.tools.job.open'
  )
  const sources = job.state === 'done' || (assembly && assembly.sources.length > 1) ? [] : sourceRows(job)
  return (
    <section
      className="min-h-full bg-white"
      data-state={job.state}
      data-ui="workbench.pdf-tools.job"
      aria-busy={job.state === 'running' ? true : undefined}
      data-dpi={job.state === 'done' && job.dpi ? String(job.dpi) : undefined}>
      <div className={`mx-auto flex w-full max-w-[920px] min-w-0 flex-col px-6 py-8`}>
        {job.state === 'failed' ? (
          <p className="text-[15px] leading-6 font-medium text-[#171717]" data-ui="workbench.pdf-tools.job.reason">
            {reason}
          </p>
        ) : null}
        {job.state === 'failed' && job.kind ? (
          <p className="mt-1 text-[13px] leading-5 text-[#8a8a8a]">{t(toolLabelKey(job.kind))}</p>
        ) : null}
        {job.state !== 'failed' && job.state !== 'choose' ? (
          <h2 className="text-[24px] leading-8 font-bold text-[#171717]">{t(toolLabelKey(job.kind))}</h2>
        ) : null}
        {job.state !== 'failed' && reason ? (
          <p className="mt-1 text-[13px] leading-5 text-[#8a8a8a]" data-ui="workbench.pdf-tools.job.reason">
            {reason}
          </p>
        ) : null}
        {job.state === 'done' ? (
          <div className="mt-4 flex flex-col gap-3">
            {job.files.map((file) => (
              <OutputActions
                key={file.url}
                file={{ name: file.filename, mediaType: file.mediaType, bytes: file.bytes }}
                url={file.url}
              />
            ))}
          </div>
        ) : null}
        {assembly ? (
          <>
            {assembly.sources.length > 1 ? (
              <details open className="mt-4 rounded-[8px] border border-[#ececec] p-3">
                <summary className="cursor-pointer text-[13px]">
                  {t('workbench.feature.pdf.tools.workspace.assembly.order')}
                </summary>
                <ol className="mt-2 flex flex-col gap-2" data-ui="document-tools.assembly-sources">
                  {assembly.sources.map((source, index) => (
                    <li key={`${index}:${source.name}`} className="flex min-w-0 items-center gap-2 text-[13px]">
                      <span className="min-w-0 flex-1 break-words">{source.name}</span>
                      <Button
                        type="button"
                        className={`${QUIET} ${FOCUS}`}
                        disabled={job.state === 'running' || index === 0}
                        aria-label={t('workbench.feature.pdf.tools.workspace.move_up', { name: source.name })}
                        onClick={() => assembly.onMove(index, -1)}>
                        <ArrowUp size={14} aria-hidden />
                      </Button>
                      <Button
                        type="button"
                        className={`${QUIET} ${FOCUS}`}
                        disabled={job.state === 'running' || index === assembly.sources.length - 1}
                        aria-label={t('workbench.feature.pdf.tools.workspace.move_down', { name: source.name })}
                        onClick={() => assembly.onMove(index, 1)}>
                        <ArrowDown size={14} aria-hidden />
                      </Button>
                    </li>
                  ))}
                </ol>
              </details>
            ) : null}
            <details
              className="mt-4 rounded-[8px] border border-[#ececec] p-3"
              data-ui="document-tools.assembly-options">
              <summary className="cursor-pointer text-[13px]">
                {t('workbench.feature.pdf.tools.workspace.options')}
              </summary>
              {job.state !== 'choose' && job.kind === 'merge' ? (
                <LongImageOptionsPanel
                  value={assembly.longImage}
                  busy={job.state === 'running'}
                  onChange={assembly.onLongImage}
                />
              ) : (
                <ImagePdfOptionsPanel
                  value={assembly.imagePdf}
                  busy={job.state === 'running'}
                  onChange={assembly.onImagePdf}
                />
              )}
              <p className="mt-3 text-[12px]">{t('workbench.feature.pdf.tools.workspace.pdf.apply_hint')}</p>
            </details>
            {job.state === 'configured' || job.state === 'failed' ? (
              <Button type="button" className={`mt-4 w-fit ${PRIMARY} ${FOCUS}`} onClick={assembly.onRun}>
                {t('workbench.feature.pdf.tools.workspace.process')}
              </Button>
            ) : null}
          </>
        ) : null}
        {note && note !== 'workbench.feature.pdf.tools.job.preview' ? (
          <p className="mt-2 text-[12px] leading-5 text-[#8a8a8a]">{t(note)}</p>
        ) : null}
        {job.state === 'done' && job.label ? (
          <p className="mt-2 text-[12px] leading-5 text-[#3f3f3f]" data-ui="workbench.pdf-tools.job.label">
            {job.label}
          </p>
        ) : null}
        {job.state === 'done' && job.copy ? (
          <p className="mt-2 text-[12px] leading-5 text-[#8a8a8a]" data-ui="workbench.pdf-tools.job.copy">
            {job.copy}
          </p>
        ) : null}
        {job.state === 'preview' && job.previews ? (
          <div className="mt-4 flex flex-wrap gap-2">
            {job.previews.map((item) => (
              <button
                key={item.filter}
                type="button"
                aria-pressed={filterPick === item.filter}
                data-ui={`workbench.pdf-tools.filter.${item.filter}`}
                onClick={() => onFilter(item.filter)}
                className={`overflow-hidden rounded-[8px] border ${filterPick === item.filter ? 'border-[#171717]' : 'border-[#e6e6e6]'} ${FOCUS}`}>
                <img alt={item.filter} src={item.url} className="h-16 w-16 object-contain" />
              </button>
            ))}
          </div>
        ) : null}
        {job.state === 'done' ? (
          <div className="mt-4 flex flex-col gap-3" data-ui="document-tools.assembly-preview">
            {job.files.map((file) => (
              <figure key={file.url}>
                <figcaption className="mb-2 text-[12px] text-[#8a8a8a]">
                  {file.filename}
                  {file.width && file.height ? ` · ${file.width} × ${file.height} px` : ''}
                </figcaption>
                <OutputPreview
                  file={{ name: file.filename, mediaType: file.mediaType, bytes: file.bytes }}
                  url={file.url}
                />
              </figure>
            ))}
          </div>
        ) : (
          <JobFiles rows={sources} openLabel={openLabel} perRow={false} />
        )}
        {job.state === 'running' ? <JobProgress label={reason} /> : null}
        {job.state === 'running' && job.partial ? (
          <p
            className="mt-4 max-h-40 overflow-y-auto rounded-[8px] bg-[#f5f5f5] px-3 py-2 text-[13px] leading-5 text-[#3f3f3f]"
            data-ui="workbench.pdf-tools.job.partial">
            {job.partial}
          </p>
        ) : null}
        {job.state === 'choose' ? (
          <div className="mt-4 flex flex-wrap gap-2">
            {job.kinds.map((kind) => (
              <button
                key={kind}
                type="button"
                onClick={() => onChoose(kind)}
                className={`inline-flex h-9 items-center rounded-[8px] border border-[#e6e6e6] bg-white px-3.5 text-[13px] text-[#171717] hover:bg-[#f7f7f7] ${FOCUS}`}>
                {t(toolLabelKey(kind))}
              </button>
            ))}
          </div>
        ) : null}
        {job.state === 'preview' ? (
          <button
            type="button"
            data-ui="workbench.pdf-tools.confirm"
            onClick={onConfirm}
            className={`mt-6 w-fit ${PRIMARY} ${FOCUS}`}>
            {t('workbench.feature.pdf.tools.job.confirm')}
          </button>
        ) : null}
        {job.state === 'running' ? (
          <button
            type="button"
            onClick={onCancel}
            disabled={job.cancelling}
            className={`mt-5 h-9 w-fit rounded-[8px] px-3.5 text-[13px] text-[#171717] hover:bg-[#f5f5f5] ${FOCUS}`}>
            {t('workbench.feature.pdf.tools.job.cancel')}
          </button>
        ) : null}
      </div>
    </section>
  )
}

function sourceRows(job: JobView): JobFileRow[] {
  if (job.state === 'running' || job.state === 'failed' || job.state === 'configured') {
    return job.sources.map((file, index) => ({ key: `${index}:${file.name}`, name: file.name, size: file.size }))
  }
  if (job.state === 'choose') {
    return job.files.map((file, index) => ({ key: `${index}:${file.name}`, name: file.name, size: file.size }))
  }
  if (job.state === 'preview') {
    return job.files.map((file) => ({ key: file.url, name: file.filename, size: file.bytes.byteLength }))
  }
  return []
}

function JobFiles({ rows, openLabel, perRow }: { rows: readonly JobFileRow[]; openLabel: string; perRow: boolean }) {
  if (rows.length === 0) return null
  return (
    <ul className="mt-5 overflow-hidden rounded-[8px] border border-[#efefef]" data-ui="workbench.pdf-tools.job.files">
      {rows.map((row) => (
        <li
          key={row.key}
          className="flex h-9 items-center gap-2.5 border-b border-[#efefef] px-4 last:border-b-0 hover:bg-[#f7f7f7]">
          <DocumentFileIcon type={documentFileIconType(row.name)} className="h-4 w-[13px]" />
          <span className="min-w-0 flex-1 truncate text-[13px] text-[#3f3f3f]">{row.name}</span>
          <span className="shrink-0 text-[12px] text-[#8a8a8a] tabular-nums">{formatJobBytes(row.size)}</span>
          {perRow && row.url ? (
            <a
              className={`${QUIET} ${FOCUS}`}
              download={row.name}
              href={row.url}
              aria-label={`${openLabel} ${row.name}`}>
              {openLabel}
            </a>
          ) : null}
        </li>
      ))}
    </ul>
  )
}

function JobProgress({ label }: { label: string | null }) {
  return (
    <div
      className="relative mt-5 h-1 overflow-hidden rounded-full bg-[#f5f5f5]"
      role="progressbar"
      aria-valuetext={label ?? undefined}>
      <style>
        {
          '@keyframes a3s-pdf-job{from{transform:translateX(-130%)}to{transform:translateX(280%)}}@media (prefers-reduced-motion:reduce){.a3s-pdf-job-bar{animation:none!important;width:100%;opacity:.35}}'
        }
      </style>
      <span
        className="a3s-pdf-job-bar block h-full w-1/3 rounded-full bg-[#171717]"
        style={{ animation: 'a3s-pdf-job 1.15s ease-in-out infinite' }}
      />
    </div>
  )
}

function formatJobBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B'
  if (bytes < 1024) return `${Math.round(bytes)} B`
  const kilobytes = bytes / 1024
  if (kilobytes < 1024) return `${formatJobUnit(kilobytes)} KB`
  const megabytes = kilobytes / 1024
  if (megabytes < 1024) return `${formatJobUnit(megabytes)} MB`
  return `${formatJobUnit(megabytes / 1024)} GB`
}

function formatJobUnit(value: number): string {
  const rounded = Math.round(value * 10) / 10
  return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1)
}

function viewFromResult(
  id: number,
  kind: PdfToolKind,
  result: PdfToolRunResult,
  remember: (bytes: Uint8Array, type: string) => string
): JobView {
  if (result.state === 'choose') return { id, state: 'choose', files: [], kinds: result.kinds }
  if (result.state === 'failed') return { id, state: 'failed', kind: result.kind, reason: result.reason, sources: [] }
  if (result.state === 'preview') {
    return {
      id,
      state: 'preview',
      kind,
      note: result.note,
      confirmation: result.confirmation,
      files: result.files.map((file) => savedFile(file, remember)),
      previews: result.previews?.map((item) => ({ filter: item.filter, url: remember(item.bytes, 'image/png') }))
    }
  }
  return {
    id,
    state: 'done',
    kind,
    note: result.note,
    label: result.label,
    copy: result.copy,
    editor: result.editor,
    dpi: result.dpi,
    files: result.files.map((file) => savedFile(file, remember))
  }
}

function savedFile(file: ToolOutput, remember: (bytes: Uint8Array, type: string) => string): SavedFile {
  return {
    url: remember(file.bytes, file.mediaType),
    filename: file.name,
    bytes: file.bytes,
    mediaType: file.mediaType,
    width: file.width,
    height: file.height
  }
}

async function readToolBytes(list: readonly File[]): Promise<ToolBytes[]> {
  return Promise.all(
    list.map(async (file) => ({
      name: file.name,
      type: file.type,
      bytes: new Uint8Array(await blobBytes(file))
    }))
  )
}

function blobBytes(file: Blob): Promise<ArrayBuffer> {
  if (typeof file.arrayBuffer === 'function') return file.arrayBuffer()
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as ArrayBuffer)
    reader.onerror = () => reject(reader.error ?? new Error('unreadable'))
    reader.readAsArrayBuffer(file)
  })
}

function asToolFile(file: File): ToolFile {
  return { name: file.name, type: file.type, size: file.size }
}

function orgDriveReady(): boolean {
  try {
    const token = localStorage.getItem('a3s_org_member_token')?.trim() ?? ''
    const org = readCenterId()
    return token.length > 0 && org.length > 0 && org !== PERSONAL_CENTER
  } catch {
    return false
  }
}

async function uploadScan(file: ToolBytes): Promise<void> {
  await driveApi.upload(new File([file.bytes.slice()], file.name, { type: file.type }))
}

function PdfMark() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" className="size-4 shrink-0" aria-hidden>
      <path
        d="M4 1.8h5.2L12.2 5v8.2a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2.8a1 1 0 0 1 1-1Z"
        fill="none"
        stroke="#5b35d4"
        strokeWidth="1.4"
      />
      <path d="M9.1 1.9v3.2h3.1" fill="none" stroke="#5b35d4" strokeWidth="1.4" />
    </svg>
  )
}

function SideLink({
  icon: Icon,
  title,
  body,
  onClick
}: {
  icon: LucideIcon
  title: string
  body: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={title}
      className={`flex flex-1 items-center gap-2 rounded-[14px] border border-[#ececec] bg-white px-3 py-3 text-left transition-colors hover:bg-[#fafafa] ${FOCUS}`}>
      <span className="min-w-0 flex-1">
        <span className="flex min-w-0 items-center gap-1.5 text-[14px] font-medium break-words text-[#1a1a1a]">
          <Icon size={16} strokeWidth={1.7} aria-hidden />
          {title}
        </span>
        <span className="mt-1 block text-[12px] leading-5 text-[#8b8b8b]">{body}</span>
      </span>
      <ChevronRight size={16} strokeWidth={1.7} className="shrink-0 text-[#8a8a8a]" aria-hidden />
    </button>
  )
}
