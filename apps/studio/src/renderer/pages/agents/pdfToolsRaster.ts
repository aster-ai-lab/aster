import { longImageLayout } from './pdfToolsJob'
import { decodePng, encodePng, type Raster } from './pdfToolsPng'

export const PDF_RASTER_DPI = 144
export const OFFICE_CAPTURE_DPI = 96
export const IMAGE_PDF_DPI = 72

export type FilterName = 'document' | 'grayscale' | 'contrast'

export function rotate90(raster: Raster): Raster {
  const width = raster.height
  const height = raster.width
  const rgba = new Uint8Array(width * height * 4)
  for (let y = 0; y < raster.height; y++) {
    for (let x = 0; x < raster.width; x++) {
      const nextX = raster.height - 1 - y
      const nextY = x
      copyPixel(raster.rgba, (y * raster.width + x) * 4, rgba, (nextY * width + nextX) * 4)
    }
  }
  return { width, height, rgba }
}

export function crop(raster: Raster, rect: { x: number; y: number; width: number; height: number }): Raster {
  const x0 = clamp(Math.round(rect.x), 0, raster.width)
  const y0 = clamp(Math.round(rect.y), 0, raster.height)
  const width = clamp(Math.round(rect.width), 1, raster.width - x0)
  const height = clamp(Math.round(rect.height), 1, raster.height - y0)
  const rgba = new Uint8Array(width * height * 4)
  for (let y = 0; y < height; y++) {
    const from = ((y0 + y) * raster.width + x0) * 4
    rgba.set(raster.rgba.subarray(from, from + width * 4), y * width * 4)
  }
  return { width, height, rgba }
}

export function applyFilter(raster: Raster, name: FilterName): Raster {
  const rgba = new Uint8Array(raster.rgba)
  for (let index = 0; index < rgba.length; index += 4) {
    if (name === 'grayscale') writeGray(rgba, index, luminance(rgba[index], rgba[index + 1], rgba[index + 2]))
    else if (name === 'contrast') {
      rgba[index] = contrast(rgba[index])
      rgba[index + 1] = contrast(rgba[index + 1])
      rgba[index + 2] = contrast(rgba[index + 2])
    } else {
      let value = contrast(luminance(rgba[index], rgba[index + 1], rgba[index + 2]))
      if (value > 200) value = Math.min(255, Math.round(value + (255 - value) * 0.55))
      writeGray(rgba, index, value)
    }
  }
  return { width: raster.width, height: raster.height, rgba }
}

/** Drop a near-white margin. A page with no margin, or no ink, keeps every pixel. */
export function autoCrop(raster: Raster): { raster: Raster; kept: boolean } {
  let minX = raster.width
  let minY = raster.height
  let maxX = -1
  let maxY = -1
  for (let y = 0; y < raster.height; y++) {
    for (let x = 0; x < raster.width; x++) {
      const index = (y * raster.width + x) * 4
      if (raster.rgba[index] >= 245 && raster.rgba[index + 1] >= 245 && raster.rgba[index + 2] >= 245) continue
      if (x < minX) minX = x
      if (y < minY) minY = y
      if (x > maxX) maxX = x
      if (y > maxY) maxY = y
    }
  }
  if (maxX < 0 || (minX === 0 && minY === 0 && maxX === raster.width - 1 && maxY === raster.height - 1)) {
    return { raster: { width: raster.width, height: raster.height, rgba: raster.rgba.slice() }, kept: true }
  }
  return {
    raster: crop(raster, { x: minX, y: minY, width: maxX - minX + 1, height: maxY - minY + 1 }),
    kept: false
  }
}

export async function stackLong(rasters: readonly Raster[]): Promise<Uint8Array> {
  const layout = longImageLayout(rasters.map((raster) => ({ width: raster.width, height: raster.height })))
  if ('error' in layout) throw new Error(layout.error)
  const rgba = new Uint8Array(layout.width * layout.height * 4)
  rgba.fill(255)
  let top = 0
  for (const [index, raster] of rasters.entries()) {
    const height = layout.rows[index]?.height ?? 0
    blit(rgba, layout.width, raster, 0, top, layout.width, height)
    top += height
  }
  return encodePng({ width: layout.width, height: layout.height, rgba })
}

export async function rasterFromPng(bytes: Uint8Array): Promise<Raster> {
  return decodePng(bytes)
}

function blit(
  dest: Uint8Array,
  destWidth: number,
  source: Raster,
  left: number,
  top: number,
  width: number,
  height: number
) {
  for (let y = 0; y < height; y++) {
    const sourceY = Math.min(source.height - 1, Math.floor((y * source.height) / height))
    for (let x = 0; x < width; x++) {
      const sourceX = Math.min(source.width - 1, Math.floor((x * source.width) / width))
      copyPixel(source.rgba, (sourceY * source.width + sourceX) * 4, dest, ((top + y) * destWidth + left + x) * 4)
    }
  }
}

function copyPixel(from: Uint8Array, fromIndex: number, to: Uint8Array, toIndex: number) {
  to[toIndex] = from[fromIndex]
  to[toIndex + 1] = from[fromIndex + 1]
  to[toIndex + 2] = from[fromIndex + 2]
  to[toIndex + 3] = from[fromIndex + 3]
}

function luminance(red: number, green: number, blue: number): number {
  return Math.round(red * 0.2126 + green * 0.7152 + blue * 0.0722)
}

function contrast(value: number): number {
  return clamp(Math.round((value - 128) * 1.4 + 128), 0, 255)
}

function writeGray(rgba: Uint8Array, index: number, value: number) {
  rgba[index] = value
  rgba[index + 1] = value
  rgba[index + 2] = value
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value))
}
