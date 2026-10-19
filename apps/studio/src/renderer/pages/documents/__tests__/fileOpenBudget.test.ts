import { afterEach, describe, expect, it, vi } from 'vitest'

import { decodeUtf8Async } from '../decodeUtf8'
import {
  COMFORTABLE_TEXT_CHARS,
  PDF_READ_BYTES,
  SPREADSHEET_READ_BYTES,
  TEXT_READ_BYTES,
  VIRTUAL_TEXT_CHARS,
  VISUAL_TEXT_UNITS,
  codeMinimap,
  codeSurface,
  largeMonacoOverrides,
  readBudgetBytes,
  visualSourceAllowed
} from '../fileOpenBudget'
import { FileTooLargeError, readLocalDocumentBytes } from '../useLocalDocuments'

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('file open budget', () => {
  it('keeps word wrap for a short file and turns it off past the comfortable size', () => {
    const row = `${'a'.repeat(80)}\n`
    const wrapped = (chars: number) => row.repeat(Math.ceil(chars / row.length)).slice(0, chars)
    expect(codeSurface('a'.repeat(1000), false)).toBe('monaco')
    expect(codeSurface(wrapped(COMFORTABLE_TEXT_CHARS), false)).toBe('monaco')
    expect(codeSurface(wrapped(COMFORTABLE_TEXT_CHARS + 1), false)).toBe('monaco-large')
    expect(codeSurface('a'.repeat(10_000), false)).toBe('monaco')
    expect(codeSurface('a'.repeat(10_001), false)).toBe('monaco-large')
    expect(codeSurface(`${'a'.repeat(10_000)}\n`, false)).toBe('monaco')
  })

  it('uses the plain field in tests until the virtual limit, then refuses the file', () => {
    const over = 'a'.repeat(VIRTUAL_TEXT_CHARS + 1)
    expect(codeSurface('a'.repeat(COMFORTABLE_TEXT_CHARS + 1), true)).toBe('textarea')
    expect(codeSurface(over, true)).toBe('too-large')
    expect(codeSurface(over, false)).toBe('too-large')
  })

  it('opens markdown and csv above the visual budget as source, and caps binary office kinds', () => {
    expect(visualSourceAllowed('md', 100)).toBe(true)
    expect(visualSourceAllowed('md', VISUAL_TEXT_UNITS)).toBe(true)
    expect(visualSourceAllowed('markdown', VISUAL_TEXT_UNITS + 1)).toBe(false)
    expect(visualSourceAllowed('csv', VISUAL_TEXT_UNITS + 1)).toBe(false)
    expect(visualSourceAllowed('html', VISUAL_TEXT_UNITS + 1)).toBe(false)
    expect(visualSourceAllowed('txt', VISUAL_TEXT_UNITS + 1)).toBe(true)
    expect(readBudgetBytes('表.csv')).toBe(TEXT_READ_BYTES)
    expect(readBudgetBytes('表.tsv')).toBe(TEXT_READ_BYTES)
    expect(readBudgetBytes('表.xlsx')).toBe(SPREADSHEET_READ_BYTES)
    expect(readBudgetBytes('幻灯.pptx')).toBe(12 * 1024 * 1024)
    expect(readBudgetBytes('说明.docx')).toBe(64 * 1024 * 1024)
    expect(Math.round(32.1 * 1024 * 1024)).toBeLessThan(readBudgetBytes('说明.docx'))
    expect(readBudgetBytes('手册.pdf')).toBe(PDF_READ_BYTES)
    expect(SPREADSHEET_READ_BYTES).toBeLessThan(TEXT_READ_BYTES)
  })

  it('keeps the minimap on the right for a normal file and hides it for a large file', () => {
    expect(codeMinimap(false)).toEqual({
      enabled: true,
      side: 'right',
      showSlider: 'mouseover',
      renderCharacters: true,
      maxColumn: 120
    })
    expect(codeMinimap(true)).toMatchObject({ enabled: false, side: 'right' })
  })

  it('turns off tokenization helpers for a large buffer', () => {
    const modest = largeMonacoOverrides('笔记.ts', 1000)
    expect(modest.wordWrap).toBe('off')
    expect(modest.language).toBe('typescript')
    expect(modest.matchBrackets).toBe('never')
    expect(modest.occurrencesHighlight).toBe('off')
    expect(largeMonacoOverrides('笔记.ts', 1_000_001).language).toBe('plaintext')
  })

  it('joins a character that is split across decode chunks and rejects invalid utf-8', async () => {
    const bytes = new TextEncoder().encode('甲乙')
    expect(bytes.byteLength).toBeGreaterThan(2)
    await expect(decodeUtf8Async(bytes, 2)).resolves.toBe('甲乙')
    await expect(decodeUtf8Async(Uint8Array.from([0xff]))).resolves.toBeNull()
  })

  it('cancels a response whose content-length is past the read budget', async () => {
    const read = vi.fn()
    const cancel = vi.fn(async () => undefined)
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32) }
    })
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        headers: {
          get: (name: string) => (name.toLowerCase() === 'content-length' ? String(TEXT_READ_BYTES + 5) : null)
        },
        body: { cancel, getReader: () => ({ read }) },
        arrayBuffer: async () => {
          read()
          return new ArrayBuffer(0)
        }
      }))
    )

    const error = await readLocalDocumentBytes('/workspace/a3s-documents', '过大.txt', undefined, TEXT_READ_BYTES).then(
      () => null,
      (caught: unknown) => caught
    )
    expect(error).toBeInstanceOf(FileTooLargeError)
    expect(error).toMatchObject({ bytes: TEXT_READ_BYTES + 5 })
    expect(cancel).toHaveBeenCalledOnce()
    expect(read).not.toHaveBeenCalled()
  })
})
