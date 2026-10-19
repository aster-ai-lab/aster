import { describe, expect, it, vi } from 'vitest'

import { encodePng, solid } from '../pdfToolsPng'
import type { PdfToolRunResult } from '../pdfToolsRun'
import { documentToolEngine } from './toolEngine'

describe('document tool engine port', () => {
  it('uses the real raster converter and preserves the exact preview candidate', async () => {
    const bytes = await encodePng(solid(2, 2, [10, 80, 160, 255]))
    const file = { name: 'source.png', type: 'image/png', bytes }
    const source = { id: 'source', name: file.name, type: file.type, size: bytes.byteLength }
    const artifact = await documentToolEngine('filter', async () => file, {
      autoCrop: false,
      filter: 'grayscale'
    })(source, new globalThis.AbortController().signal)
    expect(artifact.preview?.files[0]).toBe(artifact.output.files[0])
    expect(artifact.output.files[0].bytes).not.toEqual(bytes)
  })

  it('refuses changed source bytes and cancellation before invoking the converter', async () => {
    const run = vi.fn(
      async (): Promise<PdfToolRunResult> => ({
        state: 'failed',
        kind: 'filter',
        reason: 'failure'
      })
    )
    const source = { id: 'source', name: 'source.png', type: 'image/png', size: 10 }
    const engine = documentToolEngine(
      'filter',
      async () => ({ name: source.name, type: source.type, bytes: new Uint8Array([1]) }),
      {},
      run
    )
    await expect(engine(source, new globalThis.AbortController().signal)).rejects.toThrow('unreadable')
    const controller = new globalThis.AbortController()
    controller.abort()
    await expect(engine(source, controller.signal)).rejects.toThrow('cancel')
    expect(run).not.toHaveBeenCalled()
  })
})
