import { act, renderHook } from '@testing-library/react'
import type { TFunction } from 'i18next'
import type React from 'react'
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

import { toast } from '@renderer/services/toast'
import { FILE_TYPE, type FileMetadata } from '@renderer/types/file'
import type { ComposerAttachment } from '@renderer/utils/message/composerAttachment'

import { getSingleDroppedPathFromText, useFileDragDrop } from '../useFileDragDrop'

vi.mock('@renderer/services/toast', () => ({
  toast: {
    info: vi.fn(),
    error: vi.fn()
  }
}))

const mocks = vi.hoisted(() => ({ request: vi.fn() }))
vi.mock('@renderer/ipc', () => ({ ipcApi: { request: mocks.request } }))

const fileApi = {
  get: vi.fn(),
  getPathForFile: vi.fn(),
  createTempFile: vi.fn(),
  write: vi.fn()
}
const t = ((key: string) => key) as TFunction

function createFileMetadata(path: string): FileMetadata {
  return {
    id: 'file-1',
    name: path.split('/').pop() ?? 'note.md',
    origin_name: path.split('/').pop() ?? 'note.md',
    path,
    size: 12,
    ext: '.md',
    type: FILE_TYPE.TEXT,
    created_at: '',
    count: 1
  }
}

function createDropEvent(text: string, files: File[] = []) {
  return {
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    dataTransfer: {
      files,
      items: [],
      getData: vi.fn((type: string) => (type === 'text' ? text : ''))
    }
  } as unknown as React.DragEvent<HTMLDivElement>
}

describe('useFileDragDrop', () => {
  beforeAll(() => {
    if (typeof Blob.prototype.arrayBuffer !== 'function') {
      Blob.prototype.arrayBuffer = function (this: Blob): Promise<ArrayBuffer> {
        return new Promise((resolve, reject) => {
          const reader = new FileReader()
          reader.onload = () => resolve(reader.result as ArrayBuffer)
          reader.onerror = () => reject(reader.error)
          reader.readAsArrayBuffer(this)
        })
      }
    }
  })

  beforeEach(() => {
    fileApi.get.mockReset()
    fileApi.getPathForFile.mockReset()
    fileApi.createTempFile.mockReset()
    fileApi.write.mockReset()
    mocks.request.mockReset()
    vi.mocked(toast.info).mockReset()
    vi.mocked(toast.error).mockReset()
    Object.defineProperty(window, 'api', {
      value: {
        file: fileApi
      },
      configurable: true
    })
  })

  it('extracts a single local path from dropped text', () => {
    expect(getSingleDroppedPathFromText('  /Users/jd/Notes/a.md  ')).toBe('/Users/jd/Notes/a.md')
    expect(getSingleDroppedPathFromText('"file:///Users/jd/Notes/a.md"')).toBe('/Users/jd/Notes/a.md')
    expect(getSingleDroppedPathFromText('plain text')).toBeNull()
    expect(getSingleDroppedPathFromText('/tmp/a.md\n/tmp/b.md')).toBeNull()
  })

  it('turns a dropped file path text into a composer attachment', async () => {
    const path = '/Users/jd/Notes/a.md'
    const file = createFileMetadata(path)
    let files: ComposerAttachment[] = []
    const setFiles = vi.fn((updater: (prevFiles: ComposerAttachment[]) => ComposerAttachment[]) => {
      files = updater(files)
    })
    const onTextDropped = vi.fn()
    const onFolderPathDropped = vi.fn()
    mocks.request.mockResolvedValue({ kind: 'file' })
    fileApi.get.mockResolvedValue(file)

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.md'],
        setFiles,
        onTextDropped,
        onFolderPathDropped,
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent(path))
    })

    expect(mocks.request).toHaveBeenCalledWith('file.get_metadata', { kind: 'path', path })
    expect(fileApi.get).toHaveBeenCalledWith(path)
    expect(setFiles).toHaveBeenCalledTimes(1)
    expect(files).toEqual([expect.objectContaining({ path, origin_name: 'a.md' })])
    expect(onTextDropped).not.toHaveBeenCalled()
    expect(onFolderPathDropped).not.toHaveBeenCalled()
  })

  it('prefers DataTransfer files over a text path', async () => {
    const path = '/Users/jd/Finder/finder.md'
    const file = createFileMetadata(path)
    let files: ComposerAttachment[] = []
    const setFiles = vi.fn((updater: (prevFiles: ComposerAttachment[]) => ComposerAttachment[]) => {
      files = updater(files)
    })
    const onTextDropped = vi.fn()
    const onFolderPathDropped = vi.fn()
    const nativeFile = { name: 'finder.md' } as File
    fileApi.getPathForFile.mockReturnValue(path)
    fileApi.get.mockResolvedValue(file)
    mocks.request.mockResolvedValue({ kind: 'file' })

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.md'],
        setFiles,
        onTextDropped,
        onFolderPathDropped,
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent('/Users/jd/Notes', [nativeFile]))
    })

    expect(fileApi.getPathForFile).toHaveBeenCalledWith(nativeFile)
    expect(mocks.request).toHaveBeenCalledWith('file.get_metadata', { kind: 'path', path })
    expect(files).toEqual([expect.objectContaining({ path, origin_name: 'finder.md' })])
    expect(onTextDropped).not.toHaveBeenCalled()
    expect(onFolderPathDropped).not.toHaveBeenCalled()
  })

  it('turns a DataTransfer directory into a folder token callback', async () => {
    const path = '/Users/jd/Notes/Project Notes'
    const directory = createFileMetadata(path)
    const setFiles = vi.fn()
    const onTextDropped = vi.fn()
    const onFolderPathDropped = vi.fn()
    const nativeFile = { name: 'Project Notes' } as File
    fileApi.getPathForFile.mockReturnValue(path)
    fileApi.get.mockResolvedValue(directory)
    mocks.request.mockResolvedValue({ kind: 'directory' })

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.md'],
        setFiles,
        onTextDropped,
        onFolderPathDropped,
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent('/Users/jd/Notes/Project Notes', [nativeFile]))
    })

    expect(fileApi.getPathForFile).toHaveBeenCalledWith(nativeFile)
    expect(mocks.request).toHaveBeenCalledWith('file.get_metadata', { kind: 'path', path })
    expect(onFolderPathDropped).toHaveBeenCalledWith(path)
    expect(setFiles).not.toHaveBeenCalled()
    expect(onTextDropped).not.toHaveBeenCalled()
    expect(toast.info).not.toHaveBeenCalled()
  })

  it('handles mixed DataTransfer files and directories', async () => {
    const filePath = '/Users/jd/Finder/finder.md'
    const directoryPath = '/Users/jd/Notes/Project Notes'
    const file = createFileMetadata(filePath)
    const directory = createFileMetadata(directoryPath)
    let files: ComposerAttachment[] = []
    const setFiles = vi.fn((updater: (prevFiles: ComposerAttachment[]) => ComposerAttachment[]) => {
      files = updater(files)
    })
    const onTextDropped = vi.fn()
    const onFolderPathDropped = vi.fn()
    const nativeFile = { name: 'finder.md' } as File
    const nativeDirectory = { name: 'Project Notes' } as File
    fileApi.getPathForFile.mockImplementation((item: File) => (item === nativeFile ? filePath : directoryPath))
    fileApi.get.mockImplementation((path: string) => Promise.resolve(path === filePath ? file : directory))
    mocks.request.mockImplementation((_route: string, handle: { path: string }) =>
      Promise.resolve({ kind: handle.path === directoryPath ? 'directory' : 'file' })
    )

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.md'],
        setFiles,
        onTextDropped,
        onFolderPathDropped,
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent('/Users/jd/Notes', [nativeFile, nativeDirectory]))
    })

    expect(mocks.request).toHaveBeenCalledWith('file.get_metadata', { kind: 'path', path: filePath })
    expect(mocks.request).toHaveBeenCalledWith('file.get_metadata', { kind: 'path', path: directoryPath })
    expect(onFolderPathDropped).toHaveBeenCalledWith(directoryPath)
    expect(files).toEqual([expect.objectContaining({ path: filePath, origin_name: 'finder.md' })])
    expect(onTextDropped).not.toHaveBeenCalled()
    expect(toast.info).not.toHaveBeenCalled()
  })

  it('turns a dropped directory path text into a folder token callback', async () => {
    const path = '/Users/jd/Notes'
    const setFiles = vi.fn()
    const onTextDropped = vi.fn()
    const onFolderPathDropped = vi.fn()
    mocks.request.mockResolvedValue({ kind: 'directory' })

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.md'],
        setFiles,
        onTextDropped,
        onFolderPathDropped,
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent(path))
    })

    expect(mocks.request).toHaveBeenCalledWith('file.get_metadata', { kind: 'path', path })
    expect(onFolderPathDropped).toHaveBeenCalledWith(path)
    expect(setFiles).not.toHaveBeenCalled()
    expect(onTextDropped).not.toHaveBeenCalled()
  })

  it('keeps non-path dropped text as text', async () => {
    const setFiles = vi.fn()
    const onTextDropped = vi.fn()
    const onFolderPathDropped = vi.fn()

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.md'],
        setFiles,
        onTextDropped,
        onFolderPathDropped,
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent('plain text'))
    })

    expect(mocks.request).not.toHaveBeenCalled()
    expect(onTextDropped).toHaveBeenCalledWith('plain text')
    expect(onFolderPathDropped).not.toHaveBeenCalled()
    expect(setFiles).not.toHaveBeenCalled()
  })

  it('keeps missing URL-like text as text when the path does not resolve', async () => {
    const path = '/api/v1/users'
    const setFiles = vi.fn()
    const onTextDropped = vi.fn()
    const onFolderPathDropped = vi.fn()
    mocks.request.mockResolvedValue({ kind: 'file' })
    fileApi.get.mockResolvedValue(null)

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.md'],
        setFiles,
        onTextDropped,
        onFolderPathDropped,
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent(path))
    })

    expect(mocks.request).toHaveBeenCalledWith('file.get_metadata', { kind: 'path', path })
    expect(fileApi.get).toHaveBeenCalledWith(path)
    expect(onTextDropped).toHaveBeenCalledWith(path)
    expect(onFolderPathDropped).not.toHaveBeenCalled()
    expect(setFiles).not.toHaveBeenCalled()
    expect(toast.info).not.toHaveBeenCalled()
    expect(toast.error).not.toHaveBeenCalled()
  })

  it('shows the unsupported-file toast only after a dropped path resolves to a file', async () => {
    const path = '/Users/jd/Notes/archive.bin'
    const setFiles = vi.fn()
    const onTextDropped = vi.fn()
    const onFolderPathDropped = vi.fn()
    mocks.request.mockResolvedValue({ kind: 'file' })
    fileApi.get.mockResolvedValue({ ...createFileMetadata(path), ext: '.bin' })

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.md'],
        setFiles,
        onTextDropped,
        onFolderPathDropped,
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent(path))
    })

    expect(mocks.request).toHaveBeenCalledWith('file.get_metadata', { kind: 'path', path })
    expect(fileApi.get).toHaveBeenCalledWith(path)
    expect(toast.info).toHaveBeenCalledWith('chat.input.file_not_supported')
    expect(toast.error).not.toHaveBeenCalled()
    expect(onTextDropped).not.toHaveBeenCalled()
    expect(onFolderPathDropped).not.toHaveBeenCalled()
    expect(setFiles).not.toHaveBeenCalled()
  })

  it('saves a pathless dropped file and keeps its name out of the draft', async () => {
    const saved = { ...createFileMetadata('/tmp/paper.pdf'), name: 'paper.pdf', origin_name: 'paper.pdf', ext: '.pdf' }
    let files: ComposerAttachment[] = []
    const setFiles = vi.fn((updater: (prevFiles: ComposerAttachment[]) => ComposerAttachment[]) => {
      files = updater(files)
    })
    const onTextDropped = vi.fn()
    const nativeFile = new File(['pdf-bytes'], 'paper.pdf', { type: 'application/pdf' })
    fileApi.getPathForFile.mockReturnValue('')
    fileApi.createTempFile.mockResolvedValue('pending:1:paper.pdf')
    fileApi.write.mockResolvedValue(undefined)
    fileApi.get.mockResolvedValue(saved)
    mocks.request.mockResolvedValue({ kind: 'file' })

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.pdf'],
        setFiles,
        onTextDropped,
        onFolderPathDropped: vi.fn(),
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent('paper.pdf', [nativeFile]))
    })

    expect(fileApi.createTempFile).toHaveBeenCalledWith('paper.pdf')
    expect(new TextDecoder().decode(fileApi.write.mock.calls[0][1] as Uint8Array)).toBe('pdf-bytes')
    expect(files).toEqual([expect.objectContaining({ path: '/tmp/paper.pdf', origin_name: 'paper.pdf' })])
    expect(onTextDropped).not.toHaveBeenCalled()
    expect(toast.error).not.toHaveBeenCalled()
  })

  it('reports a file error when a dropped file cannot be saved', async () => {
    const setFiles = vi.fn()
    const onTextDropped = vi.fn()
    const nativeFile = new File(['pdf-bytes'], 'paper.pdf', { type: 'application/pdf' })
    fileApi.getPathForFile.mockReturnValue('')
    fileApi.createTempFile.mockRejectedValue(new Error('disk'))

    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.pdf'],
        setFiles,
        onTextDropped,
        onFolderPathDropped: vi.fn(),
        enabled: true,
        t
      })
    )

    await act(async () => {
      await result.current.handleDrop?.(createDropEvent('paper.pdf', [nativeFile]))
    })

    expect(toast.error).toHaveBeenCalledWith('chat.input.file_error')
    expect(onTextDropped).not.toHaveBeenCalled()
    expect(setFiles).not.toHaveBeenCalled()
  })

  it('shows the drop affordance only while files are over the composer', () => {
    const { result } = renderHook(() =>
      useFileDragDrop({
        supportedExts: ['.pdf'],
        setFiles: vi.fn(),
        onTextDropped: vi.fn(),
        enabled: true,
        t
      })
    )
    const target = document.createElement('div')
    const file = new File(['abc'], 'report.pdf', { type: 'application/pdf' })
    const fileDrag = {
      preventDefault: vi.fn(),
      stopPropagation: vi.fn(),
      currentTarget: target,
      relatedTarget: null,
      dataTransfer: { files: [file], types: ['Files'], dropEffect: 'none' }
    } as unknown as React.DragEvent<HTMLDivElement>
    const textDrag = {
      preventDefault: vi.fn(),
      stopPropagation: vi.fn(),
      currentTarget: target,
      relatedTarget: null,
      dataTransfer: { files: [], types: ['text/plain'], dropEffect: 'none' }
    } as unknown as React.DragEvent<HTMLDivElement>

    act(() => {
      result.current.handleDragEnter?.(textDrag)
    })
    expect(result.current.isDragging).toBe(false)

    act(() => {
      result.current.handleDragOver?.(fileDrag)
    })
    expect(result.current.isDragging).toBe(true)
    expect(fileDrag.dataTransfer.dropEffect).toBe('copy')

    act(() => {
      result.current.handleDragLeave?.(fileDrag)
    })
    expect(result.current.isDragging).toBe(false)
  })
})
