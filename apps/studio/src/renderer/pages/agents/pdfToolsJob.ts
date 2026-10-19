/** One conversion job. Buttons only choose the kind and the files. */

export const PDF_TOOL_FILE_LIMIT = 20
export const PDF_TOOL_BYTE_LIMIT = 25 * 1024 * 1024
export const LONG_IMAGE_MAX_EDGE = 8192
export const LONG_IMAGE_MAX_HEIGHT = 32768

export type PdfToolKind =
  | 'extract'
  | 'rotate'
  | 'filter'
  | 'handwriting'
  | 'watermark'
  | 'erase'
  | 'merge'
  | 'pdf-to-word'
  | 'pdf-to-excel'
  | 'pdf-to-image'
  | 'pdf-to-long'
  | 'word-to-pdf'
  | 'word-to-image'
  | 'excel-to-pdf'
  | 'excel-to-image'
  | 'image-to-word'
  | 'image-to-excel'
  | 'image-to-pdf'
  | 'audio'
  | 'pdf-edit'
  | 'scan'

export type ToolFileKind = 'pdf' | 'word' | 'excel' | 'image' | 'audio' | 'unknown'

export type ToolEngine =
  | 'local'
  | 'local-editor'
  | 'office'
  | 'pdfium'
  | 'ocr'
  | 'asr'
  | 'inpaint'
  | 'pdf-editor'
  | 'scan'

export type ToolFailure = 'empty' | 'mixed' | 'type' | 'too-many' | 'too-big'

export type ToolFile = { name: string; type: string; size: number }

export type ToolIntake =
  | { state: 'choose'; kinds: PdfToolKind[] }
  | { state: 'run'; kind: PdfToolKind }
  | { state: 'gap'; kind: PdfToolKind; engine: Exclude<ToolEngine, 'local'> }
  | { state: 'stop'; failure: ToolFailure }

const EXT: Record<Exclude<ToolFileKind, 'unknown'>, readonly string[]> = {
  pdf: ['pdf'],
  word: ['doc', 'docx'],
  excel: ['xls', 'xlsx'],
  image: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'tif', 'tiff'],
  audio: ['mp3', 'wav', 'm4a', 'aac', 'flac', 'ogg', 'webm']
}

const LABEL: Record<PdfToolKind, string> = {
  extract: 'workbench.feature.pdf.tools.chip.extract',
  rotate: 'workbench.feature.pdf.tools.chip.rotate',
  filter: 'workbench.feature.pdf.tools.chip.filter',
  handwriting: 'workbench.feature.pdf.tools.chip.handwriting',
  watermark: 'workbench.feature.pdf.tools.chip.watermark',
  erase: 'workbench.feature.pdf.tools.chip.erase',
  merge: 'workbench.feature.pdf.tools.chip.merge',
  'pdf-to-word': 'workbench.feature.pdf.tools.item.pdf_to_word',
  'pdf-to-excel': 'workbench.feature.pdf.tools.item.pdf_to_excel',
  'pdf-to-image': 'workbench.feature.pdf.tools.item.pdf_to_image',
  'pdf-to-long': 'workbench.feature.pdf.tools.item.pdf_to_long',
  'word-to-pdf': 'workbench.feature.pdf.tools.item.word_to_pdf',
  'word-to-image': 'workbench.feature.pdf.tools.item.word_to_image',
  'excel-to-pdf': 'workbench.feature.pdf.tools.item.excel_to_pdf',
  'excel-to-image': 'workbench.feature.pdf.tools.item.excel_to_image',
  'image-to-word': 'workbench.feature.pdf.tools.item.image_to_word',
  'image-to-excel': 'workbench.feature.pdf.tools.item.image_to_excel',
  'image-to-pdf': 'workbench.feature.pdf.tools.item.image_to_pdf',
  audio: 'workbench.feature.pdf.tools.audio.title',
  'pdf-edit': 'workbench.feature.pdf.tools.edit.title',
  scan: 'workbench.feature.pdf.tools.scan.title'
}

const FAILURE_KEY: Record<ToolFailure, string> = {
  empty: 'workbench.feature.pdf.tools.job.empty',
  mixed: 'workbench.feature.pdf.tools.job.mixed',
  type: 'workbench.feature.pdf.tools.job.type',
  'too-many': 'workbench.feature.pdf.tools.job.too_many',
  'too-big': 'workbench.feature.pdf.tools.job.too_big'
}

const GAP_KEY: Record<Exclude<ToolEngine, 'local'>, string> = {
  'local-editor': 'workbench.feature.pdf.tools.job.gap.local',
  office: 'workbench.feature.pdf.tools.job.gap.office',
  pdfium: 'workbench.feature.pdf.tools.job.gap.pdfium',
  ocr: 'workbench.feature.pdf.tools.job.gap.ocr',
  asr: 'workbench.feature.pdf.tools.job.gap.asr',
  inpaint: 'workbench.feature.pdf.tools.job.gap.inpaint',
  'pdf-editor': 'workbench.feature.pdf.tools.job.gap.pdf_edit',
  scan: 'workbench.feature.pdf.tools.job.gap.scan'
}

export const PDF_TOOL_KIND_KEY = 'a3s.pdf-tools.kind'

export function isPdfToolKind(value: string | null | undefined): value is PdfToolKind {
  return typeof value === 'string' && Object.prototype.hasOwnProperty.call(LABEL, value)
}

/** Remember a homepage choice until the format window opens its picker. */
export function stagePdfToolKind(kind: PdfToolKind | null): void {
  if (!kind) {
    localStorage.removeItem(PDF_TOOL_KIND_KEY)
    return
  }
  localStorage.setItem(PDF_TOOL_KIND_KEY, kind)
}

export function takePdfToolKind(): PdfToolKind | null {
  const value = localStorage.getItem(PDF_TOOL_KIND_KEY)
  localStorage.removeItem(PDF_TOOL_KIND_KEY)
  return isPdfToolKind(value) ? value : null
}

export function toolLabelKey(kind: PdfToolKind): string {
  return LABEL[kind]
}

export function failureKey(failure: ToolFailure): string {
  return FAILURE_KEY[failure]
}

export function gapKey(engine: Exclude<ToolEngine, 'local'>): string {
  return GAP_KEY[engine]
}

export function fileKindOf(file: Pick<ToolFile, 'name' | 'type'>): ToolFileKind {
  const ext = file.name.split('.').pop()?.toLowerCase() ?? ''
  for (const kind of Object.keys(EXT) as Exclude<ToolFileKind, 'unknown'>[]) {
    if (EXT[kind].includes(ext)) return kind
  }
  const type = file.type.toLowerCase()
  if (type === 'application/pdf') return 'pdf'
  if (type.startsWith('image/')) return 'image'
  if (type.startsWith('audio/')) return 'audio'
  if (type.includes('wordprocessingml') || type === 'application/msword') return 'word'
  if (type.includes('spreadsheetml') || type === 'application/vnd.ms-excel') return 'excel'
  return 'unknown'
}

export function intakeAccept(kind: PdfToolKind | null): { accept: string; multiple: boolean } {
  if (kind === null || kind === 'scan') {
    return {
      accept:
        '.pdf,.doc,.docx,.xls,.xlsx,.png,.jpg,.jpeg,.gif,.webp,.bmp,.tif,.tiff,.mp3,.wav,.m4a,.aac,.flac,.ogg,.webm',
      multiple: true
    }
  }
  if (kind === 'audio') return { accept: '.mp3,.wav,.m4a,.aac,.flac,.ogg,.webm,audio/*', multiple: false }
  if (kind === 'merge' || kind === 'image-to-pdf') {
    return { accept: '.png,.jpg,.jpeg,.gif,.webp,.bmp,.tif,.tiff,image/*', multiple: true }
  }
  const kinds = acceptedKinds(kind)
  const accept = kinds.flatMap((item) => EXT[item].map((ext) => `.${ext}`)).join(',')
  return { accept, multiple: kind !== 'pdf-edit' }
}

export function intake(kind: PdfToolKind | null, files: readonly ToolFile[]): ToolIntake {
  if (files.length === 0) return { state: 'stop', failure: 'empty' }
  if (files.length > PDF_TOOL_FILE_LIMIT) return { state: 'stop', failure: 'too-many' }
  if (files.some((file) => file.size > PDF_TOOL_BYTE_LIMIT)) return { state: 'stop', failure: 'too-big' }
  const kinds = files.map(fileKindOf)
  if (kinds.some((item) => item === 'unknown')) return { state: 'stop', failure: 'type' }
  const distinct = new Set(kinds)
  if (distinct.size > 1) return { state: 'stop', failure: 'mixed' }
  const fileKind = kinds[0]
  if (!fileKind) return { state: 'stop', failure: 'empty' }
  if (fileKind === 'unknown') return { state: 'stop', failure: 'type' }
  if (kind === null) {
    const choices = legalKinds(fileKind)
    if (choices.length === 0) return { state: 'stop', failure: 'type' }
    return { state: 'choose', kinds: choices }
  }
  if (!acceptedKinds(kind).includes(fileKind)) return { state: 'stop', failure: 'type' }
  return { state: 'run', kind }
}

/** The engine a kind still needs. The runner uses it when that engine is not configured. */
export function engineForKind(kind: PdfToolKind, fileKind: ToolFileKind): ToolEngine {
  return engineFor(kind, fileKind)
}

export function longImageLayout(
  sizes: readonly { width: number; height: number }[]
): { width: number; height: number; rows: { height: number }[] } | { error: 'empty' | 'too-long' } {
  if (sizes.length === 0 || sizes.some((size) => size.width <= 0 || size.height <= 0)) return { error: 'empty' }
  const width = Math.min(LONG_IMAGE_MAX_EDGE, Math.max(...sizes.map((size) => size.width)))
  const rows = sizes.map((size) => ({ height: Math.max(1, Math.round((size.height * width) / size.width)) }))
  const height = rows.reduce((sum, row) => sum + row.height, 0)
  if (height > LONG_IMAGE_MAX_HEIGHT) return { error: 'too-long' }
  return { width, height, rows }
}

function acceptedKinds(kind: PdfToolKind): Exclude<ToolFileKind, 'unknown'>[] {
  switch (kind) {
    case 'merge':
    case 'handwriting':
    case 'watermark':
    case 'erase':
    case 'image-to-word':
    case 'image-to-excel':
    case 'image-to-pdf':
      return ['image']
    case 'filter':
    case 'rotate':
    case 'extract':
      return ['image', 'pdf']
    case 'pdf-to-word':
    case 'pdf-to-excel':
    case 'pdf-to-image':
    case 'pdf-to-long':
    case 'pdf-edit':
      return ['pdf']
    case 'word-to-pdf':
    case 'word-to-image':
      return ['word']
    case 'excel-to-pdf':
    case 'excel-to-image':
      return ['excel']
    case 'audio':
      return ['audio']
    case 'scan':
      return ['image', 'pdf']
    default:
      return []
  }
}

function legalKinds(fileKind: Exclude<ToolFileKind, 'unknown'>): PdfToolKind[] {
  switch (fileKind) {
    case 'image':
      return [
        'merge',
        'image-to-pdf',
        'image-to-word',
        'image-to-excel',
        'extract',
        'rotate',
        'filter',
        'handwriting',
        'watermark',
        'erase'
      ]
    case 'pdf':
      return ['pdf-to-word', 'pdf-to-excel', 'pdf-to-image', 'pdf-to-long', 'extract', 'pdf-edit', 'filter']
    case 'word':
      return ['word-to-pdf', 'word-to-image']
    case 'excel':
      return ['excel-to-pdf', 'excel-to-image']
    case 'audio':
      return ['audio']
    default:
      return []
  }
}

function engineFor(kind: PdfToolKind, fileKind: ToolFileKind): ToolEngine {
  if (kind === 'merge') return 'local'
  if (kind === 'rotate' || kind === 'extract')
    return fileKind === 'pdf' ? 'pdfium' : kind === 'extract' ? 'ocr' : 'local-editor'
  if (kind === 'filter') return fileKind === 'pdf' ? 'pdfium' : 'local-editor'
  if (kind === 'image-to-pdf') return 'local-editor'
  if (kind === 'handwriting' || kind === 'watermark' || kind === 'erase') return 'inpaint'
  if (kind === 'pdf-to-image' || kind === 'pdf-to-long') return 'pdfium'
  if (kind === 'pdf-to-word' || kind === 'pdf-to-excel' || kind === 'image-to-word' || kind === 'image-to-excel')
    return 'ocr'
  if (kind === 'word-to-pdf' || kind === 'word-to-image' || kind === 'excel-to-pdf' || kind === 'excel-to-image')
    return 'office'
  if (kind === 'audio') return 'asr'
  if (kind === 'pdf-edit') return 'pdf-editor'
  return 'scan'
}
