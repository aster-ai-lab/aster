import { beforeAll, describe, expect, it } from 'vitest'

import { blankOfficeFileBytes, blankPdfBytes } from '../blankOfficeFile'

beforeAll(() => {
  const prototype = Blob.prototype as Blob & { arrayBuffer?: () => Promise<ArrayBuffer> }
  if (typeof prototype.arrayBuffer !== 'function') {
    prototype.arrayBuffer = function arrayBuffer(this: Blob) {
      return new Promise((resolve, reject) => {
        const reader = new FileReader()
        reader.onload = () => resolve(reader.result as ArrayBuffer)
        reader.onerror = () => reject(reader.error ?? new Error('blob read failed'))
        reader.readAsArrayBuffer(this)
      })
    }
  }
  if (typeof globalThis.XMLSerializer !== 'function') {
    globalThis.XMLSerializer = window.XMLSerializer
  }
})

describe('blank office files', () => {
  it('builds a one-page pdf', () => {
    const bytes = blankPdfBytes()
    const text = new TextDecoder().decode(bytes)
    expect(text.startsWith('%PDF-1.4\n')).toBe(true)
    const start = Number(text.split('startxref\n')[1]?.split('\n')[0])
    expect(new TextDecoder().decode(bytes.slice(start, start + 4))).toBe('xref')
  })

  it('builds a blank presentation package', async () => {
    const bytes = await blankOfficeFileBytes('presentation')
    expect(bytes[0]).toBe(0x50)
    expect(bytes[1]).toBe(0x4b)
  })

  it('builds blank document, spreadsheet, and markdown bytes', async () => {
    const documentBytes = await blankOfficeFileBytes('document')
    const sheetBytes = await blankOfficeFileBytes('spreadsheet')
    const markdown = new TextDecoder().decode(await blankOfficeFileBytes('markdown'))
    expect(documentBytes[0]).toBe(0x50)
    expect(documentBytes[1]).toBe(0x4b)
    expect(sheetBytes[0]).toBe(0x50)
    expect(sheetBytes[1]).toBe(0x4b)
    expect(markdown).toBe('')
  })
})
