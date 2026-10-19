import { documentServiceFailureKey } from '@renderer/data/documentServiceTransport'

import { DEFAULT_IMAGE_PDF, validateImagePdf, type ImagePdfOptions } from './documentTools/imagePdf'
import {
  DEFAULT_LONG_IMAGE,
  composeLongImage,
  validateLongImage,
  type LongImageOptions
} from './documentTools/longImage'
import { wordLayoutPdf } from './documentTools/officeLayout'
import { DEFAULT_OFFICE_EXPORT, validateOfficeExport, type OfficeExportOptions } from './documentTools/officeSvg'
import {
  DEFAULT_PDF_EXPORT,
  parsePdfExportDpi,
  parsePdfImageFormat,
  type PdfExportOptions
} from './documentTools/pdfExport'
import { encodePdfRaster } from './documentTools/pdfImage'
import {
  applyFilterEdit,
  filterPageEdit,
  validateFilterDocument,
  type FilterDocumentEdit
} from './documentTools/rasterFilter'
import { applyRasterEdit, validateRasterEdit, type RasterEdit } from './documentTools/rasterGeometry'
import { RASTER_OUTPUT_BYTE_LIMIT, RASTER_PIXEL_LIMIT, validateRasterSize } from './documentTools/rasterLimits'
import { officialWritingDraft } from './officialWriting'
import { DOCX_MEDIA_TYPE, XLSX_MEDIA_TYPE, docxFromPlainText, xlsxFromRows } from './pdfToolsDocx'
import {
  engineForKind,
  failureKey,
  fileKindOf,
  gapKey,
  intake,
  type PdfToolKind,
  type ToolEngine,
  type ToolFileKind
} from './pdfToolsJob'
import { captureOfficePages } from './pdfToolsOffice'
import { imagesToPdf, pngsToPdf, rotatePdf, type PdfTextBox } from './pdfToolsPdf'
import { pdfBytesLookEncrypted, pdfiumPages, pngOf } from './pdfToolsPdfium'
import { decodePng, encodePng, type Raster } from './pdfToolsPng'
import {
  IMAGE_PDF_DPI,
  OFFICE_CAPTURE_DPI,
  PDF_RASTER_DPI,
  applyFilter,
  autoCrop as cropMargins,
  crop,
  rotate90,
  type FilterName
} from './pdfToolsRaster'

export const LAYOUT_LABEL = '按识别结果排版'
export const LOCAL_ONLY_COPY = '结果只在这台电脑上。'

const PNG = 'image/png'
const PDF = 'application/pdf'
const TEXT = 'text/plain;charset=utf-8'

const KEY = {
  cancel: 'workbench.feature.pdf.tools.job.cancel',
  confirm: 'workbench.feature.pdf.tools.job.preview',
  cropKept: 'workbench.feature.pdf.tools.job.crop_kept',
  encrypted: 'workbench.feature.pdf.tools.job.encrypted',
  failed: 'workbench.feature.pdf.tools.job.failed',
  layout: 'workbench.feature.pdf.tools.job.layout',
  localOnly: 'workbench.feature.pdf.tools.job.local_only',
  noTable: 'workbench.feature.pdf.tools.job.no_table',
  pdfEdit: 'workbench.feature.pdf.tools.job.pdf_edit',
  unreadable: 'workbench.feature.pdf.tools.job.unreadable'
}

export type ToolBytes = { name: string; type: string; bytes: Uint8Array }

export type ToolOutput = {
  name: string
  mediaType: string
  bytes: Uint8Array
  width?: number
  height?: number
  sourcePage?: number
  previewPage?: number
  sourceSheet?: number
}

export type FilterPreview = { filter: FilterName; bytes: Uint8Array }

export type PdfToolRunInput = {
  kind: PdfToolKind | null
  files: readonly ToolBytes[]
  signal?: AbortSignal
  autoCrop?: boolean
  confirm?: boolean
  filter?: FilterName
  filterDocument?: FilterDocumentEdit
  pdfExport?: PdfExportOptions
  officeExport?: OfficeExportOptions
  longImage?: LongImageOptions
  imagePdf?: ImagePdfOptions
  rasterEdit?: RasterEdit
  /** Pixel crop applied after a 90-degree rotate. Omit it to rotate only. */
  crop?: { x: number; y: number; width: number; height: number }
  /** When false, a rotate job crops without turning the page. */
  rotate?: boolean
  gatewayBase?: string
  maskedEditUrl?: string
  fetchImpl?: typeof fetch
  upload?: (file: ToolBytes) => Promise<void>
  onPartial?: (text: string) => void
}

export type PdfToolRunResult =
  | { state: 'choose'; kinds: PdfToolKind[] }
  | {
      state: 'preview'
      kind: PdfToolKind
      files: ToolOutput[]
      previews?: FilterPreview[]
      sourceFiles?: ToolOutput[]
      note: string
      confirmation: Extract<PdfToolRunResult, { state: 'done' }>
    }
  | {
      state: 'done'
      kind: PdfToolKind
      files: ToolOutput[]
      note?: string
      label?: string
      copy?: string
      editor?: 'pdf'
      dpi?: number
      localOnly?: boolean
      inImageText?: boolean
      streamed?: boolean
      partials?: string[]
      transcript?: string
      minutes?: string
    }
  | { state: 'failed'; kind: PdfToolKind | null; reason: string }

type OcrBox = { x: number; y: number; width: number; height: number }
type OcrBlock = { text?: string; boundingBox?: OcrBox }
type OcrPayload = { text?: string; blocks?: OcrBlock[] }

const FILTERS: readonly FilterName[] = ['document', 'grayscale', 'contrast']

/** Run one conversion. Pixel edits, filters, rotate, and crop stay a preview until confirm. */
export async function runPdfToolJob(input: PdfToolRunInput): Promise<PdfToolRunResult> {
  await Promise.resolve()
  try {
    if (input.signal?.aborted) return fail(input.kind, KEY.cancel)
    const gate = intake(
      input.kind,
      input.files.map((file) => ({ name: file.name, type: file.type, size: file.bytes.byteLength }))
    )
    if (gate.state === 'stop') return fail(input.kind, failureKey(gate.failure))
    if (gate.state === 'choose') return { state: 'choose', kinds: gate.kinds }
    if (gate.state === 'gap') return fail(gate.kind, gapKey(gate.engine))
    const kind = gate.kind
    if (input.files.some((file) => fileKindOf(file) === 'pdf' && pdfBytesLookEncrypted(file.bytes))) {
      return fail(kind, KEY.encrypted)
    }
    throwIfAborted(input.signal)
    return await dispatch(kind, input)
  } catch (error) {
    return fail(input.kind, pdfToolFailureReason(error))
  }
}

async function dispatch(kind: PdfToolKind, input: PdfToolRunInput): Promise<PdfToolRunResult> {
  const fileKind = fileKindOf(input.files[0] ?? { name: '', type: '' })
  if (kind === 'merge') return merge(input)
  if (kind === 'image-to-pdf') return imagePdf(input)
  if (kind === 'rotate') return rotateJob(input, fileKind)
  if (kind === 'filter') return filterJob(input)
  if (kind === 'handwriting' || kind === 'watermark' || kind === 'erase') return pixelJob(kind, input)
  if (kind === 'pdf-to-image' || kind === 'pdf-to-long') return pdfRaster(kind, input)
  if (kind === 'word-to-pdf' || kind === 'word-to-image' || kind === 'excel-to-pdf' || kind === 'excel-to-image') {
    return officeJob(kind, input)
  }
  if (kind === 'extract') return extractJob(input, fileKind)
  if (kind === 'pdf-to-word' || kind === 'image-to-word') return wordJob(kind, input, fileKind)
  if (kind === 'pdf-to-excel' || kind === 'image-to-excel') return excelJob(kind, input, fileKind)
  if (kind === 'audio') return audioJob(input)
  if (kind === 'scan') return scanJob(input, fileKind)
  if (kind === 'pdf-edit') return pdfEditJob(input)
  return fail(kind, gapKey(engineGap(kind, fileKind)))
}

async function merge(input: PdfToolRunInput): Promise<PdfToolRunResult> {
  const options = validateLongImage(input.longImage ?? DEFAULT_LONG_IMAGE)
  const loaded = await loadRasters(input)
  const files = await longImageFiles(loaded.rasters, options, input.signal)
  return done('merge', files, { note: loaded.note })
}

async function longImageFiles(
  rasters: readonly Raster[],
  options: LongImageOptions,
  signal?: AbortSignal
): Promise<ToolOutput[]> {
  const files: ToolOutput[] = []
  let bytesTotal = 0
  for await (const segment of composeLongImage(rasters, options, signal)) {
    throwIfAborted(signal)
    const bytes =
      options.imageFormat === 'png'
        ? segment.bytes
        : await encodePdfRaster(await decodePng(segment.bytes), options.imageFormat)
    bytesTotal += bytes.byteLength
    if (bytesTotal > RASTER_OUTPUT_BYTE_LIMIT) throw new Error('image-output-limit')
    files.push({
      name: `long-${files.length + 1}.${options.imageFormat === 'png' ? 'png' : 'jpg'}`,
      mediaType: `image/${options.imageFormat}`,
      bytes,
      width: segment.width,
      height: segment.height
    })
  }
  throwIfAborted(signal)
  if (files.length === 1) files[0] = { ...files[0], name: files[0].name.replace('long-1.', 'long.') }
  return files
}

async function imagePdf(input: PdfToolRunInput): Promise<PdfToolRunResult> {
  const options = validateImagePdf(input.imagePdf ?? DEFAULT_IMAGE_PDF)
  const loaded = await loadRasters(input)
  const pngs: Uint8Array[] = []
  for (const raster of loaded.rasters) {
    throwIfAborted(input.signal)
    pngs.push(await encodePng(raster))
  }
  const pdf = await imagesToPdf(pngs, options)
  throwIfAborted(input.signal)
  if (pdf.byteLength > RASTER_OUTPUT_BYTE_LIMIT) throw new Error('image-output-limit')
  return done('image-to-pdf', [output('images.pdf', PDF, pdf)], { note: loaded.note, dpi: IMAGE_PDF_DPI })
}

async function rotateJob(input: PdfToolRunInput, fileKind: ToolFileKind): Promise<PdfToolRunResult> {
  if (fileKind === 'pdf') {
    const file = input.files[0]
    if (!file) return fail('rotate', KEY.unreadable)
    const edit = input.rasterEdit ? validateRasterEdit(input.rasterEdit) : undefined
    if (
      edit &&
      (edit.straighten !== 0 || edit.corners.some((p, i) => p.x !== [0, 1, 1, 0][i] || p.y !== [0, 0, 1, 1][i]))
    )
      throw new Error('raster-edit-pdf')
    const turned = await rotatePdf(file.bytes, edit ? edit.quarterTurns * 90 : 90)
    throwIfAborted(input.signal)
    const out = output(swapExt(file.name, 'pdf'), PDF, turned)
    if (!input.confirm) return preview('rotate', [out])
    return done('rotate', [out])
  }
  const loaded = await loadRasters(input.rasterEdit ? { ...input, autoCrop: false } : input)
  const files: ToolOutput[] = []
  const sourceFiles: ToolOutput[] = []
  for (const [index, raster] of loaded.rasters.entries()) {
    throwIfAborted(input.signal)
    let next = input.rasterEdit
      ? await applyRasterEdit(raster, input.rasterEdit, input.signal)
      : input.rotate === false
        ? raster
        : rotate90(raster)
    if (input.crop) next = crop(next, input.crop)
    const name = input.files[index]?.name ?? `page-${index + 1}.png`
    files.push({ ...output(swapExt(name, 'png'), PNG, await encodePng(next)), width: next.width, height: next.height })
    sourceFiles.push({
      ...output(swapExt(name, 'png'), PNG, await encodePng(raster)),
      width: raster.width,
      height: raster.height
    })
  }
  if (!input.confirm) return { ...preview('rotate', files, loaded.note), sourceFiles }
  return done('rotate', files, { note: loaded.note })
}

async function filterJob(input: PdfToolRunInput): Promise<PdfToolRunResult> {
  const loaded: { rasters: Raster[]; names: string[] } = { rasters: [], names: [] }
  for (const file of input.files) {
    throwIfAborted(input.signal)
    const rasters =
      fileKindOf(file) === 'pdf' ? (await pdfiumPages(file.bytes, PDF_RASTER_DPI)).pages : [await rasterOf(file)]
    for (const [index, raster] of rasters.entries()) {
      loaded.rasters.push(raster)
      loaded.names.push(
        fileKindOf(file) === 'pdf'
          ? `${file.name.replace(/\.[^.]+$/, '')}-page-${index + 1}.png`
          : swapExt(file.name, 'png')
      )
    }
  }
  const settings = validateFilterDocument(
    input.filterDocument ?? { default: { name: input.filter ?? 'document', intensity: 100 }, pages: {} },
    loaded.rasters.length
  )
  const previews: FilterPreview[] = []
  for (const name of input.filterDocument ? [] : FILTERS) {
    throwIfAborted(input.signal)
    previews.push({ filter: name, bytes: await encodePng(applyFilter(loaded.rasters[0] ?? emptyRaster(), name)) })
  }
  const files: ToolOutput[] = [],
    sourceFiles: ToolOutput[] = []
  for (const [index, raster] of loaded.rasters.entries()) {
    throwIfAborted(input.signal)
    const sourcePage = index + 1
    const name = loaded.names[index] ?? `page-${sourcePage}.png`
    const edited = await applyFilterEdit(raster, filterPageEdit(settings, sourcePage), input.signal)
    files.push({
      ...output(name, PNG, await encodePng(edited)),
      width: raster.width,
      height: raster.height,
      sourcePage
    })
    sourceFiles.push({
      ...output(name, PNG, await encodePng(raster)),
      width: raster.width,
      height: raster.height,
      sourcePage
    })
  }
  if (!input.confirm)
    return {
      state: 'preview',
      kind: 'filter',
      files,
      previews,
      sourceFiles,
      note: KEY.confirm,
      confirmation: done('filter', files)
    }
  return done('filter', files)
}

async function pixelJob(
  kind: 'handwriting' | 'watermark' | 'erase',
  input: PdfToolRunInput
): Promise<PdfToolRunResult> {
  if (!input.maskedEditUrl) return fail(kind, gapKey('inpaint'))
  const loaded = await loadRasters(input)
  const files: ToolOutput[] = []
  for (const [index, raster] of loaded.rasters.entries()) {
    throwIfAborted(input.signal)
    const edited = await maskedEdit(await encodePng(raster), input.maskedEditUrl, fetchOf(input))
    const name = input.files[index]?.name ?? `page-${index + 1}.png`
    files.push(output(swapExt(name, 'png'), PNG, edited))
  }
  if (!input.confirm) return preview(kind, files, loaded.note)
  return done(kind, files, { note: loaded.note })
}

async function pdfRaster(kind: 'pdf-to-image' | 'pdf-to-long', input: PdfToolRunInput): Promise<PdfToolRunResult> {
  const options = input.pdfExport ?? DEFAULT_PDF_EXPORT
  const dpi = parsePdfExportDpi(options.dpi)
  const format = parsePdfImageFormat(options.imageFormat)
  const pages: Raster[] = []
  const files: ToolOutput[] = []
  for (const file of input.files) {
    throwIfAborted(input.signal)
    const rendered = await pdfiumPages(file.bytes, dpi, { pageRange: options.pageRange, signal: input.signal })
    if (kind === 'pdf-to-long') pages.push(...rendered.pages)
    else {
      const stem = file.name.replace(/\.[^.]+$/, '') || 'document'
      for (const [index, page] of rendered.pages.entries()) {
        throwIfAborted(input.signal)
        const sourcePage = rendered.indices[index] + 1
        const bytes = await encodePdfRaster(page, format)
        files.push({
          name: `${stem}-page-${sourcePage}.${format === 'jpeg' ? 'jpg' : 'png'}`,
          mediaType: `image/${format}`,
          bytes,
          width: page.width,
          height: page.height,
          sourcePage
        })
      }
    }
  }
  if (kind === 'pdf-to-long') {
    if (pages.length === 0) return fail(kind, KEY.unreadable)
    const files = await longImageFiles(
      pages,
      { ...validateLongImage(input.longImage ?? DEFAULT_LONG_IMAGE), imageFormat: format },
      input.signal
    )
    return done(kind, files, { dpi })
  }
  if (files.length === 0) return fail(kind, KEY.unreadable)
  throwIfAborted(input.signal)
  return done(kind, files, { dpi })
}

async function officeJob(
  kind: 'word-to-pdf' | 'word-to-image' | 'excel-to-pdf' | 'excel-to-image',
  input: PdfToolRunInput
): Promise<PdfToolRunResult> {
  const files: ToolOutput[] = []
  const options = validateOfficeExport(input.officeExport ?? DEFAULT_OFFICE_EXPORT)
  const imageOutput = kind === 'word-to-image' || kind === 'excel-to-image'
  for (const file of input.files) {
    throwIfAborted(input.signal)
    if (kind === 'word-to-pdf') {
      const pdf = await wordLayoutPdf(file.bytes, file.name, options.pageRange, input.signal)
      files.push(output(swapExt(file.name, 'pdf'), PDF, pdf))
      continue
    }
    const capture = await captureOfficePages(file.bytes, file.name, input.signal, options.pageRange)
    throwIfAborted(input.signal)
    if (imageOutput) {
      const stem = file.name.replace(/\.[^.]+$/, '') || 'page'
      for (const [index, png] of capture.pages.entries()) {
        throwIfAborted(input.signal)
        const raster = await decodePng(png)
        const sourcePage = capture.sourcePages[index]
        const ext = options.imageFormat === 'jpeg' ? 'jpg' : 'png'
        const bytes = options.imageFormat === 'png' ? png : await encodePdfRaster(raster, options.imageFormat)
        files.push({
          name: `${stem}-${sourcePage}.${ext}`,
          mediaType: `image/${options.imageFormat}`,
          bytes,
          width: raster.width,
          height: raster.height,
          ...(kind === 'excel-to-image' ? { sourceSheet: sourcePage } : { previewPage: sourcePage })
        })
      }
    } else {
      // The semantic engine has no positioned per-page text layer. Repeating all
      // source text on every page would add unrelated, duplicated visible content.
      const pdf = await pngsToPdf(capture.pages, OFFICE_CAPTURE_DPI)
      throwIfAborted(input.signal)
      files.push(output(swapExt(file.name, 'pdf'), PDF, pdf))
    }
  }
  return done(kind, files, {
    dpi: OFFICE_CAPTURE_DPI,
    note:
      kind === 'word-to-pdf'
        ? 'workbench.feature.pdf.tools.job.office_layout_pdf'
        : 'workbench.feature.pdf.tools.job.office_semantic' + (imageOutput ? '_image' : '_pdf')
  })
}

async function extractJob(input: PdfToolRunInput, fileKind: ToolFileKind): Promise<PdfToolRunResult> {
  const text = await recognizedText(input, fileKind)
  throwIfAborted(input.signal)
  return done('extract', [output('extract.txt', TEXT, utf8(text))])
}

async function wordJob(
  kind: 'pdf-to-word' | 'image-to-word',
  input: PdfToolRunInput,
  fileKind: ToolFileKind
): Promise<PdfToolRunResult> {
  const text = await recognizedText(input, fileKind)
  throwIfAborted(input.signal)
  const docx = await docxFromPlainText(`${LAYOUT_LABEL}\n${text}`)
  return done(kind, [output('layout.docx', DOCX_MEDIA_TYPE, docx)], { label: LAYOUT_LABEL, note: KEY.layout })
}

async function excelJob(
  kind: 'pdf-to-excel' | 'image-to-excel',
  input: PdfToolRunInput,
  fileKind: ToolFileKind
): Promise<PdfToolRunResult> {
  const rows = await recognizedTable(input, fileKind)
  if (!rows) return fail(kind, KEY.noTable)
  throwIfAborted(input.signal)
  const xlsx = await xlsxFromRows(rows)
  return done(kind, [output('table.xlsx', XLSX_MEDIA_TYPE, xlsx)])
}

async function audioJob(input: PdfToolRunInput): Promise<PdfToolRunResult> {
  const file = input.files[0]
  if (!file) return fail('audio', KEY.unreadable)
  if (!input.gatewayBase) return fail('audio', gapKey('asr'))
  const spoken = await transcribe(file, input)
  const minutes = officialWritingDraft('minutes', spoken.text)
  if (!minutes) return fail('audio', KEY.failed)
  throwIfAborted(input.signal)
  const docx = await docxFromPlainText(minutes)
  return done('audio', [output('minutes.docx', DOCX_MEDIA_TYPE, docx)], {
    minutes,
    transcript: spoken.text,
    streamed: spoken.streamed,
    partials: spoken.partials
  })
}

async function scanJob(input: PdfToolRunInput, fileKind: ToolFileKind): Promise<PdfToolRunResult> {
  const rasters =
    fileKind === 'pdf' ? await pdfRasters(input) : (await loadRasters({ ...input, autoCrop: true })).rasters
  const filtered = rasters.map((raster) => applyFilter(cropMargins(raster).raster, 'document'))
  let pngs: Uint8Array[] = []
  for (const raster of filtered) {
    throwIfAborted(input.signal)
    pngs.push(await encodePng(raster))
  }
  if (input.maskedEditUrl) {
    const next: Uint8Array[] = []
    for (const png of pngs) {
      throwIfAborted(input.signal)
      next.push(await maskedEdit(png, input.maskedEditUrl, fetchOf(input)))
    }
    pngs = next
  }
  const pdf = await imagesToPdf(pngs)
  throwIfAborted(input.signal)
  const file = output('scan.pdf', PDF, pdf)
  if (input.upload) {
    await input.upload({ name: file.name, type: file.mediaType, bytes: file.bytes })
    throwIfAborted(input.signal)
    return done('scan', [file], { dpi: IMAGE_PDF_DPI, localOnly: false })
  }
  return done('scan', [file], { dpi: IMAGE_PDF_DPI, localOnly: true, copy: LOCAL_ONLY_COPY, note: KEY.localOnly })
}

async function pdfEditJob(input: PdfToolRunInput): Promise<PdfToolRunResult> {
  const file = input.files[0]
  if (!file) return fail('pdf-edit', KEY.unreadable)
  const rendered = await pdfiumPages(file.bytes, PDF_RASTER_DPI)
  throwIfAborted(input.signal)
  const hasText = rendered.texts.some((text) => text.trim().length > 0)
  const original = output(file.name, PDF, file.bytes)
  if (hasText || !input.maskedEditUrl) {
    return done('pdf-edit', [original], { editor: 'pdf', inImageText: false, note: KEY.pdfEdit, dpi: PDF_RASTER_DPI })
  }
  const ocr = ocrCaller(input)
  if (!ocr) return done('pdf-edit', [original], { editor: 'pdf', inImageText: false, note: KEY.pdfEdit })
  const page = rendered.pages[0]
  if (!page) return fail('pdf-edit', KEY.unreadable)
  const payload = await ocr(await pngOf(page))
  const placed = textBoxes(payload)
  if (placed.length === 0) return done('pdf-edit', [original], { editor: 'pdf', inImageText: false, note: KEY.pdfEdit })
  const mask = await encodePng(strokeMask(page, placed))
  const edited = await maskedEdit(await pngOf(page), input.maskedEditUrl, fetchOf(input), mask)
  throwIfAborted(input.signal)
  const previewFile = output('edited.png', PNG, edited)
  const pdf = await pngsToPdf([edited], PDF_RASTER_DPI, '', [await fitBoxes(placed, page, edited)])
  const confirmed = done('pdf-edit', [output('edited.pdf', PDF, pdf)], {
    editor: 'pdf',
    inImageText: true,
    dpi: PDF_RASTER_DPI,
    label: placed.map((box) => box.text).join('\n')
  })
  if (!input.confirm) return preview('pdf-edit', [previewFile], undefined, confirmed)
  return confirmed
}

async function recognizedText(input: PdfToolRunInput, fileKind: ToolFileKind): Promise<string> {
  if (fileKind === 'image') {
    const loaded = await loadRasters(input)
    const parts: string[] = []
    for (const raster of loaded.rasters) {
      throwIfAborted(input.signal)
      parts.push(await ocrRequired(input, await encodePng(raster)))
    }
    return parts.filter((part) => part.length > 0).join('\n')
  }
  const parts: string[] = []
  for (const file of input.files) {
    throwIfAborted(input.signal)
    const rendered = await pdfiumPages(file.bytes, PDF_RASTER_DPI)
    for (const [index, page] of rendered.pages.entries()) {
      const layer = rendered.texts[index]?.trim() ?? ''
      if (layer) {
        parts.push(layer)
        continue
      }
      parts.push(await ocrRequired(input, await pngOf(page)))
    }
  }
  return parts.filter((part) => part.length > 0).join('\n')
}

async function recognizedTable(input: PdfToolRunInput, fileKind: ToolFileKind): Promise<string[][] | null> {
  const blocks: OcrBlock[] = []
  if (fileKind === 'image') {
    const loaded = await loadRasters(input)
    for (const raster of loaded.rasters) {
      throwIfAborted(input.signal)
      const payload = await ocrPayload(input, await encodePng(raster))
      blocks.push(...(payload.blocks ?? []))
    }
    return tableRows(blocks)
  }
  let sawText = false
  for (const file of input.files) {
    throwIfAborted(input.signal)
    const rendered = await pdfiumPages(file.bytes, PDF_RASTER_DPI)
    for (const [index, page] of rendered.pages.entries()) {
      const layer = rendered.texts[index]?.trim() ?? ''
      if (layer) {
        sawText = true
        continue
      }
      const payload = await ocrPayload(input, await pngOf(page))
      blocks.push(...(payload.blocks ?? []))
    }
  }
  const rows = tableRows(blocks)
  if (rows) return rows
  if (sawText || blocks.length > 0) return null
  return null
}

async function loadRasters(input: PdfToolRunInput): Promise<{ rasters: Raster[]; note?: string }> {
  if (input.files.reduce((sum, file) => sum + file.bytes.byteLength, 0) > RASTER_OUTPUT_BYTE_LIMIT)
    throw new Error('image-output-limit')
  const useCrop = input.autoCrop !== false
  const rasters: Raster[] = []
  let kept = false
  let pixels = 0
  for (const file of input.files) {
    throwIfAborted(input.signal)
    let raster = await rasterOf(file)
    if (useCrop) {
      const cropped = cropMargins(raster)
      raster = cropped.raster
      if (cropped.kept) kept = true
    }
    pixels += raster.width * raster.height
    if (pixels > RASTER_PIXEL_LIMIT) throw new Error('raster-limit')
    rasters.push(raster)
  }
  return { rasters, note: useCrop && kept ? KEY.cropKept : undefined }
}

async function pdfRasters(input: PdfToolRunInput): Promise<Raster[]> {
  const rasters: Raster[] = []
  for (const file of input.files) {
    throwIfAborted(input.signal)
    const rendered = await pdfiumPages(file.bytes, PDF_RASTER_DPI)
    for (const page of rendered.pages) {
      const cropped = cropMargins(page)
      rasters.push(cropped.raster)
    }
  }
  return rasters
}

async function rasterOf(file: ToolBytes): Promise<Raster> {
  if (isPng(file.bytes)) return decodePng(file.bytes)
  const browser = await rasterFromBrowser(file)
  if (browser) return browser
  try {
    const sharpModule = (await import(/* @vite-ignore */ 'sharp')) as {
      default: (input: Uint8Array, options: { limitInputPixels: number }) => SharpPipe
    }
    const { Buffer: NodeBuffer } = await import(/* @vite-ignore */ 'node:buffer')
    const { data, info } = await sharpModule
      .default(NodeBuffer.from(file.bytes), { limitInputPixels: RASTER_PIXEL_LIMIT })
      .ensureAlpha()
      .raw()
      .toBuffer({ resolveWithObject: true })
    validateRasterSize(info.width, info.height)
    return { width: info.width, height: info.height, rgba: new Uint8Array(data) }
  } catch {
    throw new Error('unreadable')
  }
}

type SharpPipe = {
  ensureAlpha: () => SharpPipe
  raw: () => {
    toBuffer: (options: {
      resolveWithObject: true
    }) => Promise<{ data: Buffer; info: { width: number; height: number } }>
  }
}

async function rasterFromBrowser(file: ToolBytes): Promise<Raster | null> {
  if (typeof createImageBitmap !== 'function' || typeof document === 'undefined') return null
  const bitmap = await createImageBitmap(new Blob([file.bytes.slice()], { type: file.type || PNG })).catch(() => null)
  if (!bitmap) return null
  try {
    validateRasterSize(bitmap.width, bitmap.height)
    const canvas = document.createElement('canvas')
    canvas.width = bitmap.width
    canvas.height = bitmap.height
    const context = canvas.getContext('2d')
    if (!context) return null
    context.drawImage(bitmap, 0, 0)
    const data = context.getImageData(0, 0, bitmap.width, bitmap.height)
    if (data.data.length !== bitmap.width * bitmap.height * 4) return null
    return { width: bitmap.width, height: bitmap.height, rgba: new Uint8Array(data.data) }
  } finally {
    bitmap.close()
  }
}

async function ocrRequired(input: PdfToolRunInput, png: Uint8Array): Promise<string> {
  return ocrText(await ocrPayload(input, png))
}

async function ocrPayload(input: PdfToolRunInput, png: Uint8Array): Promise<OcrPayload> {
  const call = ocrCaller(input)
  if (!call) throw new Error('ocr')
  return call(png)
}

function ocrCaller(input: PdfToolRunInput): ((png: Uint8Array) => Promise<OcrPayload>) | null {
  if (!input.gatewayBase) return null
  const endpoint = `${input.gatewayBase.replace(/\/$/, '')}/v1/ocr`
  return async (png) => {
    const response = await fetchOf(input)(endpoint, {
      method: 'POST',
      signal: input.signal,
      headers: { 'Content-Type': PNG },
      body: png.slice()
    })
    if (!response.ok) throw new Error('ocr')
    return (await response.json()) as OcrPayload
  }
}

async function maskedEdit(
  png: Uint8Array,
  url: string,
  fetchImpl: typeof fetch,
  mask?: Uint8Array
): Promise<Uint8Array> {
  const body = new FormData()
  body.append('image', new Blob([png.slice()], { type: PNG }), 'page.png')
  if (mask) body.append('mask', new Blob([mask.slice()], { type: PNG }), 'mask.png')
  const response = await fetchImpl(url, { method: 'POST', body })
  if (!response.ok) throw new Error('inpaint')
  const bytes = new Uint8Array(await response.arrayBuffer())
  if (!isPng(bytes)) throw new Error('inpaint')
  return bytes
}

async function transcribe(
  file: ToolBytes,
  input: PdfToolRunInput
): Promise<{ text: string; streamed: boolean; partials: string[] }> {
  const endpoint = `${input.gatewayBase?.replace(/\/$/, '')}/v1/audio/transcriptions`
  const response = await fetchOf(input)(endpoint, {
    method: 'POST',
    signal: input.signal,
    headers: { 'Content-Type': file.type || 'application/octet-stream' },
    body: file.bytes.slice()
  })
  if (!response.ok) throw new Error('asr')
  const type = response.headers.get('content-type') ?? ''
  if (type.includes('text/event-stream') && response.body) {
    const reader = response.body.getReader()
    const decoder = new TextDecoder()
    let buffer = ''
    const partials: string[] = []
    while (true) {
      throwIfAborted(input.signal)
      const step = await reader.read()
      if (step.done) break
      buffer += decoder.decode(step.value, { stream: true })
      const lines = buffer.split('\n')
      buffer = lines.pop() ?? ''
      for (const line of lines) {
        const piece = ssePiece(line)
        if (!piece) continue
        partials.push(piece)
        input.onPartial?.(partials.join(''))
      }
    }
    const tail = ssePiece(buffer)
    if (tail) partials.push(tail)
    return { text: partials.join(''), streamed: true, partials }
  }
  const json = (await response.json()) as { text?: string }
  return { text: json.text?.trim() ?? '', streamed: false, partials: [] }
}

function ssePiece(line: string): string | null {
  const trimmed = line.trim()
  if (!trimmed.startsWith('data:')) return null
  const data = trimmed.slice(5).trim()
  if (!data || data === '[DONE]') return null
  try {
    const json = JSON.parse(data) as { text?: string; delta?: string }
    return json.delta ?? json.text ?? null
  } catch {
    return data
  }
}

function tableRows(blocks: readonly OcrBlock[]): string[][] | null {
  const placed = blocks.filter((block) => block.boundingBox && block.text?.trim())
  const sorted = [...placed].sort((a, b) => a.boundingBox!.y - b.boundingBox!.y || a.boundingBox!.x - b.boundingBox!.x)
  const rows: OcrBlock[][] = []
  for (const block of sorted) {
    const y = block.boundingBox!.y
    const last = rows[rows.length - 1]
    const anchor = last?.[0]?.boundingBox?.y
    if (last && anchor !== undefined && Math.abs(anchor - y) <= 8) last.push(block)
    else rows.push([block])
  }
  if (rows.length < 2 || !rows.some((row) => row.length >= 2)) return null
  return rows.map((row) =>
    row.sort((a, b) => a.boundingBox!.x - b.boundingBox!.x).map((block) => block.text?.trim() ?? '')
  )
}

function textBoxes(payload: OcrPayload): PdfTextBox[] {
  const boxes: PdfTextBox[] = []
  for (const block of payload.blocks ?? []) {
    const text = block.text?.trim() ?? ''
    const box = block.boundingBox
    if (!text || !box || box.width <= 0 || box.height <= 0) continue
    boxes.push({ text, x: box.x, y: box.y, width: box.width, height: box.height })
  }
  return boxes
}

function strokeMask(raster: Raster, boxes: readonly PdfTextBox[]): Raster {
  const rgba = new Uint8Array(raster.width * raster.height * 4)
  for (const box of boxes) {
    const x0 = clampInt(box.x, 0, raster.width)
    const y0 = clampInt(box.y, 0, raster.height)
    const x1 = clampInt(Math.ceil(box.x + box.width), 0, raster.width)
    const y1 = clampInt(Math.ceil(box.y + box.height), 0, raster.height)
    for (let y = y0; y < y1; y++) {
      for (let x = x0; x < x1; x++) {
        const index = (y * raster.width + x) * 4
        if (raster.rgba[index] >= 245 && raster.rgba[index + 1] >= 245 && raster.rgba[index + 2] >= 245) continue
        rgba[index] = 255
        rgba[index + 1] = 255
        rgba[index + 2] = 255
        rgba[index + 3] = 255
      }
    }
  }
  return { width: raster.width, height: raster.height, rgba }
}

async function fitBoxes(boxes: readonly PdfTextBox[], source: Raster, png: Uint8Array): Promise<PdfTextBox[]> {
  const image = await decodePng(png)
  const sx = source.width > 0 ? image.width / source.width : 1
  const sy = source.height > 0 ? image.height / source.height : 1
  return boxes.map((box) => ({
    text: box.text,
    x: box.x * sx,
    y: box.y * sy,
    width: box.width * sx,
    height: box.height * sy
  }))
}

function clampInt(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Math.floor(value)))
}

function ocrText(payload: OcrPayload): string {
  const fromBlocks = (payload.blocks ?? [])
    .map((block) => block.text?.trim() ?? '')
    .filter((text) => text.length > 0)
    .join('\n')
  const text = payload.text?.trim() ?? ''
  return text || fromBlocks
}

function fetchOf(input: PdfToolRunInput): typeof fetch {
  return input.fetchImpl ?? globalThis.fetch
}

function output(name: string, mediaType: string, bytes: Uint8Array): ToolOutput {
  return { name, mediaType, bytes }
}

function done(
  kind: PdfToolKind,
  files: ToolOutput[],
  extra: Omit<Extract<PdfToolRunResult, { state: 'done' }>, 'state' | 'kind' | 'files'> = {}
): Extract<PdfToolRunResult, { state: 'done' }> {
  return { state: 'done', kind, files, ...extra }
}

function preview(
  kind: PdfToolKind,
  files: ToolOutput[],
  note?: string,
  confirmation = done(kind, files, { note })
): Extract<PdfToolRunResult, { state: 'preview' }> {
  return { state: 'preview', kind, files, note: note ?? KEY.confirm, confirmation }
}

function fail(kind: PdfToolKind | null, reason: string): PdfToolRunResult {
  return { state: 'failed', kind, reason }
}

export function pdfToolFailureReason(error: unknown): string {
  const geometryReason = error instanceof Error ? error.message : ''
  if (
    geometryReason.startsWith('raster-filter-') ||
    geometryReason.startsWith('Invalid raster filter') ||
    geometryReason.startsWith('Invalid filter comparison')
  )
    return 'workbench.feature.pdf.tools.workspace.filters.failed'
  if (geometryReason === 'raster-edit-region') return 'workbench.feature.pdf.tools.workspace.geometry.invalid'
  if (geometryReason === 'raster-edit-size') return 'workbench.feature.pdf.tools.workspace.geometry.too_large'
  if (geometryReason === 'raster-edit-pdf') return 'workbench.feature.pdf.tools.workspace.geometry.pdf_unsupported'
  const service = documentServiceFailureKey(error)
  if (service) return service
  const message = error instanceof Error ? error.message : typeof error === 'string' ? error : ''
  if (message === 'abort') return KEY.cancel
  if (message === 'encrypted') return KEY.encrypted
  if (message === 'unreadable' || message === 'png' || message === 'empty') return KEY.unreadable
  if (message === 'pdf-page-range') return 'workbench.feature.pdf.tools.workspace.pdf.range_error'
  if (message === 'pdf-raster-limit') return 'workbench.feature.pdf.tools.workspace.pdf.size_error'
  if (message === 'pdf-image-format' || message.startsWith('Invalid PDF '))
    return 'workbench.feature.pdf.tools.workspace.pdf.option_error'
  if (message === 'too-long') return 'workbench.feature.pdf.tools.job.too_long'
  if (message === 'long-image-limit' || message === 'raster-limit' || message === 'image-output-limit')
    return 'workbench.feature.pdf.tools.workspace.assembly.limit'
  if (
    message === 'long-image-options' ||
    message === 'image-pdf-options' ||
    message.startsWith('Invalid long-image') ||
    message.startsWith('Invalid image-PDF')
  )
    return 'workbench.feature.pdf.tools.workspace.assembly.options_error'
  if (message === 'ocr') return gapKey('ocr')
  if (message === 'asr') return gapKey('asr')
  if (message === 'inpaint') return gapKey('inpaint')
  if (message === 'office-layout') return 'workbench.feature.pdf.tools.workspace.office.layout_error'
  if (message === 'office') return gapKey('office')
  if (message === 'office-timeout') return 'workbench.feature.pdf.tools.job.office_timeout'
  if (message === 'office-output-limit') return 'workbench.feature.pdf.tools.job.office_output_limit'
  if (message === 'office-page-range') return 'workbench.feature.pdf.tools.workspace.office.range_error'
  if (message === 'office-raster-limit') return 'workbench.feature.pdf.tools.workspace.office.raster_error'
  if (message === 'pdfium') return gapKey('pdfium')
  return KEY.failed
}

function engineGap(kind: PdfToolKind, fileKind: ToolFileKind): Exclude<ToolEngine, 'local'> {
  const engine = engineForKind(kind, fileKind)
  return engine === 'local' ? 'local-editor' : engine
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) throw new Error('abort')
}

function utf8(text: string): Uint8Array {
  return new TextEncoder().encode(text)
}

function isPng(bytes: Uint8Array): boolean {
  return bytes.length >= 8 && bytes[0] === 137 && bytes[1] === 80 && bytes[2] === 78 && bytes[3] === 71
}

function swapExt(name: string, ext: string): string {
  const base = name.replace(/\.[^.]+$/, '') || 'file'
  return `${base}.${ext}`
}

function emptyRaster(): Raster {
  return { width: 1, height: 1, rgba: new Uint8Array([255, 255, 255, 255]) }
}
