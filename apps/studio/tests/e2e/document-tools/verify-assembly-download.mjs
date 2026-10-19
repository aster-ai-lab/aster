import assert from 'node:assert/strict'
import { readFile, writeFile } from 'node:fs/promises'
import { inflateSync } from 'node:zlib'

import { PDFDict, PDFDocument, PDFName, PDFRawStream } from 'pdf-lib'
import sharp from 'sharp'

const report = JSON.parse(await readFile(process.argv[2], 'utf8'))
assert.equal(report.status, 'passed')
const evidence = report.scenarios.flatMap((scenario) => scenario.steps.flatMap((step) => step.output?.evidence ?? []))
const artifact = (name) => {
  const matches = evidence.filter((item) => item.name === name)
  assert.equal(matches.length, 1, `Expected exactly one ${name}`)
  return matches[0].path
}
const source = await sharp('tests/e2e/document-tools/fixtures/source.png')
  .ensureAlpha()
  .raw()
  .toBuffer({ resolveWithObject: true })
const second = await sharp('tests/e2e/document-tools/fixtures/second.png')
  .ensureAlpha()
  .raw()
  .toBuffer({ resolveWithObject: true })
assert.equal(source.info.width, 4)
assert.equal(second.info.width, 4)
const decoded = []
for (let index = 1; index <= 3; index++) {
  const path = artifact(`downloads/assembly-segment-${index}.png`)
  const out = await sharp(path, { limitInputPixels: 100 }).ensureAlpha().raw().toBuffer({ resolveWithObject: true })
  assert.deepEqual([out.info.width, out.info.height], [4, 3])
  decoded.push(out.data)
}
const gap = Buffer.alloc(4 * 4, 255)
assert.deepEqual(
  Buffer.concat(decoded),
  Buffer.concat([second.data, gap, source.data]),
  'Reordered image rows and gap must survive every seam'
)
const pdfs = []
for (const mode of ['natural', 'letter']) {
  const path = artifact(`downloads/assembly-${mode}.pdf`)
  const doc = await PDFDocument.load(await readFile(path))
  assert.equal(doc.getPageCount(), 2)
  for (const [index, page] of doc.getPages().entries()) {
    assert.deepEqual(page.getSize(), mode === 'natural' ? { width: 4, height: 4 } : { width: 612, height: 792 })
    const objects = page.node.Resources().lookup(PDFName.of('XObject'), PDFDict)
    assert.equal(objects.keys().length, 1)
    const image = objects.lookup(objects.keys()[0], PDFRawStream)
    assert.equal(image.dict.lookup(PDFName.of('Filter')).toString(), '/FlateDecode')
    const actual = inflateSync(image.getContents())
    const input = index === 0 ? source.data : second.data
    const expected = Buffer.from([...input].filter((_, channel) => channel % 4 !== 3))
    assert.deepEqual(actual, expected, `${mode} PDF page ${index + 1} must embed the correct source image`)
  }
  pdfs.push({ mode, path, pages: 2, source_images: 'exact' })
}
await writeFile(process.argv[3], JSON.stringify({ segmented_rows: 'exact', segments: 3, pdfs }, null, 2) + '\n')
