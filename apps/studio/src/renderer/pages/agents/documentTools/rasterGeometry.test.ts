import { PDFDocument } from 'pdf-lib'
import { describe, expect, it } from 'vitest'

import { decodePng, encodePng, solid } from '../pdfToolsPng'
import { runPdfToolJob } from '../pdfToolsRun'
import {
  applyRasterEdit,
  cropModeAsStr,
  cropRatioAsStr,
  ORIGINAL_RASTER_EDIT,
  parseCropMode,
  parseCropRatio,
  rectangleCorners,
  validateRasterEdit
} from './rasterGeometry'

function gradient(width: number, height: number) {
  const raster = solid(width, height, [0, 0, 0, 255])
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++) raster.rgba.set([x * 20, y * 40, 10, 255], (y * width + x) * 4)
  return raster
}

describe('original-source geometry', () => {
  it('preserves original dimensions, opaque pixels and hidden transparent RGB at identity', async () => {
    const raster = gradient(4, 3)
    raster.rgba.set([70, 20, 80, 0], 0)
    const result = await applyRasterEdit(raster, ORIGINAL_RASTER_EDIT)
    expect(result.width).toBe(4)
    expect(result.height).toBe(3)
    expect(result.rgba).toEqual(raster.rgba)
    expect(result.rgba).not.toBe(raster.rgba)
  })
  it('crops exact original pixel edges before turning right or left', async () => {
    const source = gradient(4, 3)
    const result = await applyRasterEdit(source, {
      ...ORIGINAL_RASTER_EDIT,
      corners: rectangleCorners(0.25, 0, 0.5, 2 / 3),
      quarterTurns: 1
    })
    expect([result.width, result.height]).toEqual([2, 2])
    expect([...result.rgba]).toEqual([20, 40, 10, 255, 20, 0, 10, 255, 40, 40, 10, 255, 40, 0, 10, 255])
    const left = await applyRasterEdit(source, { ...ORIGINAL_RASTER_EDIT, quarterTurns: 3 })
    expect([left.width, left.height]).toEqual([3, 4])
    expect([...left.rgba.subarray(0, 4)]).toEqual([60, 0, 10, 255])
    expect(source.rgba).toEqual(gradient(4, 3).rgba)
  })
  it('corrects a trapezoid with a projective vanishing point rather than bilinear corner blending', async () => {
    const result = await applyRasterEdit(gradient(8, 6), {
      ...ORIGINAL_RASTER_EDIT,
      corners: [
        { x: 0.25, y: 0 },
        { x: 0.75, y: 0 },
        { x: 1, y: 1 },
        { x: 0, y: 1 }
      ]
    })
    expect([result.width, result.height]).toEqual([8, 6])
    // For this symmetric trapezoid, source y = 3v/(1-v/2). At output row 2,
    // v=5/12 maps to pixel-center y=30/19-1/2; the independent green gradient is 40y.
    expect(result.rgba[(2 * 8 + 4) * 4 + 1]).toBe(43)
    expect(result.rgba[(2 * 8 + 4) * 4 + 3]).toBe(255)
  })
  it('expands straightened bounds without clipping and labels the white margin pixels', async () => {
    const result = await applyRasterEdit(solid(4, 2, [10, 20, 30, 255]), { ...ORIGINAL_RASTER_EDIT, straighten: 45 })
    expect([result.width, result.height]).toEqual([5, 5])
    expect([...result.rgba.subarray(0, 4)]).toEqual([255, 255, 255, 255])
    const center = (2 * 5 + 2) * 4
    expect([...result.rgba.subarray(center, center + 4)]).toEqual([10, 20, 30, 255])
  })
  it('refuses crossed/degenerate/out-of-source regions, invalid sets and enlarged allocations', async () => {
    for (const corners of [
      rectangleCorners(-0.1, 0, 0.5, 0.5),
      rectangleCorners(0, 0, 0, 1),
      [
        { x: 0, y: 0 },
        { x: 1, y: 1 },
        { x: 1, y: 0 },
        { x: 0, y: 1 }
      ]
    ]) {
      expect(() =>
        validateRasterEdit({ ...ORIGINAL_RASTER_EDIT, corners: corners as typeof ORIGINAL_RASTER_EDIT.corners })
      ).toThrow('raster-edit-region')
    }
    expect(() => validateRasterEdit({ ...ORIGINAL_RASTER_EDIT, quarterTurns: 4 })).toThrow('raster-edit-region')
    expect(() => validateRasterEdit({ ...ORIGINAL_RASTER_EDIT, straighten: 46 })).toThrow('raster-edit-region')
    expect(() => parseCropMode('automatic')).toThrow('raster crop mode')
    expect(() => parseCropRatio('2:3')).toThrow('raster crop ratio')
    expect(cropModeAsStr(parseCropMode('document'))).toBe('document')
    expect(cropRatioAsStr(parseCropRatio('4:3'))).toBe('4:3')
    await expect(
      applyRasterEdit({ width: 32768, height: 32768, rgba: new Uint8Array() }, ORIGINAL_RASTER_EDIT)
    ).rejects.toThrow('raster-edit-size')
    // The original is within the pixel budget, but its rotated bounding box is not.
    await expect(
      applyRasterEdit(solid(4096, 4096, [1, 2, 3, 255]), { ...ORIGINAL_RASTER_EDIT, straighten: 45 })
    ).rejects.toThrow('raster-edit-size')
  })
  it('settles an actual cancellation during a large transform and produces no late output', async () => {
    const controller = new globalThis.AbortController()
    const work = applyRasterEdit(
      solid(256, 256, [1, 2, 3, 255]),
      { ...ORIGINAL_RASTER_EDIT, corners: rectangleCorners(0.1, 0.1, 0.8, 0.8) },
      controller.signal
    )
    globalThis.setTimeout(() => controller.abort(), 0)
    await expect(work).rejects.toThrow('workbench.feature.pdf.tools.job.cancel')
  })
  it('exports the same actual source crop while retaining an independent original preview', async () => {
    const png = await encodePng(gradient(4, 3))
    const result = await runPdfToolJob({
      kind: 'rotate',
      files: [{ name: 'source.png', type: 'image/png', bytes: png }],
      autoCrop: true,
      rasterEdit: { ...ORIGINAL_RASTER_EDIT, corners: rectangleCorners(0.25, 0, 0.5, 2 / 3), quarterTurns: 1 }
    })
    if (result.state !== 'preview') throw new Error('Expected geometry preview')
    const cropped = await decodePng(result.confirmation.files[0].bytes)
    expect([cropped.width, cropped.height]).toEqual([2, 2])
    expect(cropped.rgba).toEqual(
      (
        await applyRasterEdit(gradient(4, 3), {
          ...ORIGINAL_RASTER_EDIT,
          corners: rectangleCorners(0.25, 0, 0.5, 2 / 3),
          quarterTurns: 1
        })
      ).rgba
    )
    expect((await decodePng(result.sourceFiles?.[0].bytes ?? new Uint8Array())).rgba).toEqual(gradient(4, 3).rgba)
    expect(result.confirmation.files[0].bytes).toBe(result.files[0].bytes)
  })
  it('keeps PDF page dimensions/content while applying only supported whole-document quarter turns', async () => {
    const pdf = await PDFDocument.create()
    pdf.addPage([100, 60])
    pdf.addPage([120, 80])
    const file = { name: 'two.pdf', type: 'application/pdf', bytes: await pdf.save() }
    const result = await runPdfToolJob({
      kind: 'rotate',
      files: [file],
      rasterEdit: { ...ORIGINAL_RASTER_EDIT, quarterTurns: 3 }
    })
    if (result.state !== 'preview') throw new Error('Expected PDF preview')
    const reopened = await PDFDocument.load(result.files[0].bytes)
    expect(reopened.getPages().map((p) => [p.getWidth(), p.getHeight(), p.getRotation().angle])).toEqual([
      [100, 60, 270],
      [120, 80, 270]
    ])
    const refused = await runPdfToolJob({
      kind: 'rotate',
      files: [file],
      rasterEdit: { ...ORIGINAL_RASTER_EDIT, straighten: 1 }
    })
    expect(refused).toMatchObject({
      state: 'failed',
      reason: 'workbench.feature.pdf.tools.workspace.geometry.pdf_unsupported'
    })
  })
})
