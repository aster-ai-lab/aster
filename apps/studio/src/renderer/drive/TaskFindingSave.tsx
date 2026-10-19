import { Tooltip } from '@a3s-lab/ui'
import { Folder } from 'lucide-react'
import { createContext, use, useState, useSyncExternalStore } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { MessageActionButton } from '@renderer/components/chat/messages/frame/MessageActionButton'
import { PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { readLocalDocumentBytes } from '@renderer/pages/documents/useLocalDocuments'

import { driveApi } from './driveTransport'
import { SaveFindingDialog } from './SaveFindingDialog'

/** Agent session id for a task-workspace turn. Empty means the turn has no label. */
export const TaskFindingSessionContext = createContext<string | undefined>(undefined)

/** Workspace root used to read product files. Empty means the files cannot be read. */
export const TaskWorkspaceRootContext = createContext<string | undefined>(undefined)

export interface TaskProductFile {
  name: string
  path: string
}

/**
 * Confirmed save of one product file into an existing drive folder.
 * A personal center, a live turn, a missing file, and a file already saved stay quiet.
 */
export function TaskFindingSave({
  file,
  peers,
  streaming,
  readFile = readLocalDocumentBytes
}: {
  file: TaskProductFile | null
  peers?: readonly TaskProductFile[]
  streaming: boolean
  readFile?: (root: string, path: string) => Promise<Uint8Array>
}) {
  const { t } = useTranslation()
  const centerId = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const savedIds = useSyncExternalStore(subscribeSaved, savedSnapshot, emptySaved)
  const sessionId = use(TaskFindingSessionContext) ?? ''
  const root = use(TaskWorkspaceRootContext)?.trim() ?? ''
  const [open, setOpen] = useState(false)
  const saved = file ? savedIds.includes(productId(root, file.path)) : false
  const quiet = streaming || centerId === PERSONAL_CENTER || !file || saved
  if (quiet && !open) return null
  if (!file) return null
  const sessionLabel = sessionId.length > 0 && sessionId.length <= 64 ? sessionId : undefined
  const label = t('messages.save_finding')
  const name = uploadName(file, peers ?? [file])

  return (
    <>
      <Tooltip content={label} delay={500}>
        <MessageActionButton
          aria-label={label}
          className="task-product-row__save"
          data-ui="task.save-finding"
          onClick={() => setOpen(true)}>
          <Folder aria-hidden="true" strokeWidth={1.7} />
        </MessageActionButton>
      </Tooltip>
      {createPortal(
        <SaveFindingDialog
          open={open}
          sessionLabel={sessionLabel}
          onSave={async (parentId) => {
            if (!root) throw new Error('read failed')
            const bytes = await readFile(root, readPath(root, file.path))
            await driveApi.upload(new File([bytes], name), parentId, undefined, sessionLabel)
            rememberSaved(root, file.path)
          }}
          onClose={() => setOpen(false)}
        />,
        document.body
      )}
    </>
  )
}

function readPath(root: string, path: string): string {
  if (path.startsWith('/') || path.startsWith('\\') || /^[A-Za-z]:[\\/]/.test(path)) return path
  const separator = root.includes('\\') && !root.includes('/') ? '\\' : '/'
  return `${root.replace(/[\\/]+$/, '')}${separator}${path.replace(/^[\\/]+/, '')}`
}

const SAVED_KEY = 'a3s.task-product-drive.v1'
const SAVED_LIMIT = 500
let savedIds = loadSaved()
const savedListeners = new Set<() => void>()

function loadSaved(): string[] {
  try {
    const raw = JSON.parse(localStorage.getItem(SAVED_KEY) || '[]')
    if (!Array.isArray(raw)) return []
    return raw.filter((item): item is string => typeof item === 'string').slice(-SAVED_LIMIT)
  } catch {
    return []
  }
}

function emptySaved(): string[] {
  return savedIds
}

function savedSnapshot(): string[] {
  return savedIds
}

function subscribeSaved(listener: () => void) {
  savedListeners.add(listener)
  return () => savedListeners.delete(listener)
}

function emitSaved() {
  for (const listener of savedListeners) listener()
}

export function resetSavedTaskProductsForTests() {
  savedIds = []
  localStorage.removeItem(SAVED_KEY)
  emitSaved()
}

function productId(root: string, path: string) {
  return `${root}\n${path}`
}

function rememberSaved(root: string, path: string) {
  const id = productId(root, path)
  if (!root || savedIds.includes(id)) return
  savedIds = [...savedIds, id].slice(-SAVED_LIMIT)
  localStorage.setItem(SAVED_KEY, JSON.stringify(savedIds))
  emitSaved()
}

function uploadName(file: TaskProductFile, peers: readonly TaskProductFile[]): string {
  const duplicated = peers.filter((peer) => peer.name === file.name).length > 1
  if (!duplicated) return file.name
  const parent = file.path.split(/[\\/]/).filter(Boolean).at(-2)
  return parent ? `${parent}-${file.name}` : file.name
}
