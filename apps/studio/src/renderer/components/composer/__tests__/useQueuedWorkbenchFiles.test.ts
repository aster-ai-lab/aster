import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { toast } from '@renderer/services/toast'
import { FILE_TYPE, type FileMetadata } from '@renderer/types/file'
import type { ComposerAttachment } from '@renderer/utils/message/composerAttachment'

import { useQueuedWorkbenchFiles } from '../useQueuedWorkbenchFiles'
import { queueWorkbenchDroppedFiles, subscribeWorkbenchDroppedFiles } from '../workbenchDroppedFiles'

vi.mock('@renderer/services/toast', () => ({
  toast: { info: vi.fn(), error: vi.fn() }
}))

vi.mock('@renderer/ipc', () => ({
  ipcApi: { request: vi.fn().mockRejectedValue(new Error('missing')) }
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) => (options?.count != null ? `${key}:${options.count}` : key)
  })
}))

const fileApi = {
  get: vi.fn(),
  getPathForFile: vi.fn(),
  createTempFile: vi.fn(),
  write: vi.fn()
}

function metadata(path: string): FileMetadata {
  return {
    id: 'file-1',
    name: path.split('/').pop() ?? 'note.txt',
    origin_name: path.split('/').pop() ?? 'note.txt',
    path,
    size: 12,
    ext: '.txt',
    type: FILE_TYPE.TEXT,
    created_at: '',
    count: 1
  }
}

describe('queued workbench drops', () => {
  beforeEach(() => {
    vi.mocked(toast.info).mockReset()
    vi.mocked(toast.error).mockReset()
    fileApi.get.mockReset()
    fileApi.getPathForFile.mockReset()
    fileApi.createTempFile.mockReset()
    fileApi.write.mockReset()
    Object.defineProperty(window, 'api', { value: { file: fileApi }, configurable: true })
    const stop = subscribeWorkbenchDroppedFiles(() => undefined)
    stop()
  })

  afterEach(() => {
    const stop = subscribeWorkbenchDroppedFiles(() => undefined)
    stop()
  })

  it('holds a drop until the homepage composer can attach it', async () => {
    const saved = metadata('/tmp/note.txt')
    fileApi.getPathForFile.mockReturnValue('/tmp/note.txt')
    fileApi.get.mockResolvedValue(saved)
    let files: ComposerAttachment[] = []
    const setFiles = vi.fn((updater: (prev: ComposerAttachment[]) => ComposerAttachment[]) => {
      files = updater(files)
    })
    queueWorkbenchDroppedFiles([new File(['hi'], 'note.txt', { type: 'text/plain' })])

    renderHook(() => useQueuedWorkbenchFiles({ enabled: true, supportedExts: ['.txt'], setFiles }))

    await waitFor(() => expect(setFiles).toHaveBeenCalled())
    expect(files).toEqual([expect.objectContaining({ path: '/tmp/note.txt', origin_name: 'note.txt' })])
    expect(toast.error).not.toHaveBeenCalled()
  })

  it('explains files the composer cannot attach', async () => {
    fileApi.getPathForFile.mockReturnValue('/tmp/clip.bin')
    fileApi.get.mockResolvedValue({
      ...metadata('/tmp/clip.bin'),
      name: 'clip.bin',
      origin_name: 'clip.bin',
      ext: '.bin'
    })
    const setFiles = vi.fn()

    renderHook(() => useQueuedWorkbenchFiles({ enabled: false, supportedExts: ['.txt'], setFiles }))
    act(() => {
      queueWorkbenchDroppedFiles([new File(['x'], 'clip.bin')])
    })
    expect(setFiles).not.toHaveBeenCalled()

    renderHook(() => useQueuedWorkbenchFiles({ enabled: true, supportedExts: ['.txt'], setFiles }))
    await waitFor(() => expect(toast.info).toHaveBeenCalledWith('chat.input.file_not_supported_count:1'))
    expect(setFiles).not.toHaveBeenCalled()
  })
})
