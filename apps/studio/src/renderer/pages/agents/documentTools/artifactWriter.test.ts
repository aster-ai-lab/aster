import { describe, expect, it, vi } from 'vitest'

import { documentArtifactWriter } from './artifactWriter'
import type { DocumentArtifact } from './toolEngine'

const artifact: DocumentArtifact = {
  output: {
    state: 'done',
    kind: 'pdf-to-image',
    files: [
      { name: 'page-1.png', mediaType: 'image/png', bytes: new Uint8Array([1, 2]) },
      { name: 'page-2.png', mediaType: 'image/png', bytes: new Uint8Array([3, 4]) }
    ]
  }
}

describe('native document artifact writer', () => {
  it('writes accepted bytes and reports all actual destinations after the last write', async () => {
    const save = vi.fn(async (name: string) => `/chosen/${name}`)
    const receipt = await documentArtifactWriter(save)(artifact)
    expect(save.mock.calls).toEqual(artifact.output.files.map((file) => [file.name, file.bytes]))
    expect(receipt.destination).toBe('/chosen/page-1.png\n/chosen/page-2.png')
    expect(Number.isFinite(Date.parse(receipt.savedAt))).toBe(true)
  })

  it('does not claim a complete save if a later destination is cancelled or denied', async () => {
    const cancelled = vi.fn().mockResolvedValueOnce('/chosen/page-1.png').mockResolvedValueOnce(null)
    await expect(documentArtifactWriter(cancelled)(artifact)).rejects.toThrow('save_cancelled')
    const denied = vi.fn().mockRejectedValue(new Error('denied'))
    await expect(documentArtifactWriter(denied)(artifact)).rejects.toThrow('denied')
    expect(artifact.output.files[1].bytes).toEqual(new Uint8Array([3, 4]))
  })
})
