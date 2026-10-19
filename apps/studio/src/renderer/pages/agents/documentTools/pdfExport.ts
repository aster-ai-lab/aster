export const PDF_EXPORT_DPI = [72, 144, 288] as const
export const PDF_IMAGE_FORMATS = ['png', 'jpeg'] as const
export type PdfImageFormat = (typeof PDF_IMAGE_FORMATS)[number]
export type PdfExportDpi = (typeof PDF_EXPORT_DPI)[number]
export type PdfExportOptions = Readonly<{
  pageRange: string
  dpi: PdfExportDpi
  imageFormat: PdfImageFormat
}>
export const DEFAULT_PDF_EXPORT: PdfExportOptions = Object.freeze({
  pageRange: '',
  dpi: 144,
  imageFormat: 'png'
})
export const PDF_PAGE_PIXEL_LIMIT = 16_777_216
export const PDF_EXPORT_PIXEL_LIMIT = 33_554_432

export function parsePdfImageFormat(value: unknown): PdfImageFormat {
  if (typeof value !== 'string' || !PDF_IMAGE_FORMATS.includes(value as PdfImageFormat)) {
    throw new Error('Invalid PDF image format')
  }
  return value as PdfImageFormat
}

export function pdfImageFormatAsStr(value: PdfImageFormat): string {
  return parsePdfImageFormat(value)
}

export function parsePdfExportDpi(value: unknown): PdfExportDpi {
  if (typeof value !== 'number' || !PDF_EXPORT_DPI.includes(value as PdfExportDpi)) {
    throw new Error('Invalid PDF export resolution')
  }
  return value as PdfExportDpi
}

export function pdfExportDpiAsStr(value: PdfExportDpi): string {
  return String(parsePdfExportDpi(value))
}

/** One-based user pages, zero-based engine indices. Preserve order and remove duplicates. */
export function selectedPdfPages(range: string, pageCount: number): readonly number[] {
  if (!Number.isSafeInteger(pageCount) || pageCount <= 0 || pageCount > 10_000) throw new Error('unreadable')
  if (range.length > 1024) throw new Error('pdf-page-range')
  if (!range.trim()) return Array.from({ length: pageCount }, (_, index) => index)
  const pages = new Set<number>()
  for (const part of range.split(',')) {
    const match = /^\s*([1-9]\d*)(?:\s*-\s*([1-9]\d*))?\s*$/.exec(part)
    if (!match) throw new Error('pdf-page-range')
    const start = Number(match[1])
    const end = Number(match[2] ?? match[1])
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || end < start || end > pageCount) {
      throw new Error('pdf-page-range')
    }
    for (let page = start; page <= end; page++) pages.add(page - 1)
  }
  return [...pages]
}

export function pdfRasterSize(widthPoints: number, heightPoints: number, dpi: number) {
  if (
    !Number.isFinite(dpi) ||
    dpi <= 0 ||
    dpi > 600 ||
    !Number.isFinite(widthPoints) ||
    !Number.isFinite(heightPoints) ||
    widthPoints <= 0 ||
    heightPoints <= 0
  ) {
    throw new Error('pdf-raster-limit')
  }
  const width = Math.ceil((widthPoints * dpi) / 72)
  const height = Math.ceil((heightPoints * dpi) / 72)
  if (!Number.isSafeInteger(width * height) || width * height > PDF_PAGE_PIXEL_LIMIT)
    throw new Error('pdf-raster-limit')
  return { width, height }
}
