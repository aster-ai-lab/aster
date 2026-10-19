import ExcelJS from 'exceljs'
import JSZip from 'jszip'
import { PDFDocument, StandardFonts } from 'pdf-lib'
import { describe, expect, it, vi } from 'vitest'

import { PDF_TOOL_BYTE_LIMIT, PDF_TOOL_FILE_LIMIT, failureKey } from '../pdfToolsJob'
import { captureOfficePages, writeOfficeDocx } from '../pdfToolsOffice'
import { pdfiumPages } from '../pdfToolsPdfium'
import { decodePng, encodePng, solid, type Raster } from '../pdfToolsPng'
import { PDF_RASTER_DPI } from '../pdfToolsRaster'
import {
  LAYOUT_LABEL,
  LOCAL_ONLY_COPY,
  pdfToolFailureReason,
  runPdfToolJob,
  type PdfToolRunResult,
  type ToolBytes
} from '../pdfToolsRun'

function pngName(name: string, bytes: Uint8Array): ToolBytes {
  return { name, type: 'image/png', bytes }
}

async function pngOf(name: string, raster: Raster): Promise<ToolBytes> {
  return pngName(name, await encodePng(raster))
}

function paintCenter(width: number, height: number): Raster {
  const raster = solid(width, height, [255, 255, 255, 255])
  for (let y = 2; y < height - 2; y++) {
    for (let x = 2; x < width - 2; x++) {
      const index = (y * width + x) * 4
      raster.rgba[index] = 0
      raster.rgba[index + 1] = 0
      raster.rgba[index + 2] = 0
      raster.rgba[index + 3] = 255
    }
  }
  return raster
}

async function textPdf(text: string, pages = 1): Promise<Uint8Array> {
  const pdf = await PDFDocument.create()
  const font = await pdf.embedFont(StandardFonts.Helvetica)
  for (let index = 0; index < pages; index++) {
    const page = pdf.addPage([200, 120])
    if (text) page.drawText(text, { x: 20, y: 60, size: 16, font })
  }
  return new Uint8Array(await pdf.save())
}

async function scannedPdf(raster: Raster): Promise<Uint8Array> {
  const png = await encodePng(raster)
  const pdf = await PDFDocument.create()
  const image = await pdf.embedPng(png)
  const width = (image.width * 72) / PDF_RASTER_DPI
  const height = (image.height * 72) / PDF_RASTER_DPI
  const page = pdf.addPage([width, height])
  page.drawImage(image, { x: 0, y: 0, width, height })
  return new Uint8Array(await pdf.save())
}

async function blobBytes(blob: Blob): Promise<Uint8Array> {
  if (typeof blob.arrayBuffer === 'function') return new Uint8Array(await blob.arrayBuffer())
  return new Uint8Array(
    await new Promise<ArrayBuffer>((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(reader.result as ArrayBuffer)
      reader.onerror = () => reject(reader.error ?? new Error('unreadable'))
      reader.readAsArrayBuffer(blob)
    })
  )
}

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), { status: 200, headers: { 'Content-Type': 'application/json' } })
}

function doneOf(result: PdfToolRunResult) {
  expect(result.state).toBe('done')
  if (result.state !== 'done') throw new Error(result.state)
  return result
}

function failedOf(result: PdfToolRunResult) {
  expect(result.state).toBe('failed')
  if (result.state !== 'failed') throw new Error(result.state)
  expect(result).not.toHaveProperty('files')
  return result
}

describe('pdf tool runner', () => {
  it('names a missing office or pdfium engine', () => {
    expect(pdfToolFailureReason(new Error('office'))).toBe('workbench.feature.pdf.tools.job.gap.office')
    expect(pdfToolFailureReason(new Error('pdfium'))).toBe('workbench.feature.pdf.tools.job.gap.pdfium')
    expect(pdfToolFailureReason('unreadable')).toBe('workbench.feature.pdf.tools.job.unreadable')
    expect(pdfToolFailureReason(new Error('nope'))).toBe('workbench.feature.pdf.tools.job.failed')
  })

  it('refuses an empty, mixed, unknown, oversized, or oversized-count selection before work', async () => {
    expect(failedOf(await runPdfToolJob({ kind: 'merge', files: [] })).reason).toBe(failureKey('empty'))
    const png = await pngOf('a.png', solid(2, 2, [1, 2, 3, 255]))
    const pdf = { name: 'a.pdf', type: 'application/pdf', bytes: await textPdf('Hello layer') }
    expect(failedOf(await runPdfToolJob({ kind: null, files: [pdf, png] })).reason).toBe(failureKey('mixed'))
    expect(
      failedOf(await runPdfToolJob({ kind: 'merge', files: [{ name: 'blob.bin', type: '', bytes: Uint8Array.of(1) }] }))
        .reason
    ).toBe(failureKey('type'))
    const many = Array.from({ length: PDF_TOOL_FILE_LIMIT + 1 }, (_, index) =>
      pngName(`${index}.png`, Uint8Array.of(1))
    )
    expect(failedOf(await runPdfToolJob({ kind: 'merge', files: many })).reason).toBe(failureKey('too-many'))
    expect(
      failedOf(
        await runPdfToolJob({
          kind: 'merge',
          files: [pngName('big.png', new Uint8Array(PDF_TOOL_BYTE_LIMIT + 1))]
        })
      ).reason
    ).toBe(failureKey('too-big'))
    const fetchImpl = vi.fn()
    const encrypted = failedOf(
      await runPdfToolJob({
        kind: 'pdf-to-word',
        files: [
          { name: 'secret.pdf', type: 'application/pdf', bytes: new TextEncoder().encode('%PDF-1.4\n/Encrypt <<>>') }
        ],
        gatewayBase: 'http://gateway.test',
        fetchImpl
      })
    )
    expect(encrypted.reason).toBe('workbench.feature.pdf.tools.job.encrypted')
    expect(fetchImpl).not.toHaveBeenCalled()
  })

  it('lists legal kinds for a drop and does not run', async () => {
    const png = await pngOf('a.png', solid(2, 2, [1, 2, 3, 255]))
    const result = await runPdfToolJob({ kind: null, files: [png, { ...png, name: 'b.png' }] })
    expect(result.state).toBe('choose')
    if (result.state !== 'choose') return
    expect(result.kinds[0]).toBe('merge')
    expect(result.kinds).toContain('image-to-pdf')
    expect(result).not.toHaveProperty('files')
  })

  it('cancels an in-flight merge without a result file', async () => {
    const left = await pngOf('a.png', solid(8, 4, [255, 0, 0, 255]))
    const right = await pngOf('b.png', solid(4, 4, [0, 0, 255, 255]))
    const controller = new AbortController()
    const pending = runPdfToolJob({ kind: 'merge', files: [left, right], signal: controller.signal, autoCrop: false })
    controller.abort()
    const result = failedOf(await pending)
    expect(result.reason).toBe('workbench.feature.pdf.tools.job.cancel')
    expect(JSON.stringify(result)).not.toContain('blob:')
  })

  it('merges on one width, rotates, crops, filters, and builds one PDF page per image', async () => {
    const red = solid(4, 2, [255, 0, 0, 255])
    const blue = solid(2, 2, [0, 0, 255, 255])
    const left = await pngOf('a.png', red)
    const right = await pngOf('b.png', blue)
    const leftBytes = left.bytes.slice()
    const merged = doneOf(await runPdfToolJob({ kind: 'merge', files: [left, right], autoCrop: false }))
    expect(merged.files[0].mediaType).toBe('image/png')
    expect(merged.files[0].bytes[0]).toBe(137)
    const stacked = await decodePng(merged.files[0].bytes)
    expect(stacked.width).toBe(4)
    expect(stacked.height).toBe(4)
    expect([stacked.rgba[0], stacked.rgba[1], stacked.rgba[2]]).toEqual([255, 0, 0])
    const blueAt = (2 * stacked.width + 1) * 4
    expect([stacked.rgba[blueAt], stacked.rgba[blueAt + 1], stacked.rgba[blueAt + 2]]).toEqual([0, 0, 255])
    expect(Array.from(left.bytes)).toEqual(Array.from(leftBytes))

    const tall = failedOf(
      await runPdfToolJob({
        kind: 'merge',
        files: [await pngOf('tall.png', solid(1, 32769, [1, 1, 1, 255]))],
        autoCrop: false
      })
    )
    expect(tall.reason).toBe('workbench.feature.pdf.tools.job.too_long')

    const turnedSource = await pngOf('turn.png', solid(3, 1, [9, 8, 7, 255]))
    const untouched = turnedSource.bytes.slice()
    const turned = await runPdfToolJob({ kind: 'rotate', files: [turnedSource], confirm: false, autoCrop: false })
    expect(turned.state).toBe('preview')
    if (turned.state !== 'preview') return
    const swapped = await decodePng(turned.files[0].bytes)
    expect(swapped.width).toBe(1)
    expect(swapped.height).toBe(3)
    expect(Array.from(turnedSource.bytes)).toEqual(Array.from(untouched))
    const saved = doneOf(await runPdfToolJob({ kind: 'rotate', files: [turnedSource], confirm: true, autoCrop: false }))
    expect(saved.files[0].bytes.byteLength).toBeGreaterThan(8)
    expect(Array.from(turnedSource.bytes)).toEqual(Array.from(untouched))

    const cropSource = await pngOf('crop.png', solid(6, 4, [4, 5, 6, 255]))
    const cropBytes = cropSource.bytes.slice()
    const cropped = await runPdfToolJob({
      kind: 'rotate',
      files: [cropSource],
      rotate: false,
      crop: { x: 1, y: 1, width: 2, height: 2 },
      confirm: false,
      autoCrop: false
    })
    expect(cropped.state).toBe('preview')
    if (cropped.state !== 'preview') return
    const clipped = await decodePng(cropped.files[0].bytes)
    expect(clipped.width).toBe(2)
    expect(clipped.height).toBe(2)
    expect(Array.from(cropSource.bytes)).toEqual(Array.from(cropBytes))

    const filteredSource = await pngOf('tone.png', solid(2, 2, [20, 80, 200, 255]))
    const filteredBytes = filteredSource.bytes.slice()
    const filters = await runPdfToolJob({ kind: 'filter', files: [filteredSource], confirm: false, autoCrop: false })
    expect(filters.state).toBe('preview')
    if (filters.state !== 'preview') return
    expect(filters.previews?.map((item) => item.filter)).toEqual(['document', 'grayscale', 'contrast'])
    const decoded = await Promise.all((filters.previews ?? []).map((item) => decodePng(item.bytes)))
    expect(decoded[1]?.rgba[0]).toBe(decoded[1]?.rgba[1])
    expect(decoded[1]?.rgba[1]).toBe(decoded[1]?.rgba[2])
    expect(decoded[0]?.rgba[0]).not.toBe(decoded[1]?.rgba[0])
    expect(decoded[2]?.rgba[0]).not.toBe(decoded[1]?.rgba[0])
    expect(Array.from(filteredSource.bytes)).toEqual(Array.from(filteredBytes))

    const kept = doneOf(
      await runPdfToolJob({
        kind: 'merge',
        files: [await pngOf('ink.png', solid(4, 4, [0, 0, 0, 255]))],
        autoCrop: true
      })
    )
    expect(kept.note).toBe('workbench.feature.pdf.tools.job.crop_kept')
    expect((await decodePng(kept.files[0].bytes)).width).toBe(4)

    const trimmed = doneOf(
      await runPdfToolJob({ kind: 'merge', files: [await pngOf('margin.png', paintCenter(6, 6))], autoCrop: true })
    )
    const trimmedRaster = await decodePng(trimmed.files[0].bytes)
    expect(trimmedRaster.width).toBe(2)
    expect(trimmedRaster.height).toBe(2)
    expect(trimmed.note).toBeUndefined()

    const pdf = doneOf(
      await runPdfToolJob({
        kind: 'image-to-pdf',
        files: [
          await pngOf('one.png', solid(3, 2, [1, 2, 3, 255])),
          await pngOf('two.png', solid(3, 2, [3, 2, 1, 255]))
        ],
        autoCrop: false
      })
    )
    expect(pdf.dpi).toBe(72)
    expect(pdf.files).toHaveLength(1)
    expect(new TextDecoder().decode(pdf.files[0].bytes.slice(0, 4))).toBe('%PDF')
    expect((await pdfiumPages(pdf.files[0].bytes)).pages).toHaveLength(2)
  }, 20000)

  it('extracts a text layer without OCR and sends a blank page to /v1/ocr', async () => {
    const fetchImpl = vi.fn()
    const layered = doneOf(
      await runPdfToolJob({
        kind: 'extract',
        files: [{ name: 'layer.pdf', type: 'application/pdf', bytes: await textPdf('Hello layer') }],
        gatewayBase: 'http://gateway.test',
        fetchImpl
      })
    )
    expect(new TextDecoder().decode(layered.files[0].bytes)).toContain('Hello layer')
    expect(fetchImpl).not.toHaveBeenCalled()

    const ocrFetch = vi.fn(async (url: string, init?: RequestInit) => {
      expect(String(url)).toBe('http://gateway.test/v1/ocr')
      expect(new Headers(init?.headers).get('Content-Type')).toBe('image/png')
      return jsonResponse({ text: 'from-ocr', blocks: [] })
    })
    const blank = doneOf(
      await runPdfToolJob({
        kind: 'extract',
        files: [{ name: 'blank.pdf', type: 'application/pdf', bytes: await textPdf('') }],
        gatewayBase: 'http://gateway.test',
        fetchImpl: ocrFetch
      })
    )
    expect(ocrFetch).toHaveBeenCalledOnce()
    expect(new TextDecoder().decode(blank.files[0].bytes)).toContain('from-ocr')

    const blocked = vi.fn()
    expect(
      failedOf(
        await runPdfToolJob({
          kind: 'extract',
          files: [{ name: 'blank.pdf', type: 'application/pdf', bytes: await textPdf('') }],
          fetchImpl: blocked
        })
      ).reason
    ).toBe('workbench.feature.pdf.tools.job.gap.ocr')
    expect(blocked).not.toHaveBeenCalled()
  }, 20000)

  it('labels Word output from recognition and refuses a table-less Excel file', async () => {
    const fetchImpl = vi.fn(async () => jsonResponse({ text: 'Title', blocks: [{ text: 'Title' }] }))
    const word = doneOf(
      await runPdfToolJob({
        kind: 'image-to-word',
        files: [await pngOf('page.png', solid(4, 4, [20, 20, 20, 255]))],
        gatewayBase: 'http://gateway.test',
        fetchImpl,
        autoCrop: false
      })
    )
    expect(word.label).toBe(LAYOUT_LABEL)
    expect(word.label).toBe('按识别结果排版')
    const xml = await JSZip.loadAsync(word.files[0].bytes).then((zip) => zip.file('word/document.xml')?.async('string'))
    expect(xml).toContain('按识别结果排版')
    expect(xml).toContain('Title')

    const prose = vi.fn(async () =>
      jsonResponse({
        text: 'just a sentence',
        blocks: [{ text: 'just a sentence', boundingBox: { x: 0, y: 0, width: 20, height: 8 } }]
      })
    )
    const excel = failedOf(
      await runPdfToolJob({
        kind: 'image-to-excel',
        files: [await pngOf('note.png', solid(4, 4, [20, 20, 20, 255]))],
        gatewayBase: 'http://gateway.test',
        fetchImpl: prose,
        autoCrop: false
      })
    )
    expect(excel.reason).toBe('workbench.feature.pdf.tools.job.no_table')

    const tableFetch = vi.fn(async () =>
      jsonResponse({
        text: 'Left Right One Two',
        blocks: [
          { text: 'Left', boundingBox: { x: 0, y: 0, width: 10, height: 8 } },
          { text: 'Right', boundingBox: { x: 40, y: 1, width: 10, height: 8 } },
          { text: 'One', boundingBox: { x: 0, y: 30, width: 10, height: 8 } },
          { text: 'Two', boundingBox: { x: 40, y: 31, width: 10, height: 8 } }
        ]
      })
    )
    const table = doneOf(
      await runPdfToolJob({
        kind: 'image-to-excel',
        files: [await pngOf('table.png', solid(8, 8, [20, 20, 20, 255]))],
        gatewayBase: 'http://gateway.test',
        fetchImpl: tableFetch,
        autoCrop: false
      })
    )
    const sheet = await JSZip.loadAsync(table.files[0].bytes).then((zip) =>
      zip.file('xl/worksheets/sheet1.xml')?.async('string')
    )
    expect(sheet).toContain('>Left<')
    expect(sheet).toContain('>Right<')
    expect(sheet).not.toContain('Left Right One Two')

    const textFetch = vi.fn()
    expect(
      failedOf(
        await runPdfToolJob({
          kind: 'pdf-to-excel',
          files: [{ name: 'layer.pdf', type: 'application/pdf', bytes: await textPdf('Hello layer') }],
          gatewayBase: 'http://gateway.test',
          fetchImpl: textFetch
        })
      ).reason
    ).toBe('workbench.feature.pdf.tools.job.no_table')
    expect(textFetch).not.toHaveBeenCalled()
  }, 20000)

  it('renders PDF pages and re-renders office files without recomputing a cached formula', async () => {
    const bytes = await textPdf('Hello layer', 2)
    const images = doneOf(
      await runPdfToolJob({
        kind: 'pdf-to-image',
        files: [{ name: 'layer.pdf', type: 'application/pdf', bytes }]
      })
    )
    expect(images.dpi).toBe(144)
    expect(images.files).toHaveLength(2)
    expect(images.files[0]?.bytes[0]).toBe(137)
    expect((await decodePng(images.files[0].bytes)).width).toBe(400)

    const long = doneOf(
      await runPdfToolJob({
        kind: 'pdf-to-long',
        files: [{ name: 'layer.pdf', type: 'application/pdf', bytes }]
      })
    )
    expect(long.files[0]?.mediaType).toBe('image/png')
    expect((await decodePng(long.files[0].bytes)).height).toBe(480)

    const docx = await writeOfficeDocx('Hello office')
    const wordImage = doneOf(
      await runPdfToolJob({
        kind: 'word-to-image',
        files: [
          {
            name: 'notes.docx',
            type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
            bytes: docx
          }
        ]
      })
    )
    expect(wordImage.dpi).toBe(96)
    expect(wordImage.files).toHaveLength(1)
    expect(wordImage.files[0]?.bytes[0]).toBe(137)
    const wordRaster = await decodePng(wordImage.files[0].bytes)
    expect(wordRaster.width).toBe(Math.ceil((1000 * 96) / 72))
    expect(wordRaster.height).not.toBe(600)

    const wordPdf = await runPdfToolJob({
      kind: 'word-to-pdf',
      files: [
        {
          name: 'notes.docx',
          type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
          bytes: docx
        }
      ]
    })
    expect(wordPdf).toEqual({
      state: 'failed',
      kind: 'word-to-pdf',
      reason: 'workbench.feature.pdf.tools.workspace.office.layout_error'
    })

    const workbook = new ExcelJS.Workbook()
    const sheet = workbook.addWorksheet('Sheet1')
    sheet.getCell('A1').value = { formula: '1+1', result: 99 }
    const second = workbook.addWorksheet('Sheet2')
    second.getCell('A1').value = 'Second'
    const xlsx = new Uint8Array(await workbook.xlsx.writeBuffer())
    const source = xlsx.slice()
    const excelPdf = doneOf(
      await runPdfToolJob({
        kind: 'excel-to-pdf',
        files: [
          { name: 'sheet.xlsx', type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', bytes: xlsx }
        ]
      })
    )
    const excelPages = await pdfiumPages(excelPdf.files[0].bytes)
    expect(excelPages.pages).toHaveLength(2)
    expect(excelPages.texts.join('')).toBe('')
    const excelText = (await captureOfficePages(xlsx, 'sheet.xlsx')).text
    expect(excelText).toContain('99')
    expect(excelText).not.toContain('A1=2')
    const excelImages = doneOf(
      await runPdfToolJob({
        kind: 'excel-to-image',
        files: [
          { name: 'sheet.xlsx', type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', bytes: xlsx }
        ]
      })
    )
    expect(excelImages.files).toHaveLength(2)
    const sheetRaster = await decodePng(excelImages.files[0].bytes)
    expect(sheetRaster.width).toBe(Math.round((1100 * 96) / 72))
    expect(sheetRaster.height).not.toBe(600)
    expect(Array.from(xlsx)).toEqual(Array.from(source))
    const sourceSheet = await JSZip.loadAsync(source).then((zip) =>
      zip.file('xl/worksheets/sheet1.xml')?.async('string')
    )
    expect(sourceSheet).toContain('1+1')
    expect(sourceSheet).toContain('<v>99</v>')
  }, 60000)

  it('writes one minutes document from a stream or a one-shot transcript', async () => {
    const encoder = new TextEncoder()
    const streamFetch = vi.fn(async () => {
      const stream = new ReadableStream({
        start(controller) {
          controller.enqueue(encoder.encode('data: {"delta":"甲"}\n\n'))
          controller.enqueue(encoder.encode('data: {"delta":"乙"}\n\n'))
          controller.close()
        }
      })
      return new Response(stream, { headers: { 'Content-Type': 'text/event-stream' } })
    })
    const streamed = doneOf(
      await runPdfToolJob({
        kind: 'audio',
        files: [{ name: 'memo.m4a', type: 'audio/mp4', bytes: Uint8Array.of(1, 2, 3) }],
        gatewayBase: 'http://gateway.test',
        fetchImpl: streamFetch
      })
    )
    expect(String(streamFetch.mock.calls[0]?.[0])).toContain('/v1/audio/transcriptions')
    expect(streamed.streamed).toBe(true)
    expect(streamed.partials).toEqual(['甲', '乙'])
    expect(streamed.transcript).toBe('甲乙')
    expect(streamed.minutes).toContain('会议概况')
    expect(streamed.minutes).toContain('议定事项')
    expect(streamed.minutes).toContain('分工与时限')
    const minutesXml = await JSZip.loadAsync(streamed.files[0].bytes).then((zip) =>
      zip.file('word/document.xml')?.async('string')
    )
    expect(minutesXml).toContain('会议概况')

    const onceFetch = vi.fn(async () => jsonResponse({ text: '全体通过预算' }))
    const once = doneOf(
      await runPdfToolJob({
        kind: 'audio',
        files: [{ name: 'memo.wav', type: 'audio/wav', bytes: Uint8Array.of(4, 5) }],
        gatewayBase: 'http://gateway.test',
        fetchImpl: onceFetch
      })
    )
    expect(once.streamed).toBe(false)
    expect(once.partials).toEqual([])
    expect(once.transcript).toBe('全体通过预算')
    expect(once.minutes).toContain('会议概况')

    const blocked = vi.fn()
    expect(
      failedOf(
        await runPdfToolJob({
          kind: 'audio',
          files: [{ name: 'memo.m4a', type: 'audio/mp4', bytes: Uint8Array.of(1) }],
          fetchImpl: blocked
        })
      ).reason
    ).toBe('workbench.feature.pdf.tools.job.gap.asr')
    expect(blocked).not.toHaveBeenCalled()
  })

  it('keeps a scan on this computer unless a drive upload is given', async () => {
    const page = await pngOf('scan.png', paintCenter(6, 6))
    const fetchImpl = vi.fn()
    const local = doneOf(await runPdfToolJob({ kind: 'scan', files: [page], fetchImpl }))
    expect(local.localOnly).toBe(true)
    expect(local.copy).toBe(LOCAL_ONLY_COPY)
    expect(local.copy).toBe('结果只在这台电脑上。')
    expect(local.note).toBe('workbench.feature.pdf.tools.job.local_only')
    expect(new TextDecoder().decode(local.files[0].bytes.slice(0, 4))).toBe('%PDF')
    expect(fetchImpl).not.toHaveBeenCalled()

    const upload = vi.fn(async () => {})
    const remote = doneOf(await runPdfToolJob({ kind: 'scan', files: [page], upload }))
    expect(upload).toHaveBeenCalledOnce()
    const sent = upload.mock.calls[0]?.[0] as ToolBytes
    expect(sent.bytes.byteLength).toBeGreaterThan(8)
    expect(new TextDecoder().decode(sent.bytes.slice(0, 4))).toBe('%PDF')
    expect(remote.localOnly).toBe(false)
    expect(remote.copy).toBeUndefined()
  })

  it('previews a masked pixel edit and names the missing model without painting', async () => {
    const source = await pngOf('mark.png', solid(2, 2, [1, 2, 3, 255]))
    const before = source.bytes.slice()
    const blocked = vi.fn()
    for (const kind of ['handwriting', 'watermark', 'erase'] as const) {
      const missing = failedOf(await runPdfToolJob({ kind, files: [source], fetchImpl: blocked, autoCrop: false }))
      expect(missing.reason).toBe('workbench.feature.pdf.tools.job.gap.inpaint')
    }
    expect(blocked).not.toHaveBeenCalled()
    expect(Array.from(source.bytes)).toEqual(Array.from(before))

    const edited = await encodePng(solid(2, 2, [9, 9, 9, 255]))
    const fetchImpl = vi.fn(async (url: string) => {
      expect(String(url)).toBe('http://edit.test/v1/images/edits')
      expect(String(url)).not.toContain('chat')
      return new Response(edited, { headers: { 'Content-Type': 'image/png' } })
    })
    const preview = await runPdfToolJob({
      kind: 'erase',
      files: [source],
      maskedEditUrl: 'http://edit.test/v1/images/edits',
      fetchImpl,
      confirm: false,
      autoCrop: false
    })
    expect(preview.state).toBe('preview')
    expect(Array.from(source.bytes)).toEqual(Array.from(before))
    const saved = doneOf(
      await runPdfToolJob({
        kind: 'erase',
        files: [source],
        maskedEditUrl: 'http://edit.test/v1/images/edits',
        fetchImpl,
        confirm: true,
        autoCrop: false
      })
    )
    expect(Array.from(saved.files[0].bytes)).toEqual(Array.from(edited))
    expect(Array.from(source.bytes)).toEqual(Array.from(before))
  })

  it('opens the PDF editor and places in-image text only after a masked edit is confirmed', async () => {
    const layered = await textPdf('Hello layer')
    const fetchImpl = vi.fn()
    const opened = doneOf(
      await runPdfToolJob({
        kind: 'pdf-edit',
        files: [{ name: 'layer.pdf', type: 'application/pdf', bytes: layered }],
        gatewayBase: 'http://gateway.test',
        fetchImpl
      })
    )
    expect(opened.editor).toBe('pdf')
    expect(opened.inImageText).toBe(false)
    expect(JSON.stringify(opened)).not.toContain('完整还原')
    expect(fetchImpl).not.toHaveBeenCalled()

    const raster = solid(64, 48, [255, 255, 255, 255])
    for (let y = 16; y < 24; y++) {
      for (let x = 20; x < 28; x++) {
        const index = (y * raster.width + x) * 4
        raster.rgba[index] = 0
        raster.rgba[index + 1] = 0
        raster.rgba[index + 2] = 0
        raster.rgba[index + 3] = 255
      }
    }
    const box = { x: 12, y: 10, width: 24, height: 20 }
    const edited = await encodePng(solid(raster.width, raster.height, [250, 250, 250, 255]))
    const calls: string[] = []
    const masks: Uint8Array[] = []
    const masked = vi.fn(async (url: string, init?: RequestInit) => {
      calls.push(String(url))
      if (String(url).includes('/v1/ocr')) {
        return jsonResponse({ text: '你好', blocks: [{ text: '你好', boundingBox: box }] })
      }
      const body = init?.body
      if (body instanceof FormData) {
        const mask = body.get('mask')
        expect(mask).toBeInstanceOf(Blob)
        masks.push(await blobBytes(mask as Blob))
      }
      return new Response(edited, { headers: { 'Content-Type': 'image/png' } })
    })
    const scanned = await scannedPdf(raster)
    const scannedBytes = scanned.slice()
    const pending = await runPdfToolJob({
      kind: 'pdf-edit',
      files: [{ name: 'scan.pdf', type: 'application/pdf', bytes: scanned }],
      gatewayBase: 'http://gateway.test',
      maskedEditUrl: 'http://edit.test/v1/images/edits',
      fetchImpl: masked,
      confirm: false
    })
    expect(pending.state).toBe('preview')
    expect(JSON.stringify(pending)).not.toContain('完整还原')
    const placed = doneOf(
      await runPdfToolJob({
        kind: 'pdf-edit',
        files: [{ name: 'scan.pdf', type: 'application/pdf', bytes: scanned }],
        gatewayBase: 'http://gateway.test',
        maskedEditUrl: 'http://edit.test/v1/images/edits',
        fetchImpl: masked,
        confirm: true
      })
    )
    expect(placed.editor).toBe('pdf')
    expect(placed.inImageText).toBe(true)
    expect(placed.label).toBe('你好')
    expect(calls.some((url) => url.endsWith('/v1/ocr'))).toBe(true)
    expect(calls.some((url) => url.endsWith('/v1/images/edits'))).toBe(true)
    expect(masks.length).toBeGreaterThan(0)
    const mask = await decodePng(masks[0])
    let ink = 0
    let clear = 0
    let outside = 0
    for (let y = 0; y < mask.height; y++) {
      for (let x = 0; x < mask.width; x++) {
        const alpha = mask.rgba[(y * mask.width + x) * 4 + 3]
        const inside = x >= box.x && x < box.x + box.width && y >= box.y && y < box.y + box.height
        if (!inside && alpha !== 0) outside += 1
        else if (inside && alpha === 255) ink += 1
        else if (inside && alpha === 0) clear += 1
      }
    }
    expect(outside).toBe(0)
    expect(ink).toBeGreaterThan(0)
    expect(clear).toBeGreaterThan(0)
    const written = await pdfiumPages(placed.files[0].bytes)
    expect(written.texts.join('\n')).toContain('你好')
    const glyphs = written.glyphs[0].filter((glyph) => glyph.right > glyph.left)
    expect(glyphs.length).toBeGreaterThan(0)
    const heightPt = (written.pages[0].height * 72) / PDF_RASTER_DPI
    const left = (box.x * 72) / PDF_RASTER_DPI
    const right = ((box.x + box.width) * 72) / PDF_RASTER_DPI
    const bottom = heightPt - ((box.y + box.height) * 72) / PDF_RASTER_DPI
    const top = heightPt - (box.y * 72) / PDF_RASTER_DPI
    const unionLeft = Math.min(...glyphs.map((glyph) => glyph.left))
    const unionRight = Math.max(...glyphs.map((glyph) => glyph.right))
    expect(unionLeft).toBeGreaterThanOrEqual(left - 1)
    expect(unionLeft).toBeLessThan(left + 1)
    expect(unionRight).toBeLessThanOrEqual(right + 1)
    expect(unionRight).toBeGreaterThan(right - 1)
    for (const glyph of glyphs) {
      expect(glyph.bottom).toBeGreaterThanOrEqual(bottom - 1)
      expect(glyph.bottom).toBeLessThanOrEqual(top)
    }
    expect(Array.from(scanned)).toEqual(Array.from(scannedBytes))
  }, 20000)
})
