import { PDFDocument, rgb } from 'pdf-lib'
import sharp from 'sharp'
import { describe, expect, it } from 'vitest'

import { pdfiumPages } from '../pdfToolsPdfium'
import { decodePng } from '../pdfToolsPng'
import { runPdfToolJob } from '../pdfToolsRun'
import {
  parsePdfExportDpi,
  parsePdfImageFormat,
  pdfExportDpiAsStr,
  pdfImageFormatAsStr,
  pdfRasterSize,
  selectedPdfPages
} from './pdfExport'

async function documentBytes() {
  const pdf = await PDFDocument.create()
  for (const color of [rgb(1, 0, 0), rgb(0, 1, 0), rgb(0, 0, 1)]) {
    const page = pdf.addPage([100, 60])
    page.drawRectangle({ x: 0, y: 0, width: 100, height: 60, color })
  }
  return new Uint8Array(await pdf.save())
}

describe('PDF image export', () => {
  it('uses bounded one-based ranges, preserves explicit order and refuses unknown options', () => {
    expect(selectedPdfPages('', 3)).toEqual([0, 1, 2])
    expect(selectedPdfPages('3, 1-2, 3', 3)).toEqual([2, 0, 1])
    for (const range of ['0', '4', '-1', '2-1', '1,', '1.5', '1-99', '99999999999999999', '1;2']) {
      expect(() => selectedPdfPages(range, 3)).toThrow('pdf-page-range')
    }
    expect(() => selectedPdfPages('1'.repeat(1025), 3)).toThrow('pdf-page-range')
    expect(pdfExportDpiAsStr(parsePdfExportDpi(288))).toBe('288')
    expect(pdfImageFormatAsStr(parsePdfImageFormat('jpeg'))).toBe('jpeg')
    expect(() => parsePdfImageFormat('webp')).toThrow('PDF image format')
    expect(() => parsePdfExportDpi('144')).toThrow('PDF export resolution')
    expect(() => parsePdfExportDpi(145)).toThrow('PDF export resolution')
    expect(pdfRasterSize(100, 60, 288)).toEqual({ width: 400, height: 240 })
    expect(() => pdfRasterSize(10_000, 10_000, 144)).toThrow('pdf-raster-limit')
    expect(() => pdfRasterSize(NaN, 60, 144)).toThrow('pdf-raster-limit')
  })

  it('renders only requested source pages with real PDFium and names/encodes their exact pixels', async () => {
    const bytes = await documentBytes()
    const result = await runPdfToolJob({
      kind: 'pdf-to-image',
      files: [{ name: 'colors.pdf', type: 'application/pdf', bytes }],
      pdfExport: { pageRange: '3,1', dpi: 288, imageFormat: 'png' }
    })
    if (result.state !== 'done') throw new Error(`Unexpected result: ${JSON.stringify(result)}`)
    expect(result.files.map((file) => [file.name, file.sourcePage, file.width, file.height])).toEqual([
      ['colors-page-3.png', 3, 400, 240],
      ['colors-page-1.png', 1, 400, 240]
    ])
    expect([...(await decodePng(result.files[0].bytes)).rgba.slice(0, 4)]).toEqual([0, 0, 255, 255])
    expect([...(await decodePng(result.files[1].bytes)).rgba.slice(0, 4)]).toEqual([255, 0, 0, 255])
    const rendered = await pdfiumPages(bytes, 72, { pageRange: '2' })
    expect(rendered.indices).toEqual([1])
    expect(rendered.pages[0].width).toBe(100)
    expect([...rendered.pages[0].rgba.slice(0, 4)]).toEqual([0, 255, 0, 255])
  })

  it('produces real JPEG bytes and keeps selected page order in long output', async () => {
    const bytes = await documentBytes()
    const jpeg = await runPdfToolJob({
      kind: 'pdf-to-image',
      files: [{ name: 'colors.pdf', type: 'application/pdf', bytes }],
      pdfExport: { pageRange: '2', dpi: 72, imageFormat: 'jpeg' }
    })
    if (jpeg.state !== 'done') throw new Error('Missing JPEG result')
    expect(jpeg.files[0].name).toBe('colors-page-2.jpg')
    expect(jpeg.files[0].mediaType).toBe('image/jpeg')
    const image = sharp(jpeg.files[0].bytes)
    expect((await image.metadata()).width).toBe(100)
    const pixel = [...(await image.raw().toBuffer()).slice(0, 3)]
    expect(pixel[1]).toBeGreaterThan(250)
    expect(pixel[0]).toBeLessThan(5)
    expect(pixel[2]).toBeLessThan(5)
    const long = await runPdfToolJob({
      kind: 'pdf-to-long',
      files: [{ name: 'colors.pdf', type: 'application/pdf', bytes }],
      pdfExport: { pageRange: '3,1', dpi: 72, imageFormat: 'png' }
    })
    if (long.state !== 'done') throw new Error('Missing long result')
    const raster = await decodePng(long.files[0].bytes)
    expect([raster.width, raster.height]).toEqual([100, 120])
    expect([...raster.rgba.slice(0, 4)]).toEqual([0, 0, 255, 255])
    expect([...raster.rgba.slice(100 * 60 * 4, 100 * 60 * 4 + 4)]).toEqual([255, 0, 0, 255])
  })

  it('refuses out-of-bounds pages and huge bitmap allocations without poisoning the engine', async () => {
    const bytes = await documentBytes()
    const invalid = await runPdfToolJob({
      kind: 'pdf-to-image',
      files: [{ name: 'colors.pdf', type: 'application/pdf', bytes }],
      pdfExport: { pageRange: '4', dpi: 144, imageFormat: 'png' }
    })
    expect(invalid).toMatchObject({
      state: 'failed',
      reason: 'workbench.feature.pdf.tools.workspace.pdf.range_error'
    })
    const huge = await PDFDocument.create()
    huge.addPage([10_000, 10_000])
    await expect(pdfiumPages(new Uint8Array(await huge.save()))).rejects.toThrow('pdf-raster-limit')
    const controller = new globalThis.AbortController()
    controller.abort()
    await expect(pdfiumPages(bytes, 144, { signal: controller.signal })).rejects.toThrow('abort')
    expect((await pdfiumPages(bytes, 72, { pageRange: '1' })).pages).toHaveLength(1)
  })
})
