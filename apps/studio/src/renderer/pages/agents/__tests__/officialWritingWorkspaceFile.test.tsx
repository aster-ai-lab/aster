import { render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  documentEditorPaneWidth,
  DOCUMENT_EDITOR_PANE_WIDTH,
  OfficialWritingFileOpener,
  officialWritingCopyName,
  officialWritingFileInstruction,
  officialWritingWorkspaceNote,
  placeOfficialWritingCopy,
  publishOfficialWritingFile,
  resetOfficialWritingFileForTests
} from '../officialWritingWorkspaceFile'

const documents = vi.hoisted(() => ({
  createLocalEntry: vi.fn(),
  writeLocalDocumentBytes: vi.fn(),
  removeLocalEntry: vi.fn(),
  localEntryStatus: vi.fn(() => 0)
}))

const office = vi.hoisted(() => ({
  createArtifact: vi.fn(() => ({
    id: 'artifact-1',
    kind: 'document',
    title: '无标题文字',
    content: { type: 'document', html: '<p></p>', pageSize: 'a4' }
  })),
  createArtifactBlob: vi.fn(async () => ({
    arrayBuffer: async () => Uint8Array.from([0x50, 0x4b, 3, 4]).buffer
  }))
}))

vi.mock('@renderer/pages/documents/useLocalDocuments', () => documents)
vi.mock('@a3s-lab/office/core', () => office)

describe('official writing workspace copy', () => {
  beforeEach(() => {
    documents.createLocalEntry.mockReset()
    documents.writeLocalDocumentBytes.mockReset()
    documents.removeLocalEntry.mockReset()
    documents.localEntryStatus.mockReset()
    documents.localEntryStatus.mockImplementation((error: unknown) => {
      if (error && typeof error === 'object' && 'status' in error) return Number((error as { status: number }).status)
      return 0
    })
    documents.writeLocalDocumentBytes.mockResolvedValue(undefined)
    documents.removeLocalEntry.mockResolvedValue(undefined)
    office.createArtifact.mockClear()
    office.createArtifactBlob.mockClear()
    resetOfficialWritingFileForTests()
  })

  afterEach(() => {
    resetOfficialWritingFileForTests()
  })

  it('names the copy after the document type and suffixes a taken name', () => {
    expect(officialWritingCopyName('notice', 0)).toBe('通知.docx')
    expect(officialWritingCopyName('notice', 1)).toBe('通知-2.docx')
    expect(officialWritingCopyName('general', 0)).toBe('通用公文.docx')
    expect(officialWritingCopyName('missing', 0)).toBeNull()
    expect(officialWritingWorkspaceNote('通知.docx')).toBe(
      '请直接修改会话工作区里的「通知.docx」，不要在对话里另写一份正文。'
    )
    const instruction = officialWritingFileInstruction(
      { typeId: 'notice', brief: '周五开会', choices: { banner: false }, fields: { org: '市教育局' } },
      '通知.docx'
    )
    expect(instruction).toContain('会话工作区里的「通知.docx」')
    expect(instruction).toContain('周五开会')
    expect(instruction).toContain('不使用红头')
    expect(instruction).not.toContain('（待填写）')
    expect(instruction).not.toContain('特此通知')
    expect(documentEditorPaneWidth(280)).toBe(DOCUMENT_EDITOR_PANE_WIDTH)
    expect(documentEditorPaneWidth(800)).toBe(800)
    expect(documentEditorPaneWidth(null)).toBe(DOCUMENT_EDITOR_PANE_WIDTH)
  })

  it('writes the configured template into the session workspace and retries a taken name', async () => {
    documents.createLocalEntry
      .mockRejectedValueOnce(Object.assign(new Error('taken'), { status: 409 }))
      .mockImplementation(async (_root: string, _parent: string, name: string) => ({
        name,
        path: `/tmp/workspace/${name}`,
        kind: 'file' as const,
        sizeBytes: 1,
        modifiedMs: 0
      }))

    const placed = await placeOfficialWritingCopy('/tmp/workspace', {
      typeId: 'notice',
      brief: '周五开会',
      choices: { banner: true },
      fields: { org: '市教育局', mainTo: '各学校' }
    })

    expect(placed).toEqual({ path: '/tmp/workspace/通知-2.docx', name: '通知-2.docx' })
    expect(documents.createLocalEntry).toHaveBeenNthCalledWith(
      1,
      '/tmp/workspace',
      '/tmp/workspace',
      '通知.docx',
      'file'
    )
    expect(documents.createLocalEntry).toHaveBeenNthCalledWith(
      2,
      '/tmp/workspace',
      '/tmp/workspace',
      '通知-2.docx',
      'file'
    )
    const bytes = documents.writeLocalDocumentBytes.mock.calls[0]?.[2] as Uint8Array
    expect(Array.from(bytes)).toEqual([0x50, 0x4b, 3, 4])
    const artifact = office.createArtifactBlob.mock.calls[0]?.[0] as {
      title: string
      content: { type: string; html: string; model?: unknown }
    }
    expect(artifact.title).toBe('通知')
    expect(artifact.content.type).toBe('document')
    expect(artifact.content.model).toBeUndefined()
    expect(artifact.content.html).toContain('周五开会')
    expect(artifact.content.html).toContain('市教育局')
    expect(artifact.content.html).toContain('各学校')
    expect(artifact.content.html).toContain('特此通知')
  })

  it('removes the entry when the bytes cannot be written', async () => {
    documents.createLocalEntry.mockResolvedValue({
      name: '通知.docx',
      path: '/tmp/workspace/通知.docx',
      kind: 'file',
      sizeBytes: 1,
      modifiedMs: 0
    })
    documents.writeLocalDocumentBytes.mockRejectedValue(new Error('write failed'))

    await expect(
      placeOfficialWritingCopy('/tmp/workspace', {
        typeId: 'notice',
        brief: '周五开会',
        choices: {},
        fields: {}
      })
    ).rejects.toThrow('write failed')
    expect(documents.removeLocalEntry).toHaveBeenCalledWith('/tmp/workspace', '/tmp/workspace/通知.docx')
  })

  it('opens the placed file in the workspace editor', () => {
    const openFiles = vi.fn()
    const openEditor = vi.fn()
    publishOfficialWritingFile({
      sessionId: 'session-1',
      path: '/tmp/workspace/通知.docx',
      name: '通知.docx'
    })

    const view = render(
      <OfficialWritingFileOpener
        sessionId="session-1"
        workspacePath="/tmp/workspace"
        openFiles={openFiles}
        openEditor={openEditor}
      />
    )

    expect(openFiles).toHaveBeenCalledTimes(1)
    expect(openEditor).toHaveBeenCalledWith('/tmp/workspace/通知.docx', '通知.docx')

    openFiles.mockClear()
    openEditor.mockClear()
    view.rerender(
      <OfficialWritingFileOpener
        sessionId="other"
        workspacePath="/tmp/workspace"
        openFiles={openFiles}
        openEditor={openEditor}
      />
    )
    expect(openFiles).not.toHaveBeenCalled()
    expect(openEditor).not.toHaveBeenCalled()
  })
})
