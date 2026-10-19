import { afterEach, describe, expect, it, vi } from 'vitest'

import { openMessageFileEditor, opensInMessageFileEditor } from '../openMessageFileEditor'

afterEach(() => {
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
})

describe('openMessageFileEditor', () => {
  it('leaves raster pictures and unknown binaries on the in-page preview', () => {
    expect(opensInMessageFileEditor('封面.png')).toBe(false)
    expect(opensInMessageFileEditor('归档.zip')).toBe(false)
    expect(opensInMessageFileEditor('规划.xlsx')).toBe(true)
    expect(opensInMessageFileEditor('说明.svg')).toBe(true)
  })

  it('stages the attachment and opens the read-only file editor window', async () => {
    const invoke = vi.fn(async (command: string) => {
      if (command === 'prepare_message_file_preview') {
        return { root: '/tmp/a3s-message-file-preview', directory: '/tmp/a3s-message-file-preview/m-1' }
      }
      return undefined
    })
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', { configurable: true, value: { invoke } })
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({
        ok: true,
        json: async () => ({ ok: true, path: '/tmp/a3s-message-file-preview/m-1/规划.xlsx' })
      }))
    )

    const opened = await openMessageFileEditor(new File(['pk'], '规划.xlsx'), 'm/1')

    expect(opened).toBe(true)
    expect(invoke).toHaveBeenNthCalledWith(1, 'prepare_message_file_preview', { key: 'm-1' })
    expect(invoke).toHaveBeenNthCalledWith(2, 'open_file_editor_window', {
      root: '/tmp/a3s-message-file-preview',
      path: '/tmp/a3s-message-file-preview/m-1/规划.xlsx',
      title: '规划.xlsx',
      readOnly: true
    })
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining('/api/workspace-file/import'),
      expect.objectContaining({ method: 'POST' })
    )
  })

  it('keeps the in-page preview when the shell cannot open a window', async () => {
    await expect(openMessageFileEditor(new File(['pk'], '规划.xlsx'), 'm-1')).resolves.toBe(false)
  })
})
