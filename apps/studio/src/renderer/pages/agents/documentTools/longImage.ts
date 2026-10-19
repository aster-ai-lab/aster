import { LONG_IMAGE_MAX_EDGE, LONG_IMAGE_MAX_HEIGHT } from '../pdfToolsJob'
import { encodePng, type Raster } from '../pdfToolsPng'
import { PDF_EXPORT_PIXEL_LIMIT, PDF_PAGE_PIXEL_LIMIT, parsePdfImageFormat, type PdfImageFormat } from './pdfExport'

export const LONG_ALIGNMENTS = ['left', 'center', 'right'] as const
export const LONG_BACKGROUNDS = ['white', 'transparent'] as const
export type LongAlignment = (typeof LONG_ALIGNMENTS)[number]
export type LongBackground = (typeof LONG_BACKGROUNDS)[number]
export type LongImageOptions = Readonly<{
  width: number | null
  gap: number
  alignment: LongAlignment
  background: LongBackground
  upscale: boolean
  segmentHeight: number | null
  imageFormat: PdfImageFormat
}>
export const DEFAULT_LONG_IMAGE: LongImageOptions = Object.freeze({
  width: null,
  gap: 0,
  alignment: 'center',
  background: 'white',
  upscale: false,
  segmentHeight: null,
  imageFormat: 'png'
})

export function parseLongAlignment(value: unknown): LongAlignment {
  if (!LONG_ALIGNMENTS.includes(value as LongAlignment)) throw new Error('Invalid long-image alignment')
  return value as LongAlignment
}
export function longAlignmentAsStr(value: LongAlignment) {
  return parseLongAlignment(value)
}
export function parseLongBackground(value: unknown): LongBackground {
  if (!LONG_BACKGROUNDS.includes(value as LongBackground)) throw new Error('Invalid long-image background')
  return value as LongBackground
}
export function longBackgroundAsStr(value: LongBackground) {
  return parseLongBackground(value)
}

export function validateLongImage(options: LongImageOptions): LongImageOptions {
  const bound = (value: number | null, maximum: number) => {
    if (value !== null && (!Number.isSafeInteger(value) || value < 1 || value > maximum))
      throw new Error('long-image-options')
  }
  bound(options.width, LONG_IMAGE_MAX_EDGE)
  bound(options.segmentHeight, LONG_IMAGE_MAX_HEIGHT)
  if (
    !Number.isSafeInteger(options.gap) ||
    options.gap < 0 ||
    options.gap > 1024 ||
    typeof options.upscale !== 'boolean'
  ) {
    throw new Error('long-image-options')
  }
  return Object.freeze({
    ...options,
    alignment: parseLongAlignment(options.alignment),
    background: parseLongBackground(options.background),
    imageFormat: parsePdfImageFormat(options.imageFormat)
  })
}

export function longImagePlan(
  sizes: readonly { width: number; height: number }[],
  settings: LongImageOptions = DEFAULT_LONG_IMAGE
) {
  const options = validateLongImage(settings)
  if (
    !sizes.length ||
    sizes.length > 10_000 ||
    sizes.some(
      (size) =>
        !Number.isSafeInteger(size.width) || !Number.isSafeInteger(size.height) || size.width <= 0 || size.height <= 0
    )
  )
    throw new Error('unreadable')
  const width = options.width ?? Math.min(LONG_IMAGE_MAX_EDGE, Math.max(...sizes.map((size) => size.width)))
  let top = 0
  const rows = sizes.map((size, index) => {
    const rowWidth = options.upscale ? width : Math.min(width, size.width)
    const height = Math.max(1, Math.round((size.height * rowWidth) / size.width))
    const left =
      options.alignment === 'left'
        ? 0
        : options.alignment === 'right'
          ? width - rowWidth
          : Math.floor((width - rowWidth) / 2)
    const row = { width: rowWidth, height, left, top }
    top += height + (index < sizes.length - 1 ? options.gap : 0)
    return row
  })
  if (!Number.isSafeInteger(top * width) || top * width > PDF_EXPORT_PIXEL_LIMIT) throw new Error('long-image-limit')
  const segmentHeight =
    options.segmentHeight === null ? top : Math.min(options.segmentHeight, Math.floor(PDF_PAGE_PIXEL_LIMIT / width))
  if (segmentHeight > LONG_IMAGE_MAX_HEIGHT || segmentHeight * width > PDF_PAGE_PIXEL_LIMIT) throw new Error('too-long')
  const segments: { top: number; height: number }[] = []
  for (let y = 0; y < top; y += segmentHeight) segments.push({ top: y, height: Math.min(segmentHeight, top - y) })
  return { width, height: top, rows, segments }
}

/** Rasterize only one bounded segment at a time; never allocate the full tall image. */
export async function* composeLongImage(
  sources: readonly Raster[],
  settings = DEFAULT_LONG_IMAGE,
  signal?: InstanceType<typeof globalThis.AbortController>['signal']
) {
  const options = validateLongImage(settings)
  const plan = longImagePlan(sources, options)
  for (const source of sources) {
    if (source.rgba.length !== source.width * source.height * 4) throw new Error('unreadable')
  }
  const check = () => {
    if (signal?.aborted) throw new Error('abort')
  }
  for (const segment of plan.segments) {
    check()
    const rgba = new Uint8Array(plan.width * segment.height * 4)
    if (options.background === 'white') rgba.fill(255)
    for (const [index, row] of plan.rows.entries()) {
      const source = sources[index]
      const start = Math.max(segment.top, row.top)
      const end = Math.min(segment.top + segment.height, row.top + row.height)
      for (let y = start; y < end; y++) {
        if (y % 64 === 0) {
          await new Promise<void>((resolve) => globalThis.setTimeout(resolve, 0))
          check()
        }
        const sourceY = Math.min(source.height - 1, Math.floor(((y - row.top) * source.height) / row.height))
        for (let x = 0; x < row.width; x++) {
          const sourceX = Math.min(source.width - 1, Math.floor((x * source.width) / row.width))
          const from = (sourceY * source.width + sourceX) * 4
          const to = ((y - segment.top) * plan.width + row.left + x) * 4
          const alpha = source.rgba[from + 3] / 255
          for (let channel = 0; channel < 3; channel++) {
            rgba[to + channel] =
              options.background === 'white'
                ? Math.round(source.rgba[from + channel] * alpha + 255 * (1 - alpha))
                : source.rgba[from + channel]
          }
          rgba[to + 3] = options.background === 'white' ? 255 : source.rgba[from + 3]
        }
      }
    }
    check()
    const bytes = await encodePng({ width: plan.width, height: segment.height, rgba })
    check()
    yield { bytes, width: plan.width, height: segment.height }
  }
}
