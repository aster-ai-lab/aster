import { Buffer } from 'node:buffer'

import sharp from 'sharp'
import { describe, expect, it } from 'vitest'

import { decodePng } from '../pdfToolsPng'
import { officeSvgPages } from './officeSvg'

const sheets = `<svg xmlns="http://www.w3.org/2000/svg" data-document-kind="spreadsheet" viewBox="0 0 72 160">
  <g><rect class="sheet" x="0" y="0" width="72" height="72" fill="red"/></g>
  <g><rect class="sheet" x="0" y="88" width="72" height="72" fill="blue"/></g></svg>`

describe('bounded Office SVG projection', () => {
  it('renders a nonzero worksheet origin at full size and preserves explicit source order', async () => {
    const pages = officeSvgPages(sheets, '2,1,2')
    expect(pages.map((page) => page.sourcePage)).toEqual([2, 1])
    for (const [index, page] of pages.entries()) {
      const raster = await decodePng(new Uint8Array(await sharp(Buffer.from(page.svg)).png().toBuffer()))
      expect([raster.width, raster.height]).toEqual([96, 96])
      const color = index === 0 ? [0, 0, 255, 255] : [255, 0, 0, 255]
      for (let pixel = 0; pixel < raster.rgba.length; pixel += 4) {
        expect([...raster.rgba.subarray(pixel, pixel + 4)]).toEqual(color)
      }
    }
  })

  it('retains a later Word semantic slice and replaces old intrinsic dimensions', async () => {
    const y = (1000 * 11) / 8.5
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1" viewBox="0 0 72 ${y + 72}">
      <rect x="0" y="${y}" width="72" height="72" fill="#00ff00"/></svg>`
    const [page] = officeSvgPages(svg, '2')
    const raster = await decodePng(new Uint8Array(await sharp(Buffer.from(page.svg)).png().toBuffer()))
    expect(page.sourcePage).toBe(2)
    expect([raster.width, raster.height]).toEqual([96, 96])
    expect([...raster.rgba.subarray((48 * 96 + 48) * 4, (48 * 96 + 48) * 4 + 4)]).toEqual([0, 255, 0, 255])
  })

  it('refuses malformed bounds and impossible ranges before any raster allocation', () => {
    for (const value of ['0 0 NaN 72', '0 0 0 72', '0 0 72 Infinity', '0 0 72 -1', '0 0 72']) {
      expect(() => officeSvgPages(`<svg viewBox="${value}"/>`)).toThrow('unreadable')
    }
    for (const range of ['0', '3', '2-1', '1,', 'x', '1'.repeat(1025)]) {
      expect(() => officeSvgPages(sheets, range)).toThrow('office-page-range')
    }
    expect(() => officeSvgPages('<svg viewBox="0 0 100000 100000"/>')).toThrow('office-raster-limit')
  })

  it('bounds aggregate selected pixels while allowing a smaller explicit range', () => {
    const svg = '<svg viewBox="0 0 1000 25000"/>'
    expect(() => officeSvgPages(svg)).toThrow('office-raster-limit')
    expect(officeSvgPages(svg, '2')).toHaveLength(1)
    const largeSvg = `<svg viewBox="0 0 100 25000"><desc>${'x'.repeat(4 * 1024 * 1024)}</desc></svg>`
    expect(() => officeSvgPages(largeSvg)).toThrow('office-raster-limit')
    expect(officeSvgPages(largeSvg, '2')).toHaveLength(1)
  })
})
