import { afterEach, describe, expect, it, vi } from 'vitest'

import { captureOfficePages, writeOfficeDocx, writeOfficeXlsx } from '../pdfToolsOffice'
import { pdfToolFailureReason } from '../pdfToolsRun'

vi.mock('node:fs/promises', () => ({}))
const { AbortController } = globalThis

afterEach(() => vi.unstubAllGlobals())

describe('native Office request settlement', () => {
  it('keeps cancellation pending until the dispatched host operation actually settles', async () => {
    let finish!: (value: unknown) => void
    const invoke = vi.fn(
      () =>
        new Promise((resolve) => {
          finish = resolve
        })
    )
    vi.stubGlobal('__TAURI_INTERNALS__', { invoke })
    const controller = new AbortController()
    let settled = false
    const result = captureOfficePages(new Uint8Array([80, 75]), 'notes.docx', controller.signal).catch(
      (error: Error) => {
        settled = true
        return error.message
      }
    )
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledOnce())
    controller.abort()
    await Promise.resolve()
    expect(settled).toBe(false)
    finish({ svg: '<svg/>', textJson: '{}' })
    expect(await result).toBe('abort')
  })

  it('preserves bounded host errors as localized, actionable failures', async () => {
    for (const code of ['office-timeout', 'office-output-limit']) {
      vi.stubGlobal('__TAURI_INTERNALS__', { invoke: vi.fn().mockRejectedValue(code) })
      await expect(captureOfficePages(new Uint8Array([80, 75]), 'notes.docx')).rejects.toThrow(code)
      expect(pdfToolFailureReason(new Error(code))).toBe(`workbench.feature.pdf.tools.job.${code.replaceAll('-', '_')}`)
    }
  })

  it('refuses invalid aggregate writers before IPC or process creation', async () => {
    const invoke = vi.fn()
    vi.stubGlobal('__TAURI_INTERNALS__', { invoke })
    await expect(writeOfficeDocx('x\0y')).rejects.toThrow('unreadable')
    await expect(writeOfficeXlsx([['x'.repeat(600000), 'y'.repeat(600000)]])).rejects.toThrow('unreadable')
    await expect(writeOfficeXlsx([Array(27).fill('x')])).rejects.toThrow('unreadable')
    expect(invoke).not.toHaveBeenCalled()
  })
})
