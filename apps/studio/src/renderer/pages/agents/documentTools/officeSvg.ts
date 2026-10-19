import { OFFICE_CAPTURE_DPI } from '../pdfToolsRaster'
import {
  PDF_EXPORT_PIXEL_LIMIT,
  parsePdfImageFormat,
  pdfRasterSize,
  selectedPdfPages,
  type PdfImageFormat
} from './pdfExport'

export type OfficeExportOptions = Readonly<{ pageRange: string; imageFormat: PdfImageFormat }>
export const DEFAULT_OFFICE_EXPORT: OfficeExportOptions = Object.freeze({ pageRange: '', imageFormat: 'png' })
export type OfficeSvgPage = Readonly<{ svg: string; width: number; height: number; sourcePage: number }>
type Box = { x: number; y: number; width: number; height: number }
const WORD_PAGE_UNITS = (1000 * 11) / 8.5
const { TextEncoder } = globalThis
const SVG_BYTE_LIMIT = 25 * 1024 * 1024
const PROJECTED_SVG_BYTE_LIMIT = 64 * 1024 * 1024

export function validateOfficeExport(options: OfficeExportOptions): OfficeExportOptions {
  if (typeof options.pageRange !== 'string' || options.pageRange.length > 1024) throw new Error('office-page-range')
  return Object.freeze({ pageRange: options.pageRange, imageFormat: parsePdfImageFormat(options.imageFormat) })
}

/** The native engine supplies semantic slices, not source Office print pagination. */
export function officeSvgPages(svg: string, range = ''): readonly OfficeSvgPage[] {
  if (svg.length > SVG_BYTE_LIMIT) throw new Error('office-output-limit')
  const svgBytes = new TextEncoder().encode(svg).byteLength
  if (svgBytes > SVG_BYTE_LIMIT) throw new Error('office-output-limit')
  const root = /<svg\b([^>]*)>/.exec(svg)
  const viewBox = root && /\bviewBox="([^"]+)"/.exec(root[1])
  const values = viewBox?.[1]
    .trim()
    .split(/[\s,]+/)
    .map(Number)
  if (!values || values.length !== 4 || !values.every(Number.isFinite) || values[2] <= 0 || values[3] <= 0)
    throw new Error('unreadable')
  const box: Box = { x: values[0], y: values[1], width: values[2], height: values[3] }
  const boxes: Box[] = []
  if (root?.[1].includes('data-document-kind="spreadsheet"')) {
    for (const group of topLevelGroups(svg)) {
      const rect = /<rect class="sheet" x="[0-9.]+" y="([0-9.]+)" width="[0-9.]+" height="([0-9.]+)"/.exec(group)
      if (!rect) continue
      const y = Number(rect[1])
      const height = Number(rect[2])
      if (
        !Number.isFinite(y) ||
        !Number.isFinite(height) ||
        height <= 0 ||
        y < box.y ||
        y + height > box.y + box.height
      )
        throw new Error('unreadable')
      boxes.push({ ...box, y, height })
      if (boxes.length > 10_000) throw new Error('office-raster-limit')
    }
    if (boxes.length === 0) throw new Error('unreadable')
  } else {
    const count = Math.ceil(box.height / WORD_PAGE_UNITS)
    if (!Number.isSafeInteger(count) || count > 10_000) throw new Error('office-raster-limit')
    for (let index = 0; index < count; index++) {
      const offset = index * WORD_PAGE_UNITS
      boxes.push({ ...box, y: box.y + offset, height: Math.min(WORD_PAGE_UNITS, box.height - offset) })
    }
  }
  let selected: readonly number[]
  try {
    selected = selectedPdfPages(range, boxes.length)
  } catch {
    throw new Error('office-page-range')
  }
  if ((svgBytes + 160) * selected.length > PROJECTED_SVG_BYTE_LIMIT) throw new Error('office-raster-limit')
  let pixels = 0
  return selected.map((index) => {
    const page = boxes[index]
    let size: { width: number; height: number }
    try {
      size = pdfRasterSize(page.width, page.height, OFFICE_CAPTURE_DPI)
    } catch {
      throw new Error('office-raster-limit')
    }
    pixels += size.width * size.height
    if (pixels > PDF_EXPORT_PIXEL_LIMIT) throw new Error('office-raster-limit')
    const projected = svg.replace(/<svg\b([^>]*)>/, (_match, attributes: string) => {
      const selfClosing = attributes.endsWith('/')
      const attrs = attributes
        .replace(/\/$/, '')
        .replace(/\s(?:width|height)="[^"]*"/g, '')
        .replace(/\bviewBox="[^"]+"/, `viewBox="${page.x} ${page.y} ${page.width} ${page.height}"`)
      return `<svg${attrs} width="${size.width}" height="${size.height}"${selfClosing ? '/>' : '>'}`
    })
    return Object.freeze({ svg: projected, ...size, sourcePage: index + 1 })
  })
}

function topLevelGroups(svg: string): string[] {
  const groups: string[] = []
  let depth = 0
  let start = -1
  for (let index = 0; index < svg.length; index++) {
    if (svg.startsWith('<g', index) && (svg[index + 2] === ' ' || svg[index + 2] === '>')) {
      if (depth === 0) start = index
      depth += 1
    } else if (svg.startsWith('</g>', index) && depth > 0) {
      depth -= 1
      if (depth === 0 && start >= 0) groups.push(svg.slice(start, index + 4))
    }
  }
  return groups
}
