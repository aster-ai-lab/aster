import type { Raster } from '../pdfToolsPng'
import { applyFilter } from '../pdfToolsRaster'
import { rasterSize } from './rasterGeometry'

export const FILTER_KINDS = ['original', 'document', 'grayscale', 'contrast'] as const
export type FilterKind = (typeof FILTER_KINDS)[number]
export type FilterEdit = Readonly<{ name: FilterKind; intensity: number }>
export type FilterDocumentEdit = Readonly<{ default: FilterEdit; pages: Readonly<Record<number, FilterEdit>> }>
export const ORIGINAL_FILTER: FilterEdit = Object.freeze({ name: 'original', intensity: 100 })
export const ORIGINAL_FILTER_DOCUMENT: FilterDocumentEdit = Object.freeze({
  default: ORIGINAL_FILTER,
  pages: Object.freeze({})
})
export const FILTER_COMPARISONS = ['side-by-side', 'split'] as const
export type FilterComparison = (typeof FILTER_COMPARISONS)[number]
export function parseFilterKind(value: unknown): FilterKind {
  if (typeof value !== 'string' || !FILTER_KINDS.includes(value as FilterKind))
    throw new Error('Invalid raster filter kind')
  return value as FilterKind
}
export function filterKindAsStr(value: FilterKind) {
  return parseFilterKind(value)
}
export function parseFilterComparison(value: unknown): FilterComparison {
  if (typeof value !== 'string' || !FILTER_COMPARISONS.includes(value as FilterComparison))
    throw new Error('Invalid filter comparison mode')
  return value as FilterComparison
}
export function filterComparisonAsStr(value: FilterComparison) {
  return parseFilterComparison(value)
}
export function validateFilterEdit(edit: FilterEdit): FilterEdit {
  const name = parseFilterKind(edit?.name)
  if (!Number.isInteger(edit.intensity) || edit.intensity < 0 || edit.intensity > 100)
    throw new Error('raster-filter-intensity')
  return Object.freeze({ name, intensity: edit.intensity })
}
export function validateFilterDocument(edit: FilterDocumentEdit, pageCount?: number): FilterDocumentEdit {
  if (!edit || !edit.pages || typeof edit.pages !== 'object' || Array.isArray(edit.pages))
    throw new Error('raster-filter-pages')
  const pages: Record<number, FilterEdit> = {}
  for (const [key, value] of Object.entries(edit.pages)) {
    const page = Number(key)
    if (
      !/^[1-9]\d*$/.test(key) ||
      !Number.isSafeInteger(page) ||
      page < 1 ||
      (pageCount !== undefined && page > pageCount)
    )
      throw new Error('raster-filter-pages')
    pages[page] = validateFilterEdit(value)
  }
  return Object.freeze({ default: validateFilterEdit(edit.default), pages: Object.freeze(pages) })
}
export function filterEditsEqual(a: FilterEdit, b: FilterEdit) {
  return a.name === b.name && a.intensity === b.intensity
}
export function filterPageEdit(document: FilterDocumentEdit, page: number): FilterEdit {
  return document.pages[page] ?? document.default
}
export function filterDocumentsEqual(a: FilterDocumentEdit, b: FilterDocumentEdit) {
  return (
    filterEditsEqual(a.default, b.default) &&
    [...new Set([...Object.keys(a.pages), ...Object.keys(b.pages)])].every((key) =>
      filterEditsEqual(filterPageEdit(a, Number(key)), filterPageEdit(b, Number(key)))
    )
  )
}

/** Shared preview/export transform: original RGB blend and unchanged alpha. */
export async function applyFilterEdit(
  source: Raster,
  edit: FilterEdit,
  signal?: InstanceType<typeof globalThis.AbortSignal>
): Promise<Raster> {
  const settings = validateFilterEdit(edit)
  if (signal?.aborted) throw new Error('workbench.feature.pdf.tools.job.cancel')
  rasterSize(source.width, source.height)
  if (source.rgba.length !== source.width * source.height * 4) throw new Error('raster-edit-size')
  const rgba = source.rgba.slice()
  const strength = settings.intensity / 100
  for (let y = 0; y < source.height; y++) {
    if (y % 64 === 0) {
      await new Promise<void>((resolve) => globalThis.setTimeout(resolve, 0))
      if (signal?.aborted) throw new Error('workbench.feature.pdf.tools.job.cancel')
    }
    if (settings.name === 'original' || strength === 0) continue
    const offset = y * source.width * 4
    const row = source.rgba.subarray(offset, offset + source.width * 4)
    const filtered = applyFilter({ width: source.width, height: 1, rgba: row }, settings.name).rgba
    for (let x = 0; x < row.length; x += 4)
      for (let channel = 0; channel < 3; channel++) {
        rgba[offset + x + channel] = Math.round(
          row[x + channel] + (filtered[x + channel] - row[x + channel]) * strength
        )
      }
  }
  if (signal?.aborted) throw new Error('workbench.feature.pdf.tools.job.cancel')
  return { width: source.width, height: source.height, rgba }
}

/** Only thumbnail work is downsampled; final pixels always use the original source. */
export function filterThumbnail(source: Raster): Raster {
  rasterSize(source.width, source.height)
  if (source.rgba.length !== source.width * source.height * 4) throw new Error('raster-edit-size')
  const scale = Math.min(1, 128 / Math.max(source.width, source.height))
  const width = Math.max(1, Math.round(source.width * scale)),
    height = Math.max(1, Math.round(source.height * scale))
  const rgba = new Uint8Array(width * height * 4)
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) {
      const offset =
        (Math.min(source.height - 1, Math.floor((y + 0.5) / scale)) * source.width +
          Math.min(source.width - 1, Math.floor((x + 0.5) / scale))) *
        4
      rgba.set(source.rgba.subarray(offset, offset + 4), (y * width + x) * 4)
    }
  return { width, height, rgba }
}
