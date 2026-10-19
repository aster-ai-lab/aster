import { afterEach, describe, expect, it, vi } from 'vitest'

import { fileUrl, pathFromFileUrl, uploadTobotFile } from '../tobotFiles'

const token = 'ab'.repeat(32)

afterEach(() => {
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('tobotFiles', () => {
  it('encodes a file url the agent plane can decode', () => {
    expect(fileUrl('/tmp/a b.txt')).toBe('file:///tmp/a%20b.txt')
    expect(fileUrl('/tmp/截图.png')).toBe('file:///tmp/%E6%88%AA%E5%9B%BE.png')
    expect(pathFromFileUrl('file:///tmp/a%20b.txt')).toBe('/tmp/a b.txt')
    expect(pathFromFileUrl('https://example.com/a.txt')).toBe('')
  })

  it('stores a picked file and returns the absolute path', async () => {
    ;(globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__ = {
      apiBaseUrl: 'http://127.0.0.1:9',
      token
    }
    const fetchMock = vi.fn(
      async () =>
        new Response(JSON.stringify({ path: '/tmp/a3s-studio-composer/id_截图.png', size: 4, name: '截图.png' }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
    )
    vi.stubGlobal('fetch', fetchMock)
    const file = new File(['abcd'], '截图.png', { type: 'image/png' })
    const part = await uploadTobotFile(file)
    expect(part).toEqual({
      type: 'file',
      url: 'file:///tmp/a3s-studio-composer/id_%E6%88%AA%E5%9B%BE.png',
      filename: '截图.png',
      mediaType: 'image/png',
      sizeBytes: 4
    })
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:9/api/composer-files/%E6%88%AA%E5%9B%BE.png',
      expect.objectContaining({ method: 'POST', body: file })
    )
  })

  it('rejects an empty file before writing it', async () => {
    const fetchMock = vi.fn()
    vi.stubGlobal('fetch', fetchMock)
    await expect(uploadTobotFile(new File([], 'empty.txt'))).rejects.toMatchObject({ code: 'empty' })
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('rejects a file larger than 20 MB', async () => {
    const huge = new File(['x'], 'big.bin')
    Object.defineProperty(huge, 'size', { value: 21 * 1024 * 1024 })
    await expect(uploadTobotFile(huge)).rejects.toMatchObject({ code: 'too_large' })
  })
})
