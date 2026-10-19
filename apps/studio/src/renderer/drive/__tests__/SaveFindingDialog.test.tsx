import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { driveApi } from '../driveTransport'
import { SaveFindingDialog } from '../SaveFindingDialog'

vi.mock('../driveTransport', () => ({
  driveApi: {
    listNodes: vi.fn(),
    upload: vi.fn()
  }
}))

const listNodes = vi.mocked(driveApi.listNodes)
const upload = vi.mocked(driveApi.upload)

const root = {
  id: 'folder-1',
  parentId: null,
  deptId: 'dept-1',
  name: '研发',
  createdAt: '2026-10-07T00:00:00Z',
  excluded: false,
  canExclude: false
}

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

describe('save finding dialog', () => {
  it('keeps confirm off at the root and cancel does not upload', async () => {
    listNodes.mockResolvedValue([root])
    const onClose = vi.fn()
    render(<SaveFindingDialog open text="结论" sessionLabel="toboss" onClose={onClose} />)
    await screen.findByRole('button', { name: '研发' })
    expect(screen.getByRole('button', { name: /写入|messages\.save_finding_confirm/ })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: /取消|common\.cancel/ }))
    expect(onClose).toHaveBeenCalled()
    expect(upload).not.toHaveBeenCalled()
  })

  it('writes one markdown file into the opened folder', async () => {
    listNodes.mockImplementation(async (parentId?: string) => (parentId ? [] : [root]))
    upload.mockResolvedValue(undefined)
    const onClose = vi.fn()
    render(<SaveFindingDialog open text="结论正文" sessionLabel="toboss" onClose={onClose} />)
    fireEvent.click(await screen.findByRole('button', { name: '研发' }))
    const confirm = await screen.findByRole('button', { name: /写入|messages\.save_finding_confirm/ })
    await waitFor(() => expect(confirm).toBeEnabled())
    fireEvent.click(confirm)
    await waitFor(() => expect(upload).toHaveBeenCalledTimes(1))
    const [file, nodeId, deptId, label] = upload.mock.calls[0]
    expect(file.name).toBe('finding.md')
    expect(file.type).toBe('text/markdown')
    const written = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(String(reader.result))
      reader.onerror = () => reject(reader.error)
      reader.readAsText(file)
    })
    expect(written).toBe('结论正文')
    expect(nodeId).toBe('folder-1')
    expect(deptId).toBeUndefined()
    expect(label).toBe('toboss')
    expect(onClose).toHaveBeenCalled()
  })
})
