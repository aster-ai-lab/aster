import type { PdfToolKind } from '../pdfToolsJob'
import { runPdfToolJob } from '../pdfToolsRun'
import type { PdfToolRunInput, PdfToolRunResult, ToolBytes } from '../pdfToolsRun'
import type { DocumentEngine } from './application'
import type { FilterDocumentEdit } from './rasterFilter'
import type { RasterEdit } from './rasterGeometry'
import type { DocumentSource } from './task'

export type DocumentArtifact = Readonly<{
  output: Extract<PdfToolRunResult, { state: 'done' }>
  preview?: Extract<PdfToolRunResult, { state: 'preview' }>
}>

export type DocumentToolOptions = Omit<
  PdfToolRunInput,
  'kind' | 'files' | 'confirm' | 'signal' | 'onPartial' | 'upload'
> & {
  rasterEditsBySource?: Readonly<Record<string, RasterEdit>>
  filterEditsBySource?: Readonly<Record<string, FilterDocumentEdit>>
}

/** Compose one real engine call per source. Grouped outputs keep their owning ordered-page workflow. */
export function documentToolEngine(
  kind: PdfToolKind,
  load: (source: DocumentSource) => Promise<ToolBytes>,
  options: DocumentToolOptions,
  run: typeof runPdfToolJob = runPdfToolJob
): DocumentEngine<DocumentArtifact> {
  const settings = Object.freeze({ ...options })
  return async (source, signal) => {
    const file = await load(source)
    if (signal.aborted) throw new Error('workbench.feature.pdf.tools.job.cancel')
    if (file.name !== source.name || file.type !== source.type || file.bytes.byteLength !== source.size) {
      throw new Error('workbench.feature.pdf.tools.job.unreadable')
    }
    const { rasterEditsBySource, filterEditsBySource, ...runSettings } = settings
    const result = await run({
      ...runSettings,
      rasterEdit: rasterEditsBySource?.[source.id] ?? settings.rasterEdit,
      filterDocument: filterEditsBySource?.[source.id] ?? settings.filterDocument,
      kind,
      files: [file],
      signal,
      confirm: false
    })
    if (result.state === 'failed') throw new Error(result.reason)
    if (result.state === 'choose') throw new Error('workbench.feature.pdf.tools.job.failed')
    return Object.freeze(
      result.state === 'preview' ? { preview: result, output: result.confirmation } : { output: result }
    )
  }
}

export function usesDocumentBatch(kind: PdfToolKind): boolean {
  return kind !== 'merge' && kind !== 'image-to-pdf' && kind !== 'scan' && kind !== 'audio' && kind !== 'pdf-edit'
}

export function isOfficeConversion(kind: PdfToolKind): boolean {
  return ['word-to-pdf', 'word-to-image', 'excel-to-pdf', 'excel-to-image'].includes(kind)
}

/** Format conversion publishes a ready download; corrections retain their review step. */
export function isDocumentConversion(kind: PdfToolKind): boolean {
  return [
    'pdf-to-word',
    'pdf-to-excel',
    'pdf-to-image',
    'pdf-to-long',
    'word-to-pdf',
    'word-to-image',
    'excel-to-pdf',
    'excel-to-image',
    'image-to-word',
    'image-to-excel',
    'image-to-pdf'
  ].includes(kind)
}
