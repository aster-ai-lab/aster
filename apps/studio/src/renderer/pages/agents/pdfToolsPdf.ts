import {
  PDFDocument,
  StandardFonts,
  degrees,
  pushGraphicsState,
  popGraphicsState,
  rectangle,
  clip,
  endPath,
  type PDFFont,
  type PDFPage
} from 'pdf-lib'

import { DEFAULT_IMAGE_PDF, imagePdfPlacement, validateImagePdf, type ImagePdfOptions } from './documentTools/imagePdf'
import { IMAGE_PDF_DPI } from './pdfToolsRaster'

const CJK_FONT = '../../../../../../packages/office/node_modules/@embedpdf/fonts-sc/fonts/NotoSansHans-Regular.otf'

/** Recognized text in image pixels. Origin is the top-left of the page raster. */
export type PdfTextBox = { text: string; x: number; y: number; width: number; height: number }

let cjkFontBytes: Promise<Uint8Array> | null = null

export async function pngsToPdf(
  pngs: readonly Uint8Array[],
  dpi: number,
  text = '',
  boxes: readonly (readonly PdfTextBox[])[] = []
): Promise<Uint8Array> {
  if (boxes.some((page) => page.length > 0)) return pngsWithBoxes(pngs, dpi, boxes)
  const pdf = await PDFDocument.create()
  const font = await pdf.embedFont(StandardFonts.Helvetica)
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.replace(/[^\x20-\x7e]/g, ' ').trim())
    .filter((line) => line.length > 0)
    .slice(0, 30)
  for (const png of pngs) {
    const image = await pdf.embedPng(png)
    const width = (image.width * 72) / dpi
    const height = (image.height * 72) / dpi
    const page = pdf.addPage([width, height])
    page.drawImage(image, { x: 0, y: 0, width, height })
    lines.forEach((line, index) => {
      page.drawText(line.slice(0, 120), { x: 8, y: 8 + index * 10, size: 8, font })
    })
  }
  return new Uint8Array(await pdf.save())
}

async function pngsWithBoxes(
  pngs: readonly Uint8Array[],
  dpi: number,
  boxes: readonly (readonly PdfTextBox[])[]
): Promise<Uint8Array> {
  const pdf = await PDFDocument.create()
  pdf.registerFontkit(await loadFontkit())
  const font = await pdf.embedFont(await readCjkFont(), { subset: true })
  for (const [index, png] of pngs.entries()) {
    const image = await pdf.embedPng(png)
    const width = (image.width * 72) / dpi
    const height = (image.height * 72) / dpi
    const page = pdf.addPage([width, height])
    page.drawImage(image, { x: 0, y: 0, width, height })
    for (const box of boxes[index] ?? []) drawTextBox(page, font, box, dpi, height)
  }
  return new Uint8Array(await pdf.save())
}

function drawTextBox(page: PDFPage, font: PDFFont, box: PdfTextBox, dpi: number, pageHeight: number) {
  const text = box.text.trim()
  const boxWidth = (box.width * 72) / dpi
  const boxHeight = (box.height * 72) / dpi
  if (!text || boxWidth <= 0 || boxHeight <= 0) return
  let size = boxHeight
  const naturalHeight = font.heightAtSize(size)
  if (naturalHeight > boxHeight && naturalHeight > 0) size *= boxHeight / naturalHeight
  const naturalWidth = font.widthOfTextAtSize(text, size)
  if (naturalWidth > boxWidth && naturalWidth > 0) size *= boxWidth / naturalWidth
  const descent = font.heightAtSize(size) - font.heightAtSize(size, { descender: false })
  const x = (box.x * 72) / dpi
  const y = pageHeight - ((box.y + box.height) * 72) / dpi + descent
  page.drawText(text, { x, y, size, font })
}

async function loadFontkit() {
  const imported = (await import('@pdf-lib/fontkit')) as { default?: Parameters<PDFDocument['registerFontkit']>[0] }
  return imported.default ?? (imported as Parameters<PDFDocument['registerFontkit']>[0])
}

function readCjkFont(): Promise<Uint8Array> {
  cjkFontBytes ??= readCjkFontOnce().catch((error: unknown) => {
    cjkFontBytes = null
    throw error
  })
  return cjkFontBytes
}

async function readCjkFontOnce(): Promise<Uint8Array> {
  try {
    const { readFile } = await import(/* @vite-ignore */ 'node:fs/promises')
    const { fileURLToPath } = await import(/* @vite-ignore */ 'node:url')
    const { dirname, join } = await import(/* @vite-ignore */ 'node:path')
    return new Uint8Array(await readFile(join(dirname(fileURLToPath(import.meta.url)), CJK_FONT)))
  } catch (error) {
    if (error instanceof Error && error.message === 'unreadable') throw error
    try {
      const url = new URL(
        '../../../../../../packages/office/node_modules/@embedpdf/fonts-sc/fonts/NotoSansHans-Regular.otf',
        import.meta.url
      )
      const response = await fetch(url)
      if (!response.ok) throw new Error('unreadable')
      return new Uint8Array(await response.arrayBuffer())
    } catch (pageError) {
      if (pageError instanceof Error && pageError.message === 'unreadable') throw pageError
      throw new Error('unreadable')
    }
  }
}

export async function imagesToPdf(
  pngs: readonly Uint8Array[],
  options: ImagePdfOptions = DEFAULT_IMAGE_PDF
): Promise<Uint8Array> {
  const settings = validateImagePdf(options)
  const pdf = await PDFDocument.create()
  for (const png of pngs) {
    const image = await pdf.embedPng(png)
    const frame = imagePdfPlacement((image.width * 72) / IMAGE_PDF_DPI, (image.height * 72) / IMAGE_PDF_DPI, settings)
    const page = pdf.addPage([frame.pageWidth, frame.pageHeight])
    const area = frame.clip
    page.pushOperators(pushGraphicsState(), rectangle(area.x, area.y, area.width, area.height), clip(), endPath())
    page.drawImage(image, { x: frame.x, y: frame.y, width: frame.width, height: frame.height })
    page.pushOperators(popGraphicsState())
  }
  return new Uint8Array(await pdf.save())
}

/** Turn the page /Rotate flag by 90 degrees. The input bytes are not modified. */
export async function rotatePdf(bytes: Uint8Array, angle = 90): Promise<Uint8Array> {
  if (!Number.isInteger(angle) || angle < 0 || angle > 270 || angle % 90 !== 0) throw new Error('raster-edit-region')
  const pdf = await PDFDocument.load(bytes)
  for (const page of pdf.getPages()) {
    page.setRotation(degrees((page.getRotation().angle + angle) % 360))
  }
  return new Uint8Array(await pdf.save())
}
