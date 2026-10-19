import type { Raster } from '../pdfToolsPng'
import { rotate90 } from '../pdfToolsRaster'

export type RasterPoint = Readonly<{ x: number; y: number }>
export type RasterCorners = readonly [RasterPoint, RasterPoint, RasterPoint, RasterPoint]
export type RasterEdit = Readonly<{
  corners: RasterCorners
  quarterTurns: number
  straighten: number
}>
export const RASTER_EDIT_PIXEL_LIMIT = 16_777_216
export const RASTER_EDIT_EDGE_LIMIT = 32768
export const ORIGINAL_CORNERS: RasterCorners = Object.freeze([
  Object.freeze({ x: 0, y: 0 }),
  Object.freeze({ x: 1, y: 0 }),
  Object.freeze({ x: 1, y: 1 }),
  Object.freeze({ x: 0, y: 1 })
])
export const ORIGINAL_RASTER_EDIT: RasterEdit = Object.freeze({
  corners: ORIGINAL_CORNERS,
  quarterTurns: 0,
  straighten: 0
})
export const CROP_MODES = ['rectangle', 'document'] as const
export type CropMode = (typeof CROP_MODES)[number]
export function parseCropMode(value: unknown): CropMode {
  if (typeof value !== 'string' || !CROP_MODES.includes(value as CropMode)) throw new Error('Invalid raster crop mode')
  return value as CropMode
}
export function cropModeAsStr(value: CropMode): string {
  return parseCropMode(value)
}
export const CROP_RATIOS = ['free', '1:1', '4:3', '16:9'] as const
export type CropRatio = (typeof CROP_RATIOS)[number]
export function parseCropRatio(value: unknown): CropRatio {
  if (typeof value !== 'string' || !CROP_RATIOS.includes(value as CropRatio))
    throw new Error('Invalid raster crop ratio')
  return value as CropRatio
}
export function cropRatioAsStr(value: CropRatio): string {
  return parseCropRatio(value)
}

export function validateRasterEdit(edit: RasterEdit): RasterEdit {
  if (
    !edit ||
    !Number.isInteger(edit.quarterTurns) ||
    edit.quarterTurns < 0 ||
    edit.quarterTurns > 3 ||
    !Number.isFinite(edit.straighten) ||
    Math.abs(edit.straighten) > 45 ||
    !Array.isArray(edit.corners) ||
    edit.corners.length !== 4 ||
    edit.corners.some((point) => !point || typeof point !== 'object')
  ) {
    throw new Error('raster-edit-region')
  }
  for (let i = 0; i < 4; i++) {
    const a = edit.corners[i],
      b = edit.corners[(i + 1) % 4],
      c = edit.corners[(i + 2) % 4]
    if (
      ![a.x, a.y].every((n) => Number.isFinite(n) && n >= 0 && n <= 1) ||
      (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x) <= 1e-8
    )
      throw new Error('raster-edit-region')
  }
  return Object.freeze({
    ...edit,
    corners: Object.freeze(edit.corners.map((point) => Object.freeze({ ...point }))) as RasterCorners
  })
}

export function rasterSize(width: number, height: number): { width: number; height: number } {
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > RASTER_EDIT_EDGE_LIMIT ||
    height > RASTER_EDIT_EDGE_LIMIT ||
    width * height > RASTER_EDIT_PIXEL_LIMIT
  ) {
    throw new Error('raster-edit-size')
  }
  return { width, height }
}

/** Coordinates describe pixel edges in the original image, not a previous edited result. */
export function rectangleCorners(x: number, y: number, width: number, height: number): RasterCorners {
  return [
    { x, y },
    { x: x + width, y },
    { x: x + width, y: y + height },
    { x, y: y + height }
  ]
}

function projection(corners: RasterCorners, width: number, height: number) {
  const p = corners.map((point) => ({ x: point.x * width, y: point.y * height }))
  const dx1 = p[1].x - p[2].x,
    dx2 = p[3].x - p[2].x,
    dx3 = p[0].x - p[1].x + p[2].x - p[3].x
  const dy1 = p[1].y - p[2].y,
    dy2 = p[3].y - p[2].y,
    dy3 = p[0].y - p[1].y + p[2].y - p[3].y
  const determinant = dx1 * dy2 - dx2 * dy1
  const g = Math.abs(dx3) + Math.abs(dy3) < 1e-9 ? 0 : (dx3 * dy2 - dx2 * dy3) / determinant
  const h = Math.abs(dx3) + Math.abs(dy3) < 1e-9 ? 0 : (dx1 * dy3 - dx3 * dy1) / determinant
  const a = p[1].x - p[0].x + g * p[1].x,
    b = p[3].x - p[0].x + h * p[3].x
  const d = p[1].y - p[0].y + g * p[1].y,
    e = p[3].y - p[0].y + h * p[3].y
  if (![a, b, d, e, g, h].every(Number.isFinite)) throw new Error('raster-edit-region')
  const distance = (i: number, j: number) => Math.hypot(p[i].x - p[j].x, p[i].y - p[j].y)
  const size = rasterSize(
    Math.max(1, Math.round(Math.max(distance(0, 1), distance(3, 2)))),
    Math.max(1, Math.round(Math.max(distance(0, 3), distance(1, 2))))
  )
  return {
    ...size,
    at: (u: number, v: number) => {
      const denominator = g * u + h * v + 1
      return {
        x: (a * u + b * v + p[0].x) / denominator - 0.5,
        y: (d * u + e * v + p[0].y) / denominator - 0.5
      }
    }
  }
}

function sample(source: Raster, x: number, y: number, destination: Uint8Array, offset: number, whiteOutside = false) {
  if (whiteOutside && (x < -0.5 || y < -0.5 || x > source.width - 0.5 || y > source.height - 0.5)) {
    destination.set([255, 255, 255, 255], offset)
    return
  }
  x = Math.max(0, Math.min(source.width - 1, x))
  y = Math.max(0, Math.min(source.height - 1, y))
  const left = Math.floor(x),
    top = Math.floor(y),
    fx = x - left,
    fy = y - top
  const positions = [
    [left, top, (1 - fx) * (1 - fy)],
    [Math.min(left + 1, source.width - 1), top, fx * (1 - fy)],
    [left, Math.min(top + 1, source.height - 1), (1 - fx) * fy],
    [Math.min(left + 1, source.width - 1), Math.min(top + 1, source.height - 1), fx * fy]
  ]
  let alpha = 0
  const rgb = [0, 0, 0]
  for (const [px, py, weight] of positions) {
    const i = (py * source.width + px) * 4,
      weighted = (weight * source.rgba[i + 3]) / 255
    alpha += weighted
    for (let channel = 0; channel < 3; channel++) rgb[channel] += source.rgba[i + channel] * weighted
  }
  for (let channel = 0; channel < 3; channel++)
    destination[offset + channel] = alpha ? Math.round(rgb[channel] / alpha) : 0
  destination[offset + 3] = Math.round(alpha * 255)
}

async function checkpoint(row: number, signal?: InstanceType<typeof globalThis.AbortSignal>) {
  if (signal?.aborted) throw new Error('workbench.feature.pdf.tools.job.cancel')
  // Yield to the UI/cancellation loop during a bounded large transform.
  if (row % 64 === 0) await new Promise<void>((resolve) => globalThis.setTimeout(resolve, 0))
  if (signal?.aborted) throw new Error('workbench.feature.pdf.tools.job.cancel')
}

export async function applyRasterEdit(
  source: Raster,
  edit: RasterEdit,
  signal?: InstanceType<typeof globalThis.AbortSignal>
): Promise<Raster> {
  if (signal?.aborted) throw new Error('workbench.feature.pdf.tools.job.cancel')
  validateRasterEdit(edit)
  rasterSize(source.width, source.height)
  if (source.rgba.length !== source.width * source.height * 4) throw new Error('raster-edit-size')
  const transform = projection(edit.corners, source.width, source.height)
  const angle = (edit.straighten * Math.PI) / 180,
    cosine = Math.cos(angle),
    sine = Math.sin(angle)
  const straightSize = edit.straighten
    ? rasterSize(
        Math.max(1, Math.ceil(Math.abs(transform.width * cosine) + Math.abs(transform.height * sine) - 1e-9)),
        Math.max(1, Math.ceil(Math.abs(transform.width * sine) + Math.abs(transform.height * cosine) - 1e-9))
      )
    : undefined
  const full = edit.corners.every(
    (point, index) => point.x === ORIGINAL_CORNERS[index].x && point.y === ORIGINAL_CORNERS[index].y
  )
  let raster: Raster = full
    ? { ...source, rgba: source.rgba.slice() }
    : {
        width: transform.width,
        height: transform.height,
        rgba: new Uint8Array(transform.width * transform.height * 4)
      }
  await checkpoint(0, signal)
  for (let y = 0; !full && y < raster.height; y++) {
    await checkpoint(y, signal)
    for (let x = 0; x < raster.width; x++) {
      const point = transform.at((x + 0.5) / raster.width, (y + 0.5) / raster.height)
      sample(source, point.x, point.y, raster.rgba, (y * raster.width + x) * 4)
    }
  }
  if (straightSize) {
    const size = straightSize
    const turned: Raster = { ...size, rgba: new Uint8Array(size.width * size.height * 4) }
    for (let y = 0; y < size.height; y++) {
      await checkpoint(y, signal)
      for (let x = 0; x < size.width; x++) {
        const dx = x + 0.5 - size.width / 2,
          dy = y + 0.5 - size.height / 2
        sample(
          raster,
          cosine * dx + sine * dy + raster.width / 2 - 0.5,
          -sine * dx + cosine * dy + raster.height / 2 - 0.5,
          turned.rgba,
          (y * size.width + x) * 4,
          true
        )
      }
    }
    raster = turned
  }
  for (let turn = 0; turn < edit.quarterTurns; turn++) {
    await checkpoint(0, signal)
    raster = rotate90(raster)
  }
  return raster
}
