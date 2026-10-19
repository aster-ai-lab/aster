import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import ExcelJS from 'exceljs'
import JSZip from 'jszip'
import { PDFDocument } from 'pdf-lib'
import sharp from 'sharp'

const root = join(process.argv[2], 'office-artifacts')
const receipts = []
async function artifact(name) {
  const bytes = await readFile(join(root, name))
  return { bytes, name, sha256: createHash('sha256').update(bytes).digest('hex') }
}
for (const name of ['ranges-source.xlsx', 'selected-source.xlsx', 'jpeg-source.xlsx']) {
  const file = await artifact(name)
  const book = new ExcelJS.Workbook()
  await book.xlsx.load(file.bytes)
  if (book.worksheets.length !== 2 || book.worksheets[0].getCell('A1').value !== 'first-only') {
    throw new Error(`Wrong source workbook: ${name}`)
  }
  const cached = book.worksheets[1].getCell('A1').value
  if (!cached || cached.formula !== '1+1' || cached.result !== 99) throw new Error('Lost cached formula value')
  receipts.push({ name, sha256: file.sha256, worksheets: 2, saved_formula_result: 99 })
}
const docx = await artifact('options-source.docx')
const xml = await (await JSZip.loadAsync(docx.bytes)).file('word/document.xml').async('string')
if (!xml.includes('Hello office')) throw new Error('Wrong real source DOCX')
receipts.push({ name: docx.name, sha256: docx.sha256, reopened_word_text: 'Hello office' })
const images = new Map()
for (const name of ['book-1.png', 'book-2.png', 'book-2.jpg', 'notes-1.jpg']) {
  const file = await artifact(name)
  const metadata = await sharp(file.bytes, { limitInputPixels: 16_777_216 }).metadata()
  const format = name.endsWith('.jpg') ? 'jpeg' : 'png'
  if (metadata.format !== format || metadata.width <= 1 || metadata.height <= 1) {
    throw new Error(`Invalid Office image: ${name}`)
  }
  images.set(name, { ...file, metadata })
  receipts.push({ name, sha256: file.sha256, format, width: metadata.width, height: metadata.height })
}
const source = images.get('book-2.png')
const jpeg = images.get('book-2.jpg')
if (source.metadata.width !== jpeg.metadata.width || source.metadata.height !== jpeg.metadata.height) {
  throw new Error('JPEG dimensions do not match the selected sheet PNG')
}
const original = await sharp(source.bytes).flatten({ background: '#ffffff' }).removeAlpha().raw().toBuffer()
const encoded = await sharp(jpeg.bytes).removeAlpha().raw().toBuffer()
if (original.length !== encoded.length) throw new Error('JPEG pixel length differs')
let error = 0
for (let index = 0; index < original.length; index++) error += Math.abs(original[index] - encoded[index])
const meanError = error / original.length
if (meanError > 8) throw new Error('JPEG pixels differ from the addressed worksheet')
receipts.push({ comparison: 'selected sheet PNG versus JPEG', mean_absolute_channel_error: meanError, bound: 8 })
const pdfFile = await artifact('selected-sheet-2.pdf')
const pdf = await PDFDocument.load(pdfFile.bytes)
if (pdf.getPageCount() !== 1) throw new Error('Selected worksheet PDF contains unexpected pages')
const size = pdf.getPage(0).getSize()
if (size.width !== (source.metadata.width * 72) / 96 || size.height !== (source.metadata.height * 72) / 96) {
  throw new Error('Reopened PDF page dimensions differ from its actual selected image')
}
receipts.push({
  name: pdfFile.name,
  sha256: pdfFile.sha256,
  pages: 1,
  width_points: size.width,
  height_points: size.height
})
await writeFile(
  process.argv[3],
  JSON.stringify(
    {
      scope: 'Real local Office conversion artifacts; does not certify native GUI or source print fidelity',
      status: 'passed',
      artifacts: receipts
    },
    null,
    2
  ) + '\n'
)
