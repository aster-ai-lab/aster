import { Buffer } from 'node:buffer'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import process from 'node:process'

import { PDFDocument, rgb } from 'pdf-lib'
import sharp from 'sharp'
import { describe, expect, it } from 'vitest'

import { imagesToPdf } from '../pdfToolsPdf'
import { pdfiumPages } from '../pdfToolsPdfium'
import { decodePng, encodePng, solid, type Raster } from '../pdfToolsPng'
import { runPdfToolJob } from '../pdfToolsRun'
import { DEFAULT_IMAGE_PDF, validateImagePdf } from './imagePdf'
import { composeLongImage, DEFAULT_LONG_IMAGE, longImagePlan, type LongImageOptions } from './longImage'

async function retain(name: string, bytes: Uint8Array) {
  const root = process.env.A3S_CLOUD_DOCUMENT_TOOLS_EVIDENCE_DIR
  if (!root) return
  const dir = join(root, 'assembly-artifacts')
  await mkdir(dir, { recursive: true })
  await writeFile(join(dir, name), bytes)
}
async function compose(sources: Raster[], options: LongImageOptions = DEFAULT_LONG_IMAGE) {
  const outputs: { bytes: Uint8Array; width: number; height: number }[] = []
  for await (const item of composeLongImage(sources, options)) outputs.push(item)
  return outputs
}
function pixel(raster: Raster, x: number, y: number) {
  return [...raster.rgba.slice((y * raster.width + x) * 4, (y * raster.width + x + 1) * 4)]
}

describe('image assembly exports', () => {
  it('keeps narrow inputs at original size, places them accurately and composites alpha over white', async () => {
    const sources = [solid(4, 2, [255, 0, 0, 255]), solid(2, 2, [0, 0, 255, 128])]
    const [output] = await compose(sources, { ...DEFAULT_LONG_IMAGE, gap: 1 })
    const image = await decodePng(output.bytes)
    expect([image.width, image.height]).toEqual([4, 5])
    expect(pixel(image, 0, 2)).toEqual([255, 255, 255, 255])
    expect(pixel(image, 0, 3)).toEqual([255, 255, 255, 255])
    expect(pixel(image, 1, 3)).toEqual([127, 127, 255, 255])
    expect(pixel(image, 2, 4)).toEqual([127, 127, 255, 255])
    const [transparent] = await compose(sources, {
      ...DEFAULT_LONG_IMAGE,
      alignment: 'right',
      background: 'transparent'
    })
    const alpha = await decodePng(transparent.bytes)
    expect(pixel(alpha, 0, 2)).toEqual([0, 0, 0, 0])
    expect(pixel(alpha, 2, 2)).toEqual([0, 0, 255, 128])
    await retain('center-gap-white.png', output.bytes)
    await retain('right-transparent.png', transparent.bytes)
  })

  it('preserves every resampled row across segment seams, including a gap and a source boundary', async () => {
    const gradient = solid(3, 7, [0, 0, 0, 255])
    for (let y = 0; y < 7; y++) for (let x = 0; x < 3; x++) gradient.rgba[(y * 3 + x) * 4] = y * 30
    const sources = [gradient, solid(2, 2, [0, 80, 160, 255])]
    const options = { ...DEFAULT_LONG_IMAGE, width: 5, upscale: true, gap: 2 }
    const [whole] = await compose(sources, options)
    const segments = await compose(sources, { ...options, segmentHeight: 4 })
    const rasters = await Promise.all(segments.map((segment) => decodePng(segment.bytes)))
    const pixels = Buffer.concat(rasters.map((raster) => Buffer.from(raster.rgba)))
    expect(rasters.map((raster) => raster.height)).toEqual([4, 4, 4, 4, 3])
    expect(pixels).toEqual(Buffer.from((await decodePng(whole.bytes)).rgba))
    const reconstructed = await sharp(pixels, { raw: { width: 5, height: 19, channels: 4 } })
      .png()
      .toBuffer()
    expect((await sharp(reconstructed).metadata()).height).toBe(19)
    for (const [index, segment] of segments.entries()) await retain(`seam-${index + 1}.png`, segment.bytes)
    await retain('seam-whole.png', whole.bytes)
  })

  it('refuses unsafe allocations and unknown options before rendering, while allowing bounded segmentation', () => {
    expect(() => longImagePlan([{ width: 8192, height: 8192 }])).toThrow('long-image-limit')
    expect(() => longImagePlan([{ width: 1, height: 40000 }])).toThrow('too-long')
    expect(
      longImagePlan([{ width: 1, height: 40000 }], { ...DEFAULT_LONG_IMAGE, segmentHeight: 20000 }).segments
    ).toHaveLength(2)
    expect(() => longImagePlan([{ width: 2, height: 2 }], { ...DEFAULT_LONG_IMAGE, width: 0 })).toThrow(
      'long-image-options'
    )
    expect(() =>
      longImagePlan([{ width: 2, height: 2 }], {
        ...DEFAULT_LONG_IMAGE,
        alignment: 'justify'
      } as unknown as LongImageOptions)
    ).toThrow('alignment')
    expect(() => validateImagePdf({ ...DEFAULT_IMAGE_PDF, margin: Number.NaN })).toThrow('image-pdf-options')
    expect(() => validateImagePdf({ ...DEFAULT_IMAGE_PDF, paper: 'unknown' } as never)).toThrow('paper')
  })

  it('cancels between bounded segments without producing later files', async () => {
    const controller = new globalThis.AbortController()
    const generator = composeLongImage(
      [solid(4, 10, [10, 20, 30, 255])],
      { ...DEFAULT_LONG_IMAGE, segmentHeight: 3 },
      controller.signal
    )
    expect((await generator.next()).value?.height).toBe(3)
    controller.abort()
    await expect(generator.next()).rejects.toThrow('abort')
    const already = new globalThis.AbortController()
    already.abort()
    await expect(
      composeLongImage([solid(2, 2, [0, 0, 0, 255])], DEFAULT_LONG_IMAGE, already.signal).next()
    ).rejects.toThrow('abort')
  })

  it('refuses huge PNG headers, truncated streams and excess decompressed scanlines', async () => {
    const png = await encodePng(solid(1, 2, [10, 80, 160, 255]))
    const huge = png.slice()
    new DataView(huge.buffer).setUint32(16, 100000)
    new DataView(huge.buffer).setUint32(20, 100000)
    await expect(decodePng(huge)).rejects.toThrow('raster-limit')
    await expect(decodePng(png.slice(0, -5))).rejects.toThrow('png')
    const bomb = png.slice()
    new DataView(bomb.buffer).setUint32(20, 1)
    await expect(decodePng(bomb)).rejects.toThrow('raster-limit')
  })

  it('exports real Letter landscape pages with margins and preserves contain/cover clipping', async () => {
    const png = await encodePng(solid(4, 2, [200, 10, 30, 255]))
    for (const fit of ['contain', 'cover'] as const) {
      const bytes = await imagesToPdf([png], { paper: 'letter', orientation: 'landscape', fit, margin: 36 })
      const pdf = await PDFDocument.load(bytes)
      expect(pdf.getPage(0).getSize()).toEqual({ width: 792, height: 612 })
      const { pages, texts } = await pdfiumPages(bytes, 72)
      expect(texts.join('')).toBe('')
      expect(pixel(pages[0], 10, 306)).toEqual([255, 255, 255, 255])
      expect(pixel(pages[0], 396, 306)).toEqual([200, 10, 30, 255])
      expect(pixel(pages[0], 396, 45)).toEqual(fit === 'cover' ? [200, 10, 30, 255] : [255, 255, 255, 255])
      await retain(`letter-${fit}.pdf`, bytes)
    }
  })

  it('keeps actual page order and source dimensions in image-size PDF mode', async () => {
    const red = await encodePng(solid(4, 2, [240, 0, 0, 255]))
    const blue = await encodePng(solid(2, 4, [0, 0, 240, 255]))
    const bytes = await imagesToPdf([blue, red])
    const pdf = await PDFDocument.load(bytes)
    expect(pdf.getPages().map((page) => page.getSize())).toEqual([
      { width: 2, height: 4 },
      { width: 4, height: 2 }
    ])
    const { pages } = await pdfiumPages(bytes, 72)
    expect(pixel(pages[0], 0, 0)).toEqual([0, 0, 240, 255])
    expect(pixel(pages[1], 0, 0)).toEqual([240, 0, 0, 255])
    await retain('source-order.pdf', bytes)
  })

  it('uses selected PDF page order and shared long-image segmentation options', async () => {
    const source = await PDFDocument.create()
    for (const color of [rgb(1, 0, 0), rgb(0, 0, 1)])
      source.addPage([4, 2]).drawRectangle({ x: 0, y: 0, width: 4, height: 2, color })
    const result = await runPdfToolJob({
      kind: 'pdf-to-long',
      files: [{ name: 'pages.pdf', type: 'application/pdf', bytes: await source.save() }],
      pdfExport: { pageRange: '2,1', dpi: 72, imageFormat: 'png' },
      longImage: { ...DEFAULT_LONG_IMAGE, gap: 1, segmentHeight: 3 }
    })
    if (result.state !== 'done') throw new Error(JSON.stringify(result))
    expect(result.files.map((file) => file.name)).toEqual(['long-1.png', 'long-2.png'])
    const images = await Promise.all(result.files.map((file) => decodePng(file.bytes)))
    expect(images.map((image) => image.height)).toEqual([3, 2])
    expect(pixel(images[0], 0, 0)).toEqual([0, 0, 255, 255])
    expect(pixel(images[0], 0, 2)).toEqual([255, 255, 255, 255])
    expect(pixel(images[1], 0, 0)).toEqual([255, 0, 0, 255])
    await retain('pdf-long-source.pdf', await source.save())
    for (const [index, file] of result.files.entries()) await retain(`pdf-long-${index + 1}.png`, file.bytes)
  })

  it('accepts real JPEG inputs and emits correctly numbered segmented JPEG downloads', async () => {
    const source = await sharp({ create: { width: 8, height: 8, channels: 3, background: '#1050a0' } })
      .jpeg()
      .toBuffer()
    const result = await runPdfToolJob({
      kind: 'merge',
      autoCrop: false,
      files: [{ name: 'input.jpg', type: 'image/jpeg', bytes: new Uint8Array(source) }],
      longImage: { ...DEFAULT_LONG_IMAGE, segmentHeight: 3, imageFormat: 'jpeg' }
    })
    expect(result.state).toBe('done')
    if (result.state !== 'done') throw new Error(JSON.stringify(result))
    expect(result.files.map((file) => file.name)).toEqual(['long-1.jpg', 'long-2.jpg', 'long-3.jpg'])
    for (const [index, file] of result.files.entries()) {
      const metadata = await sharp(Buffer.from(file.bytes)).metadata()
      expect([metadata.format, metadata.width, metadata.height]).toEqual(['jpeg', 8, index === 2 ? 2 : 3])
      await retain(file.name, file.bytes)
    }
  })
})
