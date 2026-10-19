import { afterEach, describe, expect, it, vi } from 'vitest'

import { dropPdfToolHandoff, stagePdfToolHandoff, takePdfToolHandoff } from '../pdfToolHandoff'

function readBytes(file: File): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer))
    reader.onerror = () => reject(reader.error ?? new Error('unreadable'))
    reader.readAsArrayBuffer(file)
  })
}

function setInvoke(invoke: ((command: string, args?: Record<string, unknown>) => Promise<unknown>) | undefined) {
  Object.assign(globalThis, { __TAURI_INTERNALS__: invoke ? { invoke } : undefined })
}

describe('pdf tool handoff', () => {
  afterEach(() => {
    setInvoke(undefined)
  })

  it('stays quiet outside the shell', async () => {
    setInvoke(undefined)
    expect(await stagePdfToolHandoff('pdf-to-word', [new File(['%PDF'], 'a.pdf', { type: 'application/pdf' })])).toBe(
      false
    )
    expect(await takePdfToolHandoff()).toBeNull()
    await dropPdfToolHandoff()
  })

  it('sends the chosen file to the shell and reads the same bytes back', async () => {
    let stored: { kind?: string; files?: { name: string; mediaType: string; bytesBase64: string }[] } | null = null
    const invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'stage_pdf_tool_handoff') {
        stored = args as typeof stored
        return null
      }
      if (command === 'take_pdf_tool_handoff') return stored
      if (command === 'drop_pdf_tool_handoff') {
        stored = null
        return null
      }
      throw new Error(command)
    })
    setInvoke(invoke)
    const source = new File(['%PDF'], '纪要.pdf', { type: 'application/pdf' })
    expect(await stagePdfToolHandoff('pdf-to-word', [source])).toBe(true)
    expect(invoke).toHaveBeenCalledWith('stage_pdf_tool_handoff', {
      kind: 'pdf-to-word',
      files: [{ name: '纪要.pdf', mediaType: 'application/pdf', bytesBase64: btoa('%PDF') }]
    })
    const job = await takePdfToolHandoff()
    expect(job?.kind).toBe('pdf-to-word')
    expect(job?.files).toHaveLength(1)
    expect(job?.files[0]?.name).toBe('纪要.pdf')
    expect(job?.files[0]?.type).toBe('application/pdf')
    expect(new TextDecoder().decode(await readBytes(job!.files[0]!))).toBe('%PDF')
    await dropPdfToolHandoff()
    expect(await takePdfToolHandoff()).toBeNull()
  })

  it('reports a failed store', async () => {
    setInvoke(async () => {
      throw new Error('unreadable')
    })
    expect(await stagePdfToolHandoff('pdf-to-word', [new File(['%PDF'], 'a.pdf')])).toBe(false)
    expect(await takePdfToolHandoff()).toBeNull()
  })
})
