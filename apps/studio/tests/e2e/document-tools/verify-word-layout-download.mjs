import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { inflateSync } from 'node:zlib'

import { PDFArray, PDFDict, PDFDocument, PDFName, PDFRawStream } from 'pdf-lib'
import sharp from 'sharp'

const report = JSON.parse(await readFile(process.argv[2], 'utf8'))
assert.equal(report.status, 'passed')
const evidence = report.scenarios.flatMap((scenario) => scenario.steps.flatMap((step) => step.output?.evidence ?? []))
async function open(name, count) {
  const matches = evidence.filter((item) => item.name === name)
  assert.equal(matches.length, 1, `Expected one ${name}`)
  const bytes = await readFile(matches[0].path)
  const doc = await PDFDocument.load(bytes)
  assert.equal(doc.getPageCount(), count)
  const pages = []
  for (const page of doc.getPages()) {
    assert.deepEqual(page.getSize(), { width: 612, height: 792 })
    const contents = page.node.Contents()
    const streams = contents instanceof PDFArray ? contents.asArray().map((ref) => doc.context.lookup(ref)) : [contents]
    const operators = streams
      .map((stream) => {
        assert.ok(stream instanceof PDFRawStream)
        return stream.dict.lookupMaybe(PDFName.of('Filter'), PDFName)?.toString() === '/FlateDecode'
          ? inflateSync(stream.getContents()).toString('latin1')
          : Buffer.from(stream.getContents()).toString('latin1')
      })
      .join('\n')
    const paints = [...operators.matchAll(/\/(I\d+)\s+Do/g)]
    assert.equal(paints.length, 1, 'One page image, without synthetic text overlay')
    const objects = page.node.Resources().lookup(PDFName.of('XObject'), PDFDict)
    const image = objects.lookup(PDFName.of(paints[0][1]), PDFRawStream)
    assert.equal(image.dict.lookup(PDFName.of('Filter')).toString(), '/DCTDecode')
    const raster = await sharp(Buffer.from(image.getContents()), { limitInputPixels: 16_777_216 })
      .removeAlpha()
      .raw()
      .toBuffer({ resolveWithObject: true })
    assert.deepEqual([raster.info.width, raster.info.height, raster.info.channels], [1632, 2112, 3])
    pages.push(raster)
  }
  return { path: matches[0].path, sha256: createHash('sha256').update(bytes).digest('hex'), pages }
}
const full = await open('downloads/word-layout.pdf', 2)
const selected = await open('downloads/word-layout-page-2.pdf', 1)
function count(raster, predicate, startY = 0, endY = raster.info.height) {
  let pixels = 0
  for (let y = startY; y < endY; y++)
    for (let x = 0; x < raster.info.width; x++) {
      const index = (y * raster.info.width + x) * 3
      if (predicate(...raster.data.subarray(index, index + 3))) pixels++
    }
  return pixels
}
const red = (r, g, b) => r > 130 && g < 100 && b < 100
const green = (r, g, b) => g > 60 && g > r * 1.4 && g > b * 1.4
assert.ok(count(full.pages[0], red) > 1000, 'Red bold heading survives')
assert.ok(count(full.pages[0], green) > 500, 'Green italic run survives')
assert.ok(count(full.pages[0], (r, g, b) => r > 220 && g > 220 && b < 80) > 5000, 'Table shading survives')
assert.ok(count(full.pages[1], red) > 200, 'Chinese red title is on page two')
assert.ok(count(full.pages[1], (r, g, b) => r < 100 && g < 100 && b < 100) > 500, 'Page two has visible body glyphs')
let longestUnderline = 0
const first = full.pages[0]
for (let y = 320; y < 480; y++) {
  let run = 0
  for (let x = 150; x < 750; x++) {
    const i = (y * first.info.width + x) * 3
    run = green(...first.data.subarray(i, i + 3)) ? run + 1 : 0
    longestUnderline = Math.max(longestUnderline, run)
  }
}
assert.ok(longestUnderline > 140, 'A continuous underline remains below the green text')
assert.deepEqual(selected.pages[0].data, full.pages[1].data, 'Selected physical page two matches the complete document')
await writeFile(
  process.argv[3],
  JSON.stringify(
    {
      status: 'passed',
      scope: 'Formatted DOCX browser print-layout export; image-based PDF',
      full: { path: full.path, sha256: full.sha256, pages: 2 },
      selected: { path: selected.path, sha256: selected.sha256, pages: 1 },
      longest_underline_pixels: longestUnderline,
      selected_page_pixels: 'exact'
    },
    null,
    2
  ) + '\n'
)
