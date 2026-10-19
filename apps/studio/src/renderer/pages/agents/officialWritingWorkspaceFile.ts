import { useEffect, useRef, useSyncExternalStore } from 'react'

import { cacheService } from '@data/CacheService'
import { loggerService } from '@logger'
import {
  ARTIFACT_RIGHT_PANE_CACHE_KEY,
  ARTIFACT_RIGHT_PANE_DEFAULT_WIDTH
} from '@renderer/components/chat/shell/paneLayout'
import {
  getAgentDraftCacheKey,
  writeAgentDraftCache
} from '@renderer/components/composer/variants/agent/agentDraftCache'
import {
  createLocalEntry,
  localEntryStatus,
  removeLocalEntry,
  writeLocalDocumentBytes
} from '@renderer/pages/documents/useLocalDocuments'

import {
  type OfficialWritingCopyRequest,
  type OfficialWritingHandoff,
  officialWritingCopyRequest,
  officialWritingLayoutPhrase,
  officialWritingType
} from './officialWriting'
import { armOfficialWritingDocument } from './officialWritingAgentEdit'
import { officialWritingTemplateHtml } from './officialWritingTemplate'
import { conversationEditorTarget } from './taskConversation/focusConversationFile'
import { markPendingHomeSubmit } from './workbenchComposerHost'
import { workspaceEntryName } from './workbenchWorkspace'

const logger = loggerService.withContext('officialWritingWorkspaceFile')

const COPY_ATTEMPTS = 20

export interface OfficialWritingWorkspaceFile {
  sessionId: string
  path: string
  name: string
}

/** Readable A4 column beside the conversation. Narrower than a full page so the chat stays visible. */
export const DOCUMENT_EDITOR_PANE_WIDTH = 720

/** One sentence so the assistant edits the workspace copy. */
export function officialWritingWorkspaceNote(name: string): string {
  return `请直接修改会话工作区里的「${name}」，不要在对话里另写一份正文。`
}

/**
 * The task sentence once the copy exists.
 * The file already holds the template, so this does not paste that body again.
 */
export function officialWritingFileInstruction(request: OfficialWritingCopyRequest, name: string): string {
  const type = officialWritingType(request.typeId)
  const brief = request.brief.trim()
  const note = officialWritingWorkspaceNote(name)
  if (!type || brief === '') return note
  const layout = officialWritingLayoutPhrase(type.id, request.choices)
  const layoutSentence = layout === '' ? '' : `版式：${layout}。`
  return `请按公文体例完成会话工作区里的「${name}」。补上发文机关、事由和主送机关，保留层次和结束语。${layoutSentence}\n\n写作说明：\n${brief}\n\n${note}`
}

/** Keep a wider pane when it is already wide enough for the page. */
export function documentEditorPaneWidth(current: number | null | undefined): number {
  const width = typeof current === 'number' && Number.isFinite(current) ? current : ARTIFACT_RIGHT_PANE_DEFAULT_WIDTH
  return Math.max(width, DOCUMENT_EDITOR_PANE_WIDTH)
}

/** Widen the shared artifact pane so the open official document is readable. */
export function widenDocumentEditorPane(): void {
  cacheService.setPersist(ARTIFACT_RIGHT_PANE_CACHE_KEY, (current) => documentEditorPaneWidth(current))
}

/** `通知.docx`, then `通知-2.docx` when that name is taken. */
export function officialWritingCopyName(typeId: string, attempt: number): string | null {
  const type = officialWritingType(typeId)
  if (!type || attempt < 0 || attempt >= COPY_ATTEMPTS) return null
  const name = attempt === 0 ? `${type.title}.docx` : `${type.title}-${attempt + 1}.docx`
  return workspaceEntryName(name)
}

/** Pack the configured template. A missing model keeps this HTML in the docx. */
export async function officialWritingDocxBytes(html: string, title: string): Promise<Uint8Array> {
  const { createArtifact, createArtifactBlob } = await import('@a3s-lab/office/core')
  const artifact = createArtifact('blank-document')
  artifact.title = title
  artifact.content = { type: 'document', html, pageSize: 'a4' }
  const blob = await createArtifactBlob(artifact)
  return new Uint8Array(await blob.arrayBuffer())
}

/**
 * Write one copy of the configured template at the session workspace root.
 * The file pane labels that root 会话工作区.
 */
export async function placeOfficialWritingCopy(
  root: string,
  request: OfficialWritingCopyRequest
): Promise<{ path: string; name: string } | null> {
  const workspace = root.trim()
  if (!workspace) return null
  const html = officialWritingTemplateHtml(request.typeId, request.brief, request.choices, request.fields)
  const title = officialWritingType(request.typeId)?.title
  if (!html || !title) return null
  const bytes = await officialWritingDocxBytes(html, title)
  for (let attempt = 0; attempt < COPY_ATTEMPTS; attempt += 1) {
    const name = officialWritingCopyName(request.typeId, attempt)
    if (!name) return null
    try {
      const created = await createLocalEntry(workspace, workspace, name, 'file')
      try {
        await writeLocalDocumentBytes(workspace, created.path, bytes)
      } catch (error) {
        await removeLocalEntry(workspace, created.path).catch(() => undefined)
        throw error
      }
      return { path: created.path, name: created.name }
    } catch (error) {
      if (localEntryStatus(error) === 409) continue
      throw error
    }
  }
  return null
}

let current: OfficialWritingWorkspaceFile | null = null
const listeners = new Set<() => void>()

function emitFile() {
  for (const listener of listeners) listener()
}

export function publishOfficialWritingFile(next: OfficialWritingWorkspaceFile | null) {
  if (current?.sessionId === next?.sessionId && current?.path === next?.path && current?.name === next?.name) return
  current = next
  emitFile()
}

export function subscribeOfficialWritingFile(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function readOfficialWritingFile(): OfficialWritingWorkspaceFile | null {
  return current
}

export function resetOfficialWritingFileForTests() {
  current = null
  emitFile()
}

/**
 * The work page already confirmed 交给助手. Create one task, write one docx into its
 * workspace, and open that file there.
 */
export async function openOfficialWritingTask(input: {
  handoff: OfficialWritingHandoff
  create: () => Promise<{ id: string } | null>
  workspacePath: (sessionId: string) => string
  adopt: (sessionId: string) => void
}): Promise<{ sessionId: string; file: { path: string; name: string } | null } | null> {
  const created = await input.create()
  if (!created) return null
  const text = input.handoff.text
  const copy = officialWritingCopyRequest(input.handoff)
  armOfficialWritingDocument(text, created.id)
  let instruction = text
  let file: { path: string; name: string } | null = null
  const root = input.workspacePath(created.id).trim()
  if (copy && root) {
    try {
      const placed = await placeOfficialWritingCopy(root, copy)
      if (placed) {
        file = placed
        publishOfficialWritingFile({ sessionId: created.id, path: placed.path, name: placed.name })
        instruction = officialWritingFileInstruction(copy, placed.name)
      }
    } catch (error) {
      logger.warn('Failed to place the official writing document', error as Error)
    }
  }
  writeAgentDraftCache(getAgentDraftCacheKey(created.id), {
    text: instruction,
    tokens: [],
    files: [],
    knowledgeBaseIds: [],
    workspaceKey: '',
    agentId: ''
  })
  markPendingHomeSubmit(created.id)
  input.adopt(created.id)
  return { sessionId: created.id, file }
}

export function useOfficialWritingFile(): OfficialWritingWorkspaceFile | null {
  return useSyncExternalStore(subscribeOfficialWritingFile, readOfficialWritingFile, readOfficialWritingFile)
}

/** Open the placed copy in the workspace editor after the files pane is up. */
export function OfficialWritingFileOpener({
  sessionId,
  workspacePath,
  openFiles,
  openEditor
}: {
  sessionId?: string
  workspacePath?: string
  openFiles: () => void
  openEditor: (path: string, name: string) => void
}) {
  const file = useOfficialWritingFile()
  const openedPath = useRef<string | null>(null)
  const openFilesRef = useRef(openFiles)
  const openEditorRef = useRef(openEditor)
  openFilesRef.current = openFiles
  openEditorRef.current = openEditor
  useEffect(() => {
    if (!file || !sessionId || file.sessionId !== sessionId) {
      openedPath.current = null
      return
    }
    const target = conversationEditorTarget(workspacePath, file.path)
    if (!target || openedPath.current === target.path) return
    openedPath.current = target.path
    openFilesRef.current()
    openEditorRef.current(target.path, target.name)
  }, [file, sessionId, workspacePath])
  return null
}
