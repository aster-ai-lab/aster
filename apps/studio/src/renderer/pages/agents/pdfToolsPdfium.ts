import { PDF_EXPORT_PIXEL_LIMIT, pdfRasterSize, selectedPdfPages } from './documentTools/pdfExport'
import { encodePng, type Raster } from './pdfToolsPng'
import { PDF_RASTER_DPI } from './pdfToolsRaster'

const PDFIUM_WASM = '../../../../../../packages/office/node_modules/@embedpdf/pdfium/dist/pdfium.wasm'
const PDFIUM_INDEX = '../../../../../../packages/office/node_modules/@embedpdf/pdfium/dist/index.js'

const FPDF_REVERSE_BYTE_ORDER = 0x10
const FPDF_ANNOT = 0x01

type Pdfium = {
  PDFiumExt_Init: () => void
  FPDF_InitLibrary: () => void
  FPDF_LoadMemDocument: (data: number, size: number, password: string) => number
  FPDF_GetPageCount: (doc: number) => number
  FPDF_LoadPage: (doc: number, index: number) => number
  FPDF_GetPageWidthF: (page: number) => number
  FPDF_GetPageHeightF: (page: number) => number
  FPDFBitmap_Create: (width: number, height: number, alpha: number) => number
  FPDFBitmap_FillRect: (
    bitmap: number,
    left: number,
    top: number,
    width: number,
    height: number,
    color: number
  ) => boolean
  FPDF_RenderPageBitmap: (
    bitmap: number,
    page: number,
    startX: number,
    startY: number,
    sizeX: number,
    sizeY: number,
    rotate: number,
    flags: number
  ) => void
  FPDFBitmap_GetBuffer: (bitmap: number) => number
  FPDFBitmap_GetStride: (bitmap: number) => number
  FPDFBitmap_Destroy: (bitmap: number) => void
  FPDFText_LoadPage: (page: number) => number
  FPDFText_CountChars: (textPage: number) => number
  FPDFText_GetText: (textPage: number, start: number, count: number, buffer: number) => number
  FPDFText_GetCharBox: (
    textPage: number,
    index: number,
    left: number,
    right: number,
    bottom: number,
    top: number
  ) => boolean
  FPDFText_ClosePage: (textPage: number) => void
  FPDF_ClosePage: (page: number) => void
  FPDF_CloseDocument: (doc: number) => void
  EPDF_IsEncrypted: (doc: number) => boolean
  pdfium: {
    HEAPU8: Uint8Array
    wasmExports: { malloc: (size: number) => number; free: (ptr: number) => void }
    getValue: (ptr: number, type: string) => number
  }
}

let loading: Promise<Pdfium> | null = null

export function pdfBytesLookEncrypted(bytes: Uint8Array): boolean {
  const text = new TextDecoder('latin1').decode(bytes)
  return text.includes('/Encrypt')
}

export type PdfGlyphBox = { left: number; right: number; bottom: number; top: number }

export async function pdfiumPages(
  bytes: Uint8Array,
  dpi = PDF_RASTER_DPI,
  options: { pageRange?: string; signal?: AbortSignal } = {}
): Promise<{ pages: Raster[]; texts: string[]; glyphs: PdfGlyphBox[][]; indices: readonly number[] }> {
  const pdfium = await loadPdfium()
  const heap = () => pdfium.pdfium.HEAPU8
  const ptr = pdfium.pdfium.wasmExports.malloc(bytes.length)
  try {
    heap().set(bytes, ptr)
    const doc = pdfium.FPDF_LoadMemDocument(ptr, bytes.length, '')
    if (!doc) throw new Error('unreadable')
    try {
      if (pdfium.EPDF_IsEncrypted(doc)) throw new Error('encrypted')
      const count = pdfium.FPDF_GetPageCount(doc)
      const indices = selectedPdfPages(options.pageRange ?? '', count)
      let pixels = 0
      // Refuse the entire allocation before rendering a partial huge result.
      for (const index of indices) {
        if (options.signal?.aborted) throw new Error('abort')
        const page = pdfium.FPDF_LoadPage(doc, index)
        if (!page) throw new Error('unreadable')
        try {
          const size = pdfRasterSize(pdfium.FPDF_GetPageWidthF(page), pdfium.FPDF_GetPageHeightF(page), dpi)
          pixels += size.width * size.height
          if (pixels > PDF_EXPORT_PIXEL_LIMIT) throw new Error('pdf-raster-limit')
        } finally {
          pdfium.FPDF_ClosePage(page)
        }
      }
      const pages: Raster[] = []
      const texts: string[] = []
      const glyphs: PdfGlyphBox[][] = []
      for (const index of indices) {
        if (options.signal?.aborted) throw new Error('abort')
        const page = pdfium.FPDF_LoadPage(doc, index)
        if (!page) throw new Error('unreadable')
        try {
          const text = pageText(pdfium, page)
          texts.push(text.text)
          glyphs.push(text.glyphs)
          pages.push(renderPage(pdfium, page, dpi))
        } finally {
          pdfium.FPDF_ClosePage(page)
        }
      }
      return { pages, texts, glyphs, indices }
    } finally {
      pdfium.FPDF_CloseDocument(doc)
    }
  } finally {
    pdfium.pdfium.wasmExports.free(ptr)
  }
}

function renderPage(pdfium: Pdfium, page: number, dpi: number): Raster {
  const { width, height } = pdfRasterSize(pdfium.FPDF_GetPageWidthF(page), pdfium.FPDF_GetPageHeightF(page), dpi)
  const bitmap = pdfium.FPDFBitmap_Create(width, height, 1)
  if (!bitmap) throw new Error('pdf-raster-limit')
  try {
    pdfium.FPDFBitmap_FillRect(bitmap, 0, 0, width, height, 0xffffffff)
    pdfium.FPDF_RenderPageBitmap(bitmap, page, 0, 0, width, height, 0, FPDF_ANNOT | FPDF_REVERSE_BYTE_ORDER)
    const buffer = pdfium.FPDFBitmap_GetBuffer(bitmap)
    const stride = pdfium.FPDFBitmap_GetStride(bitmap)
    const heap = pdfium.pdfium.HEAPU8
    const rgba = new Uint8Array(width * height * 4)
    for (let y = 0; y < height; y++) {
      rgba.set(heap.subarray(buffer + y * stride, buffer + y * stride + width * 4), y * width * 4)
    }
    return { width, height, rgba }
  } finally {
    pdfium.FPDFBitmap_Destroy(bitmap)
  }
}

function pageText(pdfium: Pdfium, page: number): { text: string; glyphs: PdfGlyphBox[] } {
  const textPage = pdfium.FPDFText_LoadPage(page)
  try {
    const count = pdfium.FPDFText_CountChars(textPage)
    if (count <= 0) return { text: '', glyphs: [] }
    const ptr = pdfium.pdfium.wasmExports.malloc((count + 1) * 2)
    const boxPtr = pdfium.pdfium.wasmExports.malloc(32)
    try {
      pdfium.FPDFText_GetText(textPage, 0, count, ptr)
      let text = ''
      const glyphs: PdfGlyphBox[] = []
      for (let index = 0; index < count; index++) {
        const code = pdfium.pdfium.getValue(ptr + index * 2, 'i16')
        if (code > 0) text += String.fromCharCode(code)
        if (!pdfium.FPDFText_GetCharBox(textPage, index, boxPtr, boxPtr + 8, boxPtr + 16, boxPtr + 24)) continue
        glyphs.push({
          left: pdfium.pdfium.getValue(boxPtr, 'double'),
          right: pdfium.pdfium.getValue(boxPtr + 8, 'double'),
          bottom: pdfium.pdfium.getValue(boxPtr + 16, 'double'),
          top: pdfium.pdfium.getValue(boxPtr + 24, 'double')
        })
      }
      return { text: text.replace(/\u0000/g, '').trim(), glyphs }
    } finally {
      pdfium.pdfium.wasmExports.free(boxPtr)
      pdfium.pdfium.wasmExports.free(ptr)
    }
  } finally {
    pdfium.FPDFText_ClosePage(textPage)
  }
}

async function loadPdfium(): Promise<Pdfium> {
  if (!loading) {
    loading = loadPdfiumOnce().catch((error: unknown) => {
      loading = null
      throw error
    })
  }
  return loading
}

async function loadPdfiumOnce(): Promise<Pdfium> {
  try {
    const pdfium = (await nodeFiles()) ? await loadPdfiumFromDisk() : await loadPdfiumInPage()
    pdfium.PDFiumExt_Init()
    pdfium.FPDF_InitLibrary()
    return pdfium
  } catch (error) {
    if (error instanceof Error && error.message === 'pdfium') throw error
    throw new Error('pdfium')
  }
}

async function nodeFiles(): Promise<boolean> {
  try {
    // The renderer build replaces this specifier with an empty module.
    // Only a real readFile means this process can load pdfium from disk.
    const fs = (await import(/* @vite-ignore */ 'node:fs/promises')) as { readFile?: unknown }
    return typeof fs.readFile === 'function'
  } catch {
    return false
  }
}

async function loadPdfiumFromDisk(): Promise<Pdfium> {
  const wasm = await readWasm()
  const index = await fromHere(PDFIUM_INDEX)
  const { pathToFileURL } = (await import(/* @vite-ignore */ 'node:url')) as typeof import('node:url')
  const imported = (await import(/* @vite-ignore */ pathToFileURL(index).href)) as {
    init: (options: { wasmBinary: ArrayBuffer }) => Promise<Pdfium>
  }
  return imported.init({ wasmBinary: wasm })
}

async function loadPdfiumInPage(): Promise<Pdfium> {
  const wasmUrl = new URL(
    '../../../../../../packages/office/node_modules/@embedpdf/pdfium/dist/pdfium.wasm',
    import.meta.url
  )
  const response = await fetch(wasmUrl)
  if (!response.ok) throw new Error('pdfium')
  const wasmBinary = await response.arrayBuffer()
  const imported =
    (await import('../../../../../../packages/office/node_modules/@embedpdf/pdfium/dist/index.browser.js')) as {
      init: (options: { wasmBinary: ArrayBuffer; locateFile?: (file: string) => string }) => Promise<Pdfium>
    }
  return imported.init({ wasmBinary, locateFile: () => wasmUrl.href })
}

async function readWasm(): Promise<ArrayBuffer> {
  const { readFile } = (await import(/* @vite-ignore */ 'node:fs/promises')) as typeof import('node:fs/promises')
  const bytes = await readFile(await fromHere(PDFIUM_WASM))
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer
}

async function fromHere(relative: string): Promise<string> {
  const { fileURLToPath } = (await import(/* @vite-ignore */ 'node:url')) as typeof import('node:url')
  const { dirname, join } = (await import(/* @vite-ignore */ 'node:path')) as typeof import('node:path')
  return join(dirname(fileURLToPath(import.meta.url)), relative)
}

export async function pngOf(raster: Raster): Promise<Uint8Array> {
  return encodePng(raster)
}
