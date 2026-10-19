import { describe, expect, it, vi } from 'vitest'

import { DocumentSourceFiles } from './sourceFiles'

describe('window-owned document sources', () => {
  it('keeps distinct same-name files bound to their bytes after remove/add and refuses removed identities', async () => {
    const first = new globalThis.File([Uint8Array.of(1)], 'same.png', { type: 'image/png' })
    const second = new globalThis.File([Uint8Array.of(2)], 'same.png', { type: 'image/png' })
    const replacement = new globalThis.File([Uint8Array.of(3)], 'same.png', { type: 'image/png' })
    const bytes = new Map([
      [first, 1],
      [second, 2],
      [replacement, 3]
    ])
    const read = vi.fn(async (file) => ({
      name: file.name,
      type: file.type,
      bytes: Uint8Array.of(bytes.get(file) ?? 0)
    }))
    const files = new DocumentSourceFiles(read)
    const [a, b] = files.add([first, second])
    files.retain([b.id])
    const [c] = files.add([replacement])
    expect(c.id).not.toBe(a.id)
    expect((await files.load(c)).bytes).toEqual(Uint8Array.of(3))
    expect((await files.load(b)).bytes).toEqual(Uint8Array.of(2))
    await expect(files.load(a)).rejects.toThrow('unreadable')
    await expect(files.load({ ...b, name: 'changed.png' })).rejects.toThrow('unreadable')
    await expect(files.load({ ...b, size: 2 })).rejects.toThrow('unreadable')
    expect(read).toHaveBeenCalledTimes(2)
  })
})
