import { encodePng, type Raster } from '../pdfToolsPng'
import { parsePdfImageFormat, type PdfImageFormat } from './pdfExport'

export async function encodePdfRaster(raster: Raster, format: PdfImageFormat): Promise<Uint8Array> {
  if (parsePdfImageFormat(format) === 'png') return encodePng(raster)
  if (await nodeFilesAvailable()) {
    const { default: sharp } = await import(/* @vite-ignore */ 'sharp')
    const bytes = await sharp(raster.rgba, {
      raw: { width: raster.width, height: raster.height, channels: 4 }
    })
      .flatten({ background: '#ffffff' })
      .jpeg({ quality: 90 })
      .toBuffer()
    return new Uint8Array(bytes)
  }
  const canvas = globalThis.document.createElement('canvas')
  canvas.width = raster.width
  canvas.height = raster.height
  const context = canvas.getContext('2d')
  if (!context) throw new Error('pdf-image-format')
  const pixels = new Uint8ClampedArray(raster.rgba)
  for (let index = 0; index < pixels.length; index += 4) {
    const alpha = pixels[index + 3] / 255
    for (let channel = 0; channel < 3; channel++)
      pixels[index + channel] = Math.round(pixels[index + channel] * alpha + 255 * (1 - alpha))
    pixels[index + 3] = 255
  }
  context.putImageData(new globalThis.ImageData(pixels, raster.width, raster.height), 0, 0)
  const blob = await new Promise<InstanceType<typeof globalThis.Blob> | null>((resolve) =>
    canvas.toBlob(resolve, 'image/jpeg', 0.9)
  )
  if (!blob || blob.type !== 'image/jpeg') throw new Error('pdf-image-format')
  return new Uint8Array(await blob.arrayBuffer())
}

async function nodeFilesAvailable(): Promise<boolean> {
  try {
    const fs = await import(/* @vite-ignore */ 'node:fs/promises')
    return typeof fs.readFile === 'function'
  } catch {
    return false
  }
}
