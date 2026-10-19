import { PDFDocument, rgb } from 'pdf-lib'
import { describe, expect, it } from 'vitest'

import { decodePng, solid } from '../pdfToolsPng'
import { runPdfToolJob } from '../pdfToolsRun'
import {
  applyFilterEdit,
  filterComparisonAsStr,
  filterKindAsStr,
  filterThumbnail,
  ORIGINAL_FILTER,
  ORIGINAL_FILTER_DOCUMENT,
  parseFilterComparison,
  parseFilterKind,
  validateFilterDocument,
  validateFilterEdit
} from './rasterFilter'

describe('original-based filters', () => {
  it('preserves original hidden RGB/alpha and has exact independently calculated intensity endpoints', async () => {
    const source = solid(2, 1, [10, 80, 160, 255])
    source.rgba.set([200, 30, 20, 0], 4)
    expect((await applyFilterEdit(source, ORIGINAL_FILTER)).rgba).toEqual(source.rgba)
    expect((await applyFilterEdit(source, { name: 'document', intensity: 0 })).rgba).toEqual(source.rgba)
    const half = await applyFilterEdit(source, { name: 'grayscale', intensity: 50 })
    expect([...half.rgba.slice(0, 4)]).toEqual([41, 76, 116, 255])
    expect(half.rgba[7]).toBe(0)
    expect([...(await applyFilterEdit(source, { name: 'grayscale', intensity: 100 })).rgba.slice(0, 4)]).toEqual([
      71, 71, 71, 255
    ])
    expect([...source.rgba.slice(0, 4)]).toEqual([10, 80, 160, 255])
  })
  it('refuses malformed closed sets, intensity and one-based pages before any source output', async () => {
    expect(filterKindAsStr(parseFilterKind('original'))).toBe('original')
    expect(filterComparisonAsStr(parseFilterComparison('split'))).toBe('split')
    expect(() => parseFilterKind('sepia')).toThrow('raster filter kind')
    expect(() => parseFilterComparison('slider')).toThrow('comparison mode')
    for (const intensity of [NaN, Infinity, -1, 101, 2.5])
      expect(() => validateFilterEdit({ name: 'grayscale', intensity })).toThrow('intensity')
    for (const page of ['0', '-1', '1.5', '01', '3'])
      expect(() => validateFilterDocument({ default: ORIGINAL_FILTER, pages: { [page]: ORIGINAL_FILTER } }, 2)).toThrow(
        'pages'
      )
    await expect(
      applyFilterEdit({ width: 10000, height: 10000, rgba: new Uint8Array() }, ORIGINAL_FILTER)
    ).rejects.toThrow('size')
    const controller = new globalThis.AbortController()
    controller.abort()
    await expect(applyFilterEdit(solid(1, 1, [10, 20, 30, 255]), ORIGINAL_FILTER, controller.signal)).rejects.toThrow(
      'cancel'
    )
  })
  it('cooperates with an actual cancellation during a transform and bounds source thumbnails', async () => {
    const controller = new globalThis.AbortController()
    const source = solid(512, 512, [10, 80, 160, 255])
    const promise = applyFilterEdit(source, { name: 'contrast', intensity: 100 }, controller.signal)
    globalThis.setTimeout(() => controller.abort(), 0)
    await expect(promise).rejects.toThrow('cancel')
    const thumbnail = filterThumbnail(source)
    expect([thumbnail.width, thumbnail.height]).toEqual([128, 128])
    expect([...thumbnail.rgba.slice(0, 4)]).toEqual([10, 80, 160, 255])
  })
  it('filters only the addressed real PDF page and exports every original page in order', async () => {
    const pdf = await PDFDocument.create()
    for (const color of [rgb(1, 0, 0), rgb(0, 1, 0)]) {
      pdf.addPage([20, 12]).drawRectangle({ x: 0, y: 0, width: 20, height: 12, color })
    }
    const source = { name: 'colors.pdf', type: 'application/pdf', bytes: new Uint8Array(await pdf.save()) }
    const result = await runPdfToolJob({
      kind: 'filter',
      files: [source],
      filterDocument: { ...ORIGINAL_FILTER_DOCUMENT, pages: { 2: { name: 'grayscale', intensity: 50 } } }
    })
    if (result.state !== 'preview') throw new Error('Missing actual PDF filter result')
    expect(result.files.map((file) => [file.name, file.sourcePage])).toEqual([
      ['colors-page-1.png', 1],
      ['colors-page-2.png', 2]
    ])
    expect([...(await decodePng(result.files[0].bytes)).rgba.slice(0, 4)]).toEqual([255, 0, 0, 255])
    expect([...(await decodePng(result.files[1].bytes)).rgba.slice(0, 4)]).toEqual([91, 219, 91, 255])
    expect([...(await decodePng(result.sourceFiles?.[1].bytes ?? new Uint8Array())).rgba.slice(0, 4)]).toEqual([
      0, 255, 0, 255
    ])
    expect(result.files[1].bytes).toBe(result.confirmation.files[1].bytes)
    const refused = await runPdfToolJob({
      kind: 'filter',
      files: [source],
      filterDocument: { ...ORIGINAL_FILTER_DOCUMENT, pages: { 3: ORIGINAL_FILTER } }
    })
    expect(refused).toMatchObject({ state: 'failed', reason: 'workbench.feature.pdf.tools.workspace.filters.failed' })
  })
})
