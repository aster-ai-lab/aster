import { File } from 'node:buffer'
import { createHash, webcrypto } from 'node:crypto'

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { readLocalDocumentBytes } from '@renderer/pages/documents/useLocalDocuments'

import type { WorkspacePaperSnapshot } from '../paperloomSource'
import { PaperloomWorkspace } from '../PaperloomWorkspace'

const member = vi.hoisted(() => ({ id: '33333333-3333-4333-8333-333333333333' }))
vi.mock('@renderer/components/account/memberProfile', () => ({ useMemberProfile: () => ({ id: member.id }) }))
vi.mock('@renderer/pages/settings/GeneralSettings', () => ({ useDisplayedWorkspacePath: () => '/default-workspace' }))
vi.mock('@renderer/pages/documents/useLocalDocuments', () => ({ readLocalDocumentBytes: vi.fn() }))
vi.mock('@shared/studioRuntime', () => ({
  readStudioRuntime: () => ({ apiBaseUrl: 'http://127.0.0.1:49139', token: 'fixture-shell-only' })
}))
vi.mock('../../WorkbenchWorkspaceFiles', () => ({
  WorkbenchWorkspaceFilePicker: () => <div data-testid="existing-workspace-picker" />
}))
vi.mock('../PaperloomReader', () => ({
  PaperloomReader: ({
    source,
    serviceReady,
    question,
    onSourceReady
  }: {
    source: WorkspacePaperSnapshot
    serviceReady: boolean
    question: string
    onSourceReady?: () => void
  }) => (
    <div aria-label="workspace-reader" data-processing-ready={String(serviceReady)}>
      {source.path}
      <span>{question}</span>
      <button type="button" onClick={onSourceReady}>
        Display PDF
      </button>
    </div>
  )
}))

const pdf = new TextEncoder().encode('%PDF-1.7\nworkspace paper')
const digest = createHash('sha256').update(pdf).digest('hex')
let connected = false
let uploadedId = digest
let requests: { path: string; init?: RequestInit }[]

beforeEach(() => {
  member.id = '33333333-3333-4333-8333-333333333333'
  connected = false
  uploadedId = digest
  requests = []
  vi.stubGlobal('File', File)
  vi.stubGlobal('crypto', webcrypto)
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      const path = new URL(url).pathname.split('/api/paperloom/')[1]
      requests.push({ path, init })
      const value =
        path === 'connection'
          ? {
              endpoint: connected ? 'http://127.0.0.1:49135' : '',
              apiKeyConfigured: connected,
              revision: connected ? 'revision-one' : ''
            }
          : {
              code: 0,
              data:
                path === 'uploads'
                  ? { document_id: uploadedId }
                  : {
                      document_id: digest,
                      title: 'cached name',
                      source_filename: 'cached.pdf',
                      page_count: 2,
                      bytes: pdf.length,
                      reading_status: 'unread',
                      added_at: '',
                      updated_at: '',
                      tags: []
                    }
            }
      return new Response(JSON.stringify(value))
    })
  )
  vi.mocked(readLocalDocumentBytes).mockImplementation(async (_root, _path, _progress, _limit, onSnapshot) => {
    onSnapshot?.('56'.repeat(32))
    return pdf.slice()
  })
})
afterEach(() => {
  vi.clearAllMocks()
  vi.unstubAllGlobals()
})

describe('paper reading in the existing workspace', () => {
  it('does not read the source or consume a draft before the installed capability is confirmed', async () => {
    let finish!: (value: Response) => void
    vi.mocked(fetch).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve
        })
    )
    const accepted = vi.fn()
    render(
      <PaperloomWorkspace
        source={{ root: '/task-workspace', paths: ['/task-workspace/paper.pdf'] }}
        onClose={vi.fn()}
        onSourceReady={accepted}
      />
    )
    try {
      await waitFor(() => expect(fetch).toHaveBeenCalledTimes(1))
      expect(readLocalDocumentBytes).not.toHaveBeenCalled()
      expect(accepted).not.toHaveBeenCalled()
    } finally {
      finish(new Response('{}', { status: 404 }))
    }
    await screen.findByText('请先在插件页面安装论文精读。')
    expect(screen.queryByLabelText('workspace-reader')).toBeNull()
    expect(readLocalDocumentBytes).not.toHaveBeenCalled()
    expect(accepted).not.toHaveBeenCalled()
  })

  it('reads the selected workspace snapshot without a processing service or second import', async () => {
    const accepted = vi.fn()
    render(
      <PaperloomWorkspace
        source={{ root: '/task-workspace', paths: ['/task-workspace/paper.pdf'] }}
        question="Explain methods"
        onClose={vi.fn()}
        onSourceReady={accepted}
      />
    )
    const reader = await screen.findByLabelText('workspace-reader')
    expect(accepted).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'Display PDF' }))
    await waitFor(() => expect(accepted).toHaveBeenCalledTimes(1))
    fireEvent.click(screen.getByRole('button', { name: 'Display PDF' }))
    expect(accepted).toHaveBeenCalledTimes(1)
    expect(reader).toHaveTextContent('/task-workspace/paper.pdf')
    expect(reader).toHaveTextContent('Explain methods')
    expect(reader).toHaveAttribute('data-processing-ready', 'false')
    expect(vi.mocked(readLocalDocumentBytes).mock.calls[0].slice(0, 2)).toEqual([
      '/task-workspace',
      '/task-workspace/paper.pdf'
    ])
    expect(requests.map((item) => item.path)).toEqual(['connection'])
    expect(screen.queryByText('论文库', { exact: true })).toBeNull()
    expect(document.querySelector('input[type=file]')).toBeNull()
  })

  it('indexes only the accepted bytes and requires the returned source identity to match', async () => {
    connected = true
    render(
      <PaperloomWorkspace
        source={{ root: '/task-workspace', paths: ['/task-workspace/paper.pdf'] }}
        onClose={vi.fn()}
      />
    )
    await screen.findByLabelText('workspace-reader')
    expect(requests.map((item) => item.path)).toEqual(['connection'])
    fireEvent.click(screen.getByRole('button', { name: 'Display PDF' }))
    await waitFor(() =>
      expect(screen.getByLabelText('workspace-reader')).toHaveAttribute('data-processing-ready', 'true')
    )
    const upload = requests.filter((item) => item.path === 'uploads')
    expect(upload).toHaveLength(1)
    expect(upload[0].init?.body).toEqual(pdf)
    expect(requests.some((item) => /\/ocr$|\/translate$|^ai\//.test(item.path))).toBe(false)
    expect(screen.getByLabelText('workspace-reader')).toHaveTextContent('/task-workspace/paper.pdf')
  })

  it('keeps original reading available while rejecting a mismatching service document', async () => {
    connected = true
    uploadedId = 'another-document'
    render(
      <PaperloomWorkspace
        source={{ root: '/task-workspace', paths: ['/task-workspace/paper.pdf'] }}
        onClose={vi.fn()}
      />
    )
    await screen.findByLabelText('workspace-reader')
    fireEvent.click(screen.getByRole('button', { name: 'Display PDF' }))
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('源文件已变化'))
    expect(screen.getByLabelText('workspace-reader')).toHaveAttribute('data-processing-ready', 'false')
    expect(requests.some((item) => item.path === 'documents/another-document')).toBe(false)
  })

  it('withdraws the old source immediately when the member changes', async () => {
    const close = vi.fn()
    const props = { source: { root: '/task-workspace', paths: ['/task-workspace/paper.pdf'] }, onClose: close }
    const view = render(<PaperloomWorkspace {...props} />)
    await screen.findByLabelText('workspace-reader')
    const reads = vi.mocked(readLocalDocumentBytes).mock.calls.length
    member.id = '44444444-4444-4444-8444-444444444444'
    view.rerender(<PaperloomWorkspace {...props} />)
    expect(screen.queryByLabelText('workspace-reader')).toBeNull()
    expect(close).toHaveBeenCalledOnce()
    expect(vi.mocked(readLocalDocumentBytes).mock.calls).toHaveLength(reads)
  })

  it('reuses the existing picker and never guesses the first file in a multiple selection', async () => {
    render(
      <PaperloomWorkspace
        source={{ root: '/task-workspace', paths: ['/task-workspace/a.pdf', '/task-workspace/b.pdf'] }}
        onClose={vi.fn()}
      />
    )
    expect(await screen.findByTestId('existing-workspace-picker')).toBeVisible()
    expect(screen.getByText('请选择一份 PDF；文件夹和多份文件不能作为一次精读的来源。')).toBeVisible()
    expect(screen.queryByLabelText('workspace-reader')).toBeNull()
    expect(readLocalDocumentBytes).not.toHaveBeenCalled()
    expect(document.querySelector('input[type=file]')).toBeNull()
  })
})
