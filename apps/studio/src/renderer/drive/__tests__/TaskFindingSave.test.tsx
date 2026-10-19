import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { PERSONAL_CENTER, writeCenterId } from '@renderer/data/memberOrgs'

import { driveApi } from '../driveTransport'
import {
  resetSavedTaskProductsForTests,
  TaskFindingSave,
  TaskFindingSessionContext,
  TaskWorkspaceRootContext,
  type TaskProductFile
} from '../TaskFindingSave'

vi.mock('../driveTransport', () => ({
  driveApi: {
    listNodes: vi.fn(),
    upload: vi.fn()
  }
}))

const listNodes = vi.mocked(driveApi.listNodes)
const upload = vi.mocked(driveApi.upload)
const ORG = '0b9d2a4e-1c3f-4a5b-8d6e-7f8091a2b3c4'

const folder = {
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
  resetSavedTaskProductsForTests()
  writeCenterId(PERSONAL_CENTER)
  localStorage.clear()
  document.cookie = 'a3s.org.center=; Path=/; Max-Age=0'
})

const notes: TaskProductFile = { name: 'notes.md', path: '/workspace/notes.md' }
const bytes = new Uint8Array([9, 8, 7])

function renderSave(
  file: TaskProductFile | null,
  streaming: boolean,
  sessionId?: string,
  root = '/workspace',
  readFile: (root: string, path: string) => Promise<Uint8Array> = async () => bytes,
  peers?: readonly TaskProductFile[]
) {
  return render(
    <TaskWorkspaceRootContext value={root}>
      <TaskFindingSessionContext value={sessionId}>
        <TaskFindingSave file={file} peers={peers} streaming={streaming} readFile={readFile} />
      </TaskFindingSessionContext>
    </TaskWorkspaceRootContext>
  )
}

function blobBytes(file: Blob): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer))
    reader.onerror = () => reject(reader.error ?? new Error('read failed'))
    reader.readAsArrayBuffer(file)
  })
}

async function confirmSave() {
  fireEvent.click(document.querySelector('[data-ui="task.save-finding"]')!)
  fireEvent.click(await screen.findByRole('button', { name: '研发' }))
  const confirm = document.querySelector('[data-ui="messages.save-finding.confirm"]') as HTMLButtonElement
  await waitFor(() => expect(confirm).toBeEnabled())
  fireEvent.click(confirm)
}

describe('task finding save', () => {
  it('stays quiet for a personal center, a live turn, and a turn with no products', () => {
    writeCenterId(PERSONAL_CENTER)
    const personal = renderSave(notes, false, 'session-1')
    expect(personal.container.querySelector('[data-ui="task.save-finding"]')).toBeNull()
    personal.unmount()

    writeCenterId(ORG)
    const live = renderSave(notes, true, 'session-1')
    expect(live.container.querySelector('[data-ui="task.save-finding"]')).toBeNull()
    live.unmount()

    const empty = renderSave(null, false, 'session-1')
    expect(empty.container.querySelector('[data-ui="task.save-finding"]')).toBeNull()
  })

  it('places the control beside the product files, not in the message toolbar', () => {
    writeCenterId(ORG)
    const { container } = render(
      <div className="message">
        <div data-ui="chat.message-menu-bar.message-actions part:message-actions" />
        <TaskWorkspaceRootContext value="/workspace">
          <TaskFindingSessionContext value="session-1">
            <TaskFindingSave file={notes} streaming={false} readFile={async () => bytes} />
          </TaskFindingSessionContext>
        </TaskWorkspaceRootContext>
      </div>
    )
    const button = container.querySelector('[data-ui="task.save-finding"]')
    const bar = container.querySelector('[data-ui~="part:message-actions"]')
    expect(button).not.toBeNull()
    expect(bar?.contains(button)).toBe(false)
    expect(button?.querySelector('svg')).not.toBeNull()
    expect(button?.textContent).not.toContain('写入云盘')
  })

  it('uploads each product file and keeps a short session label', async () => {
    writeCenterId(ORG)
    listNodes.mockImplementation(async (parentId?: string) => (parentId ? [] : [folder]))
    upload.mockResolvedValue(undefined)
    const readFile = vi.fn(async () => bytes)
    renderSave(notes, false, 'session-1', '/workspace', readFile)
    await confirmSave()
    await waitFor(() => expect(upload).toHaveBeenCalledTimes(1))
    const [file, nodeId, deptId, label] = upload.mock.calls[0]
    expect(file.name).toBe('notes.md')
    expect(file.size).toBe(bytes.byteLength)
    expect(await blobBytes(file)).toEqual(bytes)
    expect(nodeId).toBe('folder-1')
    expect(deptId).toBeUndefined()
    expect(label).toBe('session-1')
    expect(readFile).toHaveBeenCalledWith('/workspace', '/workspace/notes.md')
  })

  it('joins a relative product path onto the workspace root', async () => {
    writeCenterId(ORG)
    listNodes.mockImplementation(async (parentId?: string) => (parentId ? [] : [folder]))
    upload.mockResolvedValue(undefined)
    const readFile = vi.fn(async () => bytes)
    renderSave({ name: 'app.ts', path: 'src/app.ts' }, false, 'session-1', '/workspace', readFile)
    await confirmSave()
    await waitFor(() => expect(readFile).toHaveBeenCalledWith('/workspace', '/workspace/src/app.ts'))
  })

  it('prefixes a parent folder when two products share a name', async () => {
    writeCenterId(ORG)
    listNodes.mockImplementation(async (parentId?: string) => (parentId ? [] : [folder]))
    upload.mockResolvedValue(undefined)
    const peers = [
      { name: 'notes.md', path: '/workspace/a/notes.md' },
      { name: 'notes.md', path: '/workspace/b/notes.md' }
    ]
    render(
      <TaskWorkspaceRootContext value="/workspace">
        <TaskFindingSave file={peers[0]} peers={peers} streaming={false} readFile={async () => bytes} />
        <TaskFindingSave file={peers[1]} peers={peers} streaming={false} readFile={async () => bytes} />
      </TaskWorkspaceRootContext>
    )
    const buttons = () => [...document.querySelectorAll('[data-ui="task.save-finding"]')]
    expect(buttons()).toHaveLength(2)
    fireEvent.click(buttons()[0])
    fireEvent.click(await screen.findByRole('button', { name: '研发' }))
    fireEvent.click(document.querySelector('[data-ui="messages.save-finding.confirm"]')!)
    await waitFor(() => expect(upload).toHaveBeenCalledTimes(1))
    expect(upload.mock.calls[0][0].name).toBe('a-notes.md')
    await waitFor(() => expect(buttons()).toHaveLength(1))
    fireEvent.click(buttons()[0])
    fireEvent.click(await screen.findByRole('button', { name: '研发' }))
    fireEvent.click(document.querySelector('[data-ui="messages.save-finding.confirm"]')!)
    await waitFor(() => expect(upload).toHaveBeenCalledTimes(2))
    expect(upload.mock.calls[1][0].name).toBe('b-notes.md')
    await waitFor(() => expect(buttons()).toHaveLength(0))
  })

  it('keeps the dialog open when the workspace root is missing', async () => {
    writeCenterId(ORG)
    listNodes.mockImplementation(async (parentId?: string) => (parentId ? [] : [folder]))
    upload.mockResolvedValue(undefined)
    const readFile = vi.fn(async () => bytes)
    renderSave(notes, false, 'session-1', '   ', readFile)
    await confirmSave()
    expect(await screen.findByText('read failed')).toBeTruthy()
    expect(document.querySelector('[data-ui="messages.save-finding"]')).not.toBeNull()
    expect(upload).not.toHaveBeenCalled()
    expect(readFile).not.toHaveBeenCalled()
  })

  it('omits a session label longer than 64 characters', async () => {
    writeCenterId(ORG)
    listNodes.mockImplementation(async (parentId?: string) => (parentId ? [] : [folder]))
    upload.mockResolvedValue(undefined)
    renderSave(notes, false, 's'.repeat(65))
    await confirmSave()
    await waitFor(() => expect(upload).toHaveBeenCalledTimes(1))
    expect(upload.mock.calls[0][3]).toBeUndefined()
  })

  it('hides the control after a product is saved and does not upload it again', async () => {
    writeCenterId(ORG)
    listNodes.mockImplementation(async (parentId?: string) => (parentId ? [] : [folder]))
    upload.mockResolvedValue(undefined)
    const first = renderSave(notes, false, 'session-1')
    await confirmSave()
    await waitFor(() => expect(document.querySelector('[data-ui="task.save-finding"]')).toBeNull())
    first.unmount()
    const again = renderSave(notes, false, 'session-1')
    expect(again.container.querySelector('[data-ui="task.save-finding"]')).toBeNull()
    expect(upload).toHaveBeenCalledTimes(1)
  })
})
