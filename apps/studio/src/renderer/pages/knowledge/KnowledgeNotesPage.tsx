import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  EmptyState,
  PopConfirm,
  SegmentedControl
} from '@a3s-lab/ui'
import type { Extensions } from '@tiptap/core'
import {
  ChevronRight,
  FilePlus,
  FileText,
  Folder,
  FolderPlus,
  Library,
  PanelRightOpen,
  Pencil,
  RefreshCw,
  Trash2,
  Waypoints
} from 'lucide-react'
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ButtonHTMLAttributes,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode
} from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import { FileTreeSearchField } from '@renderer/components/FileTreeSearchField'
import { PageEntry } from '@renderer/components/layout/PageEntry'
import { PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { pageSidebarEdgeClassName, pageSidebarWidthClassName } from '@renderer/layout/pageSidebar'
import { decodeUtf8Async } from '@renderer/pages/documents/decodeUtf8'
import { formatDocumentSize } from '@renderer/pages/documents/documentLibrary'
import { TEXT_READ_BYTES, VIRTUAL_TEXT_CHARS, VISUAL_TEXT_UNITS } from '@renderer/pages/documents/fileOpenBudget'
import {
  createLocalEntry,
  FileTooLargeError,
  listLocalEntries,
  localEntryStatus,
  moveLocalEntry,
  openWorkspaceFile,
  readLocalDocumentBytes,
  removeLocalEntry,
  renameLocalEntry,
  writeLocalDocumentBytes
} from '@renderer/pages/documents/useLocalDocuments'
import { WorkspaceCodeEditor } from '@renderer/pages/documents/WorkspaceCodeEditor'
import { displayedWorkspacePath } from '@renderer/pages/settings/GeneralSettings'
import { cn } from '@renderer/utils/style'
import { readStudioHomePath } from '@shared/studioRuntime'

import { cloudKnowledgeBundle, type CloudKnowledgeBundle } from './cloudKnowledgeCatalog'
import { CloudKnowledgeHome } from './CloudKnowledgeHome'
import { type PersonalAcceptCounts } from './compileLocalOkf'
import { noteTags } from './flowixNotes'
import { readHeldNoteDraft, writeHeldNoteDraft } from './heldNoteDraft'
import { installCloudKnowledge, type CloudDownloadProgress } from './installCloudKnowledge'
import { type KnowledgeCompileSession } from './knowledgeCompileAgent'
import { KnowledgeCompileAgentRun } from './KnowledgeCompileAgentRun'
import { useKnowledgeCompileRun } from './knowledgeCompileRun'
import { dispatchKnowledgeCompileTurn, KnowledgeCompileTurn } from './KnowledgeCompileTurn'
import { KnowledgeGraphView } from './KnowledgeGraphView'
import { noteTitle } from './knowledgeLinks'
import { createKnowledgeReveal } from './knowledgeReveal'
import {
  cloudBundlePath,
  cloudVaultPath,
  ensurePersonalVault,
  flattenVaultFiles,
  isPersonalNotePath,
  listVaultTree,
  personalVaultPath,
  recentNotes,
  type VaultFile,
  type VaultTreeNode
} from './knowledgeVault'
import {
  createKnowledgeWikiLink,
  planUnresolvedNote,
  type WikiNoteRef,
  type WikiSuggestState
} from './knowledgeWikiLink'
import {
  indexCoverage,
  NOTE_INDEX_BYTES,
  NOTE_INDEX_LIMIT,
  noteDisplayName,
  noteHeadings,
  noteReading,
  type ReadingNote
} from './noteReading'
import { conceptFileName } from './okfConcept'
import {
  consumePendingPersonalNote,
  clearPendingPersonalNote,
  OPEN_PERSONAL_NOTE_EVENT,
  type OpenPersonalNoteDetail
} from './personalNote'
import {
  bundleRelative,
  conceptDiagnostics,
  conceptDocument,
  conceptOutlinks,
  conceptPassageHits,
  conceptProse,
  compileRunProgress,
  isPersonalVaultDirectory,
  isReservedNoteName,
  isVaultArtifact,
  noteEditorBody,
  noteTypeLine,
  parseIndexCatalog,
  resolveBundleReference,
  treeMatchingPaths,
  visibleNoteTree,
  withNoteType,
  type IndexCatalog
} from './personalOkfView'
import { WikiRetrievalStatus } from './WikiRetrievalStatus'

type LibraryId = 'local' | 'cloud'

function KnowledgeSegmentBar({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div
      role="group"
      aria-label={label}
      className="flex h-[30px] shrink-0 items-center rounded-[8px] bg-[#f5f5f5] p-0.5">
      {children}
    </div>
  )
}

function KnowledgeSegment({
  selected,
  className,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { selected?: boolean }) {
  return (
    <button
      type="button"
      {...props}
      className={cn(
        'inline-flex h-6 items-center rounded-[6px] px-2 text-[12px] whitespace-nowrap disabled:opacity-40',
        selected ? 'bg-white text-[#171717]' : 'text-[#8a8a8a]',
        className
      )}
    />
  )
}

function KnowledgeStats({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-[30px] shrink-0 items-stretch divide-x divide-[#efefef] overflow-hidden rounded-[8px] border border-[#efefef] bg-white">
      {children}
    </div>
  )
}

function KnowledgeMetric({
  phrase,
  label,
  count,
  pressed,
  disabled,
  onClick
}: {
  phrase: string
  label: string
  count: number
  pressed?: boolean
  disabled?: boolean
  onClick?: () => void
}) {
  const body = (
    <>
      <span className="text-[13px] font-medium text-[#171717] tabular-nums">{count}</span>
      <span className={cn('text-[12px]', pressed ? 'text-[#171717]' : 'text-[#8a8a8a]')}>{label}</span>
    </>
  )
  const className = cn(
    'inline-flex items-center gap-1 bg-white px-2.5 whitespace-nowrap',
    onClick && !disabled && !pressed && 'hover:bg-[#f7f7f7]',
    pressed && 'bg-[#f5f5f5]',
    disabled && 'opacity-40'
  )
  if (!onClick) {
    return (
      <span aria-label={phrase} className={className}>
        <span aria-hidden="true" className="text-[13px] font-medium text-[#171717] tabular-nums">
          {count}
        </span>
        <span aria-hidden="true" className={cn('text-[12px]', pressed ? 'text-[#171717]' : 'text-[#8a8a8a]')}>
          {label}
        </span>
        <span className="sr-only">{phrase}</span>
      </span>
    )
  }
  return (
    <button
      type="button"
      aria-label={phrase}
      aria-pressed={pressed ? true : undefined}
      disabled={disabled}
      className={className}
      onClick={onClick}>
      {body}
    </button>
  )
}

function KnowledgeCompileDialog({
  open,
  phase,
  session,
  compiledCount,
  skippedCount,
  failedCount,
  accepted,
  issueFilter,
  message,
  noteTotal,
  startedAt,
  planned,
  pending,
  workspaceRoot,
  vaultRoot,
  onOpenChange,
  onCompile,
  onFilter,
  onSettled,
  onFailed
}: {
  open: boolean
  phase: 'idle' | 'running' | 'accepting' | 'done'
  session: KnowledgeCompileSession | null
  compiledCount: number
  skippedCount: number
  failedCount: number
  accepted: PersonalAcceptCounts | null
  issueFilter: 'skipped' | 'failed' | null
  message: string
  noteTotal: number
  startedAt: number
  planned: boolean
  pending: readonly string[] | null
  workspaceRoot: string
  vaultRoot: string
  onOpenChange: (open: boolean) => void
  onCompile: () => void
  onFilter: (kind: 'skipped' | 'failed') => void
  onSettled: () => void
  onFailed: () => void
}) {
  const { t } = useTranslation()
  const running = phase === 'running'
  const busy = running || phase === 'accepting'
  const counted =
    phase === 'done' && accepted ? accepted : { concepts: compiledCount, skipped: skippedCount, failed: failedCount }
  const hasStats = !busy && (counted.concepts > 0 || counted.skipped > 0 || counted.failed > 0)
  const [runProgress, setRunProgress] = useState<{ done: number; total: number } | null>(null)
  useEffect(() => {
    if (!open || (running && !planned)) setRunProgress(null)
  }, [open, planned, running])
  useEffect(() => {
    if (!running || !planned || !workspaceRoot || !startedAt) return
    let cancelled = false
    const pendingNames = pending ? new Set(pending.map((path) => conceptFileName(path))) : null
    const tick = async () => {
      try {
        let conceptModifiedMs: number[] = []
        let pendingModifiedMs: number[] = []
        try {
          const concepts = await listLocalEntries(workspaceRoot, `${vaultRoot}/concepts`)
          for (const entry of concepts.entries) {
            if (entry.kind !== 'file' || !/\.md$/i.test(entry.name)) continue
            const modifiedMs = entry.modifiedMs ?? 0
            conceptModifiedMs.push(modifiedMs)
            if (pendingNames?.has(entry.name)) pendingModifiedMs.push(modifiedMs)
          }
        } catch (error) {
          if (localEntryStatus(error) !== 404) throw error
        }
        if (!cancelled) {
          setRunProgress(
            compileRunProgress({
              noteTotal,
              startedAt,
              conceptModifiedMs,
              pendingTotal: pending ? pending.length : null,
              pendingModifiedMs: pending ? pendingModifiedMs : null
            })
          )
        }
      } catch {
        // A missed poll keeps the last progress. The next tick tries again.
      }
    }
    void tick()
    const timer = setInterval(() => void tick(), 1000)
    return () => {
      cancelled = true
      clearInterval(timer)
    }
  }, [noteTotal, pending, planned, running, startedAt, vaultRoot, workspaceRoot])
  const progress = busy ? runProgress : null
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent size="lg" showCloseButton={false} className="max-h-[70vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="text-[15px] font-medium">{t('knowledge.hub.compile_action')}</DialogTitle>
          <DialogDescription className="text-[13px] text-[#8a8a8a]">
            {phase === 'accepting' ? t('knowledge.notes.compile_accepting') : t('knowledge.notes.compile_dialog_body')}
          </DialogDescription>
        </DialogHeader>
        {progress && progress.total > 0 ? (
          <div className="flex flex-col gap-1.5">
            <p className="m-0 flex items-center justify-between text-[13px]">
              <span className="text-[#8a8a8a]">{t('knowledge.notes.compile_progress')}</span>
              <span className="text-[#171717] tabular-nums">
                {progress.done} / {progress.total}
              </span>
            </p>
            <div
              role="progressbar"
              aria-valuemin={0}
              aria-valuemax={progress.total}
              aria-valuenow={progress.done}
              aria-label={t('knowledge.notes.compile_progress')}
              className="h-1 overflow-hidden rounded-full bg-[#f0f0f0]">
              <div
                className="h-full bg-[#171717]"
                style={{ width: `${Math.round((progress.done / progress.total) * 100)}%` }}
              />
            </div>
          </div>
        ) : null}
        {hasStats ? (
          <KnowledgeStats>
            {counted.concepts > 0 ? (
              <KnowledgeMetric
                count={counted.concepts}
                label={t('knowledge.notes.compile_compiled_short')}
                phrase={t('knowledge.notes.compile_compiled', { count: counted.concepts })}
              />
            ) : null}
            {counted.skipped > 0 ? (
              <KnowledgeMetric
                count={counted.skipped}
                label={t('knowledge.notes.compile_skipped_short')}
                phrase={t('knowledge.notes.compile_skipped', { count: counted.skipped })}
                pressed={issueFilter === 'skipped'}
                onClick={busy ? undefined : () => onFilter('skipped')}
              />
            ) : null}
            {counted.failed > 0 ? (
              <KnowledgeMetric
                count={counted.failed}
                label={t('knowledge.notes.compile_failed_short')}
                phrase={t('knowledge.notes.compile_failed', { count: counted.failed })}
                pressed={issueFilter === 'failed'}
                onClick={busy ? undefined : () => onFilter('failed')}
              />
            ) : null}
          </KnowledgeStats>
        ) : null}
        {session ? (
          <KnowledgeCompileAgentRun
            sessionId={session.sessionId}
            agentId={session.agentId}
            onSettled={onSettled}
            onFailed={onFailed}
          />
        ) : null}
        {message ? (
          <p role="alert" className="m-0 text-[13px] text-[#171717]">
            {message}
          </p>
        ) : null}
        {busy ? null : (
          <DialogFooter className="flex-row justify-end gap-2">
            {phase === 'done' ? (
              <Button
                type="button"
                variant="emphasis"
                size="md"
                className="rounded-[8px]"
                onClick={() => onOpenChange(false)}>
                {t('common.close')}
              </Button>
            ) : (
              <>
                <Button
                  type="button"
                  variant="outline"
                  size="md"
                  className="rounded-[8px]"
                  onClick={() => onOpenChange(false)}>
                  {t('common.cancel')}
                </Button>
                <Button type="button" variant="emphasis" size="md" className="rounded-[8px]" onClick={onCompile}>
                  {t('knowledge.notes.compile_start')}
                </Button>
              </>
            )}
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  )
}

interface LoadedNote {
  id: string
  name: string
  path: string
  markdown: string | null
  unavailable: boolean
}

function listedNote(file: VaultFile): LoadedNote {
  return { id: file.path, name: file.name, path: file.path, markdown: null, unavailable: false }
}

function filterVault(
  nodes: readonly VaultTreeNode[],
  needle: string,
  names: ReadonlyMap<string, string>
): VaultTreeNode[] {
  if (!needle) return [...nodes]
  return nodes.flatMap((node) => {
    if (node.kind === 'file') {
      const title = (names.get(node.path) ?? noteTitle(node.name)).toLowerCase()
      return title.includes(needle) || node.name.toLowerCase().includes(needle) ? [node] : []
    }
    const children = filterVault(node.children, needle, names)
    if (children.length === 0) return []
    return [{ ...node, children }]
  })
}

interface VaultMenuTarget {
  path: string
  kind: 'root' | 'directory' | 'file'
  x: number
  y: number
}

interface VaultDraftBase {
  name: string
  error: string
  busy: boolean
  touched: boolean
}

type VaultDraft =
  | (VaultDraftBase & { mode: 'create'; kind: 'file' | 'folder'; parent: string })
  | (VaultDraftBase & { mode: 'rename'; path: string })

function parentDir(path: string): string {
  const index = path.lastIndexOf('/')
  return index > 0 ? path.slice(0, index) : path
}

function siblingNames(nodes: readonly VaultTreeNode[], parent: string, rootPath: string): string[] {
  if (parent === rootPath) return nodes.map((node) => node.name)
  const stack = [...nodes]
  while (stack.length > 0) {
    const node = stack.pop()
    if (!node || node.kind !== 'directory') continue
    if (node.path === parent) return node.children.map((child) => child.name)
    stack.push(...node.children)
  }
  return []
}

function nextName(siblings: readonly string[], base: string): string {
  if (!siblings.includes(base)) return base
  const dot = base.lastIndexOf('.')
  const stem = dot > 0 ? base.slice(0, dot) : base
  const extension = dot > 0 ? base.slice(dot) : ''
  for (let index = 2; index < 100; index += 1) {
    const candidate = `${stem} ${index}${extension}`
    if (!siblings.includes(candidate)) return candidate
  }
  return `${stem} ${siblings.length + 1}${extension}`
}

const NOTE_TITLE_LIMIT = 80

function titleChars(value: string): string[] {
  return Array.from(value.replace(/[\r\n]/g, ''))
}

function yamlQuote(value: string): string {
  return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
}

function yamlUnquote(value: string): string {
  const trimmed = value.trim()
  if (trimmed.length >= 2 && trimmed.startsWith('"') && trimmed.endsWith('"')) {
    return trimmed.slice(1, -1).replace(/\\"/g, '"').replace(/\\\\/g, '\\')
  }
  if (trimmed.length >= 2 && trimmed.startsWith("'") && trimmed.endsWith("'")) {
    return trimmed.slice(1, -1).replace(/''/g, "'")
  }
  return trimmed
}

function frontmatterBlock(markdown: string): { body: string; lines: string[] } | null {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/.exec(markdown)
  if (!match) return null
  return { body: markdown.slice(match[0].length), lines: match[1].split(/\r?\n/) }
}

function readStoredTitle(markdown: string | null): string | null {
  if (!markdown) return null
  const block = frontmatterBlock(markdown)
  if (!block) return null
  for (const line of block.lines) {
    const match = /^title:\s*(.*)$/.exec(line)
    if (!match) continue
    const value = yamlUnquote(match[1])
    return value.length > 0 ? value : null
  }
  return null
}

function markdownBody(markdown: string): string {
  return noteEditorBody(markdown)
}

function markdownWithTitle(markdown: string, title: string | null, stem: string): string {
  const block = frontmatterBlock(markdown)
  const keep = Boolean(title && title !== stem)
  if (!block) {
    if (!keep || !title) return markdown
    return `---\ntitle: ${yamlQuote(title)}\n---\n${markdown}`
  }
  const lines = block.lines.filter((line) => !/^title:\s*/.test(line))
  if (keep && title) lines.unshift(`title: ${yamlQuote(title)}`)
  if (lines.every((line) => line.trim() === '')) return block.body
  return `---\n${lines.join('\n')}\n---\n${block.body}`
}

function composeNote(body: string, title: string | null, stem: string, typeLine: string | null = null): string {
  const titled = frontmatterBlock(body)
    ? markdownWithTitle(body, title, stem)
    : !title || title === stem
      ? body
      : `---\ntitle: ${yamlQuote(title)}\n---\n${body}`
  return withNoteType(titled, typeLine)
}

function markdownFileName(raw: string): { name: string } | { error: 'empty' | 'invalid' | 'type' } {
  const name = raw.trim()
  if (!name || name === '.' || name === '..') return { error: 'empty' }
  if (name.includes('/') || name.includes('\\') || name.startsWith('.')) return { error: 'invalid' }
  if (/\.(?:md|markdown)$/i.test(name)) return { name }
  if (/\.[^./\\]+$/.test(name)) return { error: 'type' }
  return { name: `${name}.md` }
}

function findVaultNode(nodes: readonly VaultTreeNode[], path: string): VaultTreeNode | null {
  const stack = [...nodes]
  while (stack.length > 0) {
    const node = stack.pop()
    if (!node) continue
    if (node.path === path) return node
    if (node.kind === 'directory') stack.push(...node.children)
  }
  return null
}

function canRenameVaultPath(nodes: readonly VaultTreeNode[], path: string): boolean {
  const node = findVaultNode(nodes, path)
  if (!node) return false
  if (node.kind === 'directory') return true
  return node.kind === 'file' && !isReservedNoteName(node.name)
}

function VaultFileTree({
  rootName,
  rootPath,
  nodes,
  names,
  query,
  openedId,
  draft,
  reloading,
  onOpenFile,
  onCreate,
  onRename,
  onReload,
  onMove,
  onMenu,
  onDraftName,
  onDraftCommit,
  onDraftCancel
}: {
  rootName: string
  rootPath: string
  nodes: readonly VaultTreeNode[]
  names: ReadonlyMap<string, string>
  query: string
  openedId: string | null
  draft: VaultDraft | null
  reloading: boolean
  onOpenFile: (path: string) => void
  onCreate: (kind: 'file' | 'folder', parent: string) => void
  onRename: (path: string) => void
  onReload: () => void
  onMove: (source: string, destination: string) => void
  onMenu: (target: VaultMenuTarget) => void
  onDraftName: (name: string) => void
  onDraftCommit: (shown?: string) => void
  onDraftCancel: () => void
}) {
  const { t } = useTranslation()
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => new Set())
  const [dropTarget, setDropTarget] = useState<string | null>(null)
  const [ghost, setGhost] = useState<{ label: string; x: number; y: number } | null>(null)
  const submitLock = useRef(false)
  const skipClick = useRef(false)
  const inlineOpenedAt = useRef(0)
  const armedDraft = useRef('')
  const needle = query.trim().toLowerCase()
  const visible = filterVault(nodes, needle, names)
  const fileLabel = (node: VaultTreeNode) => names.get(node.path) ?? noteTitle(node.name)
  const draftParent = draft?.mode === 'create' ? draft.parent : draft ? parentDir(draft.path) : ''
  const draftKey = draft?.mode === 'rename' ? draft.path : draft ? `${draft.parent}:${draft.kind}` : ''
  if (draftKey !== armedDraft.current) {
    armedDraft.current = draftKey
    if (draftKey) inlineOpenedAt.current = Date.now()
  }

  const draggedClick = () => {
    if (!skipClick.current) return false
    skipClick.current = false
    return true
  }

  const onMoveRef = useRef(onMove)
  onMoveRef.current = onMove
  const dragStop = useRef<(() => void) | null>(null)

  const destinationAt = (x: number, y: number) => {
    const hit = document.elementFromPoint(x, y)
    if (!(hit instanceof Element)) return null
    return hit.closest('[data-vault-drop]')?.getAttribute('data-vault-drop') || null
  }

  const beginPointerDrag = (event: ReactPointerEvent<HTMLElement>, path: string, label: string) => {
    if (event.button !== 0 || event.detail >= 2) return
    dragStop.current?.()
    const pointerId = event.pointerId
    const startX = event.clientX
    const startY = event.clientY
    let active = false
    const move = (native: PointerEvent) => {
      if (native.pointerId !== pointerId) return
      const dx = native.clientX - startX
      const dy = native.clientY - startY
      if (!active && dx * dx + dy * dy < 16) return
      active = true
      document.body.style.cursor = 'grabbing'
      const destination = destinationAt(native.clientX, native.clientY)
      setDropTarget((current) => (current === destination ? current : destination))
      setGhost({ label, x: native.clientX, y: native.clientY })
    }
    const end = (native: PointerEvent) => {
      if (native.pointerId !== pointerId) return
      stop()
      document.body.style.cursor = ''
      setGhost(null)
      if (!active) {
        setDropTarget(null)
        return
      }
      skipClick.current = true
      const swallow = (click: globalThis.MouseEvent) => {
        click.preventDefault()
        click.stopPropagation()
        window.removeEventListener('click', swallow, true)
      }
      window.addEventListener('click', swallow, true)
      window.setTimeout(() => {
        skipClick.current = false
        window.removeEventListener('click', swallow, true)
      }, 0)
      const destination = destinationAt(native.clientX, native.clientY)
      setDropTarget(null)
      if (!destination) return
      setCollapsed((current) => {
        if (!current.has(rootPath) && (destination === rootPath || !current.has(destination))) return current
        const next = new Set(current)
        next.delete(rootPath)
        next.delete(destination)
        return next
      })
      onMoveRef.current(path, destination)
    }
    const stop = () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', end)
      window.removeEventListener('pointercancel', end)
      if (dragStop.current === stop) dragStop.current = null
    }
    dragStop.current = stop
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', end)
    window.addEventListener('pointercancel', end)
  }

  useEffect(() => () => dragStop.current?.(), [])

  useEffect(() => {
    if (!draftParent) return
    setCollapsed((current) => {
      const next = new Set(current)
      let cursor = draftParent
      next.delete(rootPath)
      next.delete(cursor)
      while (cursor.length > rootPath.length) {
        const slash = cursor.lastIndexOf('/')
        if (slash <= 0) break
        cursor = cursor.slice(0, slash)
        next.delete(cursor)
      }
      return next
    })
  }, [draftParent, rootPath])

  const toggle = (path: string) => {
    setCollapsed((current) => {
      const next = new Set(current)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const openMenu = (event: MouseEvent, path: string, kind: VaultMenuTarget['kind']) => {
    event.preventDefault()
    event.stopPropagation()
    onMenu({ path, kind, x: event.clientX, y: event.clientY })
  }

  const openMenuFromKeyboard = (
    event: ReactKeyboardEvent<HTMLButtonElement>,
    path: string,
    kind: VaultMenuTarget['kind']
  ) => {
    if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return
    event.preventDefault()
    event.stopPropagation()
    const rect = event.currentTarget.getBoundingClientRect()
    onMenu({ path, kind, x: rect.left, y: rect.bottom })
  }

  const renderDraft = (depth: number, folder: boolean) => (
    <form
      key={draft?.mode === 'rename' ? draft.path : `create:${draft?.parent}:${draft?.kind}`}
      className={rowClass(true)}
      style={{ paddingLeft: depth * 16 }}
      onSubmit={(event) => {
        event.preventDefault()
        submitLock.current = true
        const field = event.currentTarget.querySelector('input')
        onDraftCommit(field instanceof HTMLInputElement ? field.value : undefined)
      }}
      onBlur={(event) => {
        const next = event.relatedTarget
        if (next instanceof Node && event.currentTarget.contains(next)) return
        if (submitLock.current) {
          submitLock.current = false
          return
        }
        // A double click focuses this field and then the same gesture blurs it.
        if (Date.now() - inlineOpenedAt.current < 400) {
          const field = event.currentTarget.querySelector('input')
          requestAnimationFrame(() => {
            if (field && document.contains(field) && document.activeElement !== field) field.focus()
          })
          return
        }
        if (!draft?.touched) onDraftCancel()
        else {
          const field = event.currentTarget.querySelector('input')
          onDraftCommit(field instanceof HTMLInputElement ? field.value : undefined)
        }
      }}>
      {folder ? (
        <Folder className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
      ) : (
        <FileText className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
      )}
      <input
        aria-label={t('knowledge.notes.name')}
        disabled={draft?.busy}
        spellCheck={false}
        value={draft?.name ?? ''}
        className="min-w-0 flex-1 bg-transparent text-[13px] text-[#171717] outline-none"
        onChange={(event) => onDraftName(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key !== 'Escape') return
          event.preventDefault()
          event.stopPropagation()
          submitLock.current = true
          onDraftCancel()
        }}
        ref={(node) => {
          if (!node || node.dataset.armed === 'true') return
          node.dataset.armed = 'true'
          node.focus()
          const dot = node.value.lastIndexOf('.')
          if (dot > 0) node.setSelectionRange(0, dot)
          else node.select()
        }}
      />
    </form>
  )

  const renderNodes = (items: readonly VaultTreeNode[], depth: number, parentPath: string) => (
    <>
      {draft?.mode === 'create' && draft.parent === parentPath ? renderDraft(depth, draft.kind === 'folder') : null}
      {draft?.mode === 'create' && draft.parent === parentPath && draft.error ? (
        <p className="m-0 px-2.5 py-1 text-[12px] text-[#8a8a8a]" role="alert">
          {draft.error}
        </p>
      ) : null}
      {items.map((node) => {
        const pad = { paddingLeft: depth * 16 }
        const renaming = draft?.mode === 'rename' && draft.path === node.path ? draft : null
        if (node.kind === 'directory') {
          const open = needle.length > 0 || !collapsed.has(node.path)
          return (
            <div key={node.path} data-vault-drop={node.path}>
              {renaming ? (
                renderDraft(depth, true)
              ) : (
                <button
                  type="button"
                  aria-expanded={open}
                  onPointerDown={(event) => beginPointerDrag(event, node.path, node.name)}
                  onClick={(event) => {
                    if (draggedClick() || event.detail > 1) return
                    toggle(node.path)
                  }}
                  onContextMenu={(event) => openMenu(event, node.path, 'directory')}
                  onKeyDown={(event) => openMenuFromKeyboard(event, node.path, 'directory')}
                  className={cn(rowClass(false), dropTarget === node.path && 'ring-2 ring-black/10 ring-inset')}
                  style={pad}>
                  <ChevronRight
                    className={cn('size-3.5 shrink-0 text-current', open && 'rotate-90')}
                    strokeWidth={1.7}
                    aria-hidden
                  />
                  <Folder className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                  <span className="truncate">{node.name}</span>
                </button>
              )}
              {renaming?.error ? (
                <p className="m-0 px-2.5 py-1 text-[12px] text-[#8a8a8a]" role="alert">
                  {renaming.error}
                </p>
              ) : null}
              {open ? renderNodes(node.children, depth + 1, node.path) : null}
            </div>
          )
        }
        const selected = node.path === openedId
        return (
          <div key={node.path}>
            {renaming ? (
              renderDraft(depth, false)
            ) : (
              <button
                type="button"
                aria-current={selected ? 'page' : undefined}
                onPointerDown={(event) => beginPointerDrag(event, node.path, fileLabel(node))}
                onClick={(event) => {
                  if (draggedClick() || event.detail > 1) return
                  onOpenFile(node.path)
                }}
                onContextMenu={(event) => openMenu(event, node.path, 'file')}
                onKeyDown={(event) => openMenuFromKeyboard(event, node.path, 'file')}
                className={rowClass(selected)}
                style={pad}>
                <span className="size-3.5 shrink-0" aria-hidden />
                <FileText className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                <span className="truncate">{fileLabel(node)}</span>
              </button>
            )}
            {renaming?.error ? (
              <p className="m-0 px-2.5 py-1 text-[12px] text-[#8a8a8a]" role="alert">
                {renaming.error}
              </p>
            ) : null}
          </div>
        )
      })}
    </>
  )

  return (
    <div role="tree" data-vault-drop={rootPath} className="flex min-h-0 flex-1 flex-col gap-0.5">
      <div className="sticky top-0 z-10 bg-white">
        <div className="group relative">
          <button
            type="button"
            onClick={() => {
              if (draggedClick()) return
            }}
            onContextMenu={(event) => openMenu(event, rootPath, 'root')}
            onKeyDown={(event) => openMenuFromKeyboard(event, rootPath, 'root')}
            className={cn(rowClass(false), 'pr-24', dropTarget === rootPath && 'ring-2 ring-black/10 ring-inset')}>
            <Library className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
            <span className="truncate">{rootName}</span>
          </button>
          <div className="absolute top-1/2 right-1 flex -translate-y-1/2 items-center gap-0.5">
            <button
              type="button"
              aria-label={t('knowledge.notes.reload')}
              aria-busy={reloading || undefined}
              className="grid size-7 place-items-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#ececec] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10"
              onClick={(event) => {
                event.stopPropagation()
                onReload()
              }}>
              <RefreshCw
                className={cn('size-3.5', reloading && 'animate-spin')}
                style={reloading ? { animationDuration: '0.7s' } : undefined}
                strokeWidth={1.7}
                aria-hidden
              />
            </button>
            <button
              type="button"
              aria-label={t('knowledge.notes.new_file')}
              className="grid size-7 place-items-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#ececec] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10"
              onClick={(event) => {
                event.stopPropagation()
                onCreate('file', rootPath)
              }}>
              <FilePlus className="size-3.5" strokeWidth={1.7} aria-hidden />
            </button>
            <button
              type="button"
              aria-label={t('knowledge.notes.new_folder')}
              className="grid size-7 place-items-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#ececec] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10"
              onClick={(event) => {
                event.stopPropagation()
                onCreate('folder', rootPath)
              }}>
              <FolderPlus className="size-3.5" strokeWidth={1.7} aria-hidden />
            </button>
          </div>
        </div>
      </div>
      {renderNodes(visible, 1, rootPath)}
      {visible.length === 0 && draftParent !== rootPath ? (
        <p className="m-0 py-2 pr-2.5 pl-8 text-[13px] text-[#8a8a8a]">{t('knowledge.notes.empty')}</p>
      ) : null}
      {ghost
        ? createPortal(
            <div
              className="pointer-events-none fixed z-50 flex h-7 max-w-48 items-center rounded-[8px] border border-[#ececec] bg-white px-2 text-[13px] text-[#171717] shadow-[0_8px_18px_rgb(0_0_0/0.08)]"
              style={{ left: ghost.x + 12, top: ghost.y + 12 }}>
              <span className="truncate">{ghost.label}</span>
            </div>,
            document.body
          )
        : null}
    </div>
  )
}

function VaultMenuButton({ icon, label, onClick }: { icon: ReactNode; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      role="menuitem"
      className="flex h-8 w-full items-center gap-2 rounded-[8px] px-2 text-left text-[13px] text-[#171717] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10"
      onClick={onClick}>
      <span className="grid size-4 shrink-0 place-items-center text-[#3f3f3f]" aria-hidden>
        {icon}
      </span>
      <span className="truncate">{label}</span>
    </button>
  )
}

function rowClass(selected: boolean) {
  return cn(
    'flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] outline-none',
    'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
    selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
  )
}

function formatSavedClock(date: Date): string {
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
}

function NoteTitleField({
  title,
  editable,
  savedAt,
  unsaved,
  onCommit
}: {
  title: string
  editable: boolean
  savedAt: Date | null
  unsaved?: boolean
  onCommit: (raw: string) => Promise<string>
}) {
  const { t } = useTranslation()
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(title)
  const [error, setError] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const committingRef = useRef(false)
  const skipBlurRef = useRef(false)

  useEffect(() => {
    if (!editing) setDraft(title)
  }, [editing, title])

  useEffect(() => {
    if (!editing) return
    inputRef.current?.focus()
    inputRef.current?.select()
  }, [editing])

  const begin = () => {
    if (!editable) return
    setDraft(title)
    setError('')
    setEditing(true)
  }

  const commit = async () => {
    if (committingRef.current) return
    committingRef.current = true
    try {
      const message = await onCommit(draft)
      if (message) {
        setError(message)
        return
      }
      setError('')
      setEditing(false)
    } finally {
      committingRef.current = false
    }
  }

  const count = titleChars(draft).length

  return (
    <>
      <div className="mb-2 flex h-[30px] shrink-0 items-center">
        {editing ? (
          <form
            className="flex h-full min-w-0 flex-1 items-center gap-2"
            onSubmit={(event) => {
              event.preventDefault()
              void commit()
            }}>
            <input
              ref={inputRef}
              aria-label={t('knowledge.notes.title_field')}
              aria-invalid={error ? true : undefined}
              value={draft}
              spellCheck={false}
              className="min-w-0 flex-1 bg-transparent px-1.5 text-[16px] leading-none font-semibold tracking-[-0.02em] text-[#171717] outline-none"
              onChange={(event) => setDraft(titleChars(event.currentTarget.value).slice(0, NOTE_TITLE_LIMIT).join(''))}
              onBlur={() => {
                if (skipBlurRef.current) {
                  skipBlurRef.current = false
                  return
                }
                void commit()
              }}
              onKeyDown={(event) => {
                if (event.key !== 'Escape') return
                event.preventDefault()
                skipBlurRef.current = true
                setError('')
                setEditing(false)
              }}
            />
            <span className="shrink-0 pr-1 text-[12px] text-[#8a8a8a] tabular-nums" aria-hidden>
              {count}/{NOTE_TITLE_LIMIT}
            </span>
          </form>
        ) : (
          <h2
            className="m-0 min-w-0 flex-1 cursor-text truncate px-1.5 text-[16px] leading-none font-semibold tracking-[-0.02em] text-[#171717] outline-none"
            tabIndex={editable ? 0 : undefined}
            onClick={begin}
            onKeyDown={(event) => {
              if (!editable || (event.key !== 'Enter' && event.key !== ' ')) return
              event.preventDefault()
              begin()
            }}>
            {title}
          </h2>
        )}
        {unsaved ? (
          <span
            role="status"
            aria-label={t('knowledge.notes.unsaved')}
            className="shrink-0 pr-1 text-[12px] leading-none text-[#8a8a8a]">
            {t('knowledge.notes.unsaved')}
          </span>
        ) : savedAt ? (
          <time
            dateTime={savedAt.toISOString()}
            className="shrink-0 pr-1 text-[12px] leading-none text-[#8a8a8a] tabular-nums">
            {t('knowledge.notes.saved_at', { time: formatSavedClock(savedAt) })}
          </time>
        ) : null}
      </div>
      {error ? (
        <p className="-mt-1 mb-2 px-1.5 text-[12px] text-[#8a8a8a]" role="alert">
          {error}
        </p>
      ) : null}
    </>
  )
}

function NoteEditor({
  fileName,
  markdown,
  onDraft,
  onReady,
  onSave,
  valueReader,
  extensions,
  preview = false
}: {
  fileName: string
  markdown: string
  onDraft: (markdown: string) => void
  onReady: () => void
  onSave?: (markdown: string) => Promise<void>
  valueReader?: { current: (() => string) | null }
  extensions?: Extensions
  preview?: boolean
}) {
  const source = !preview && (markdown.length > VISUAL_TEXT_UNITS || markdown.length > VIRTUAL_TEXT_CHARS)
  const { t } = useTranslation()
  const content = useMemo(() => ({ type: 'markdown' as const, markdown }), [markdown])
  const [editors, setEditors] = useState<typeof import('@a3s-lab/office/react') | null>(null)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    if (!source) return undefined
    onReady()
    return undefined
  }, [onReady, source])

  useEffect(() => {
    if (source) return undefined
    let cancelled = false
    void import('@a3s-lab/office/styles.css').catch(() => undefined)
    void import('@a3s-lab/office/react')
      .then((mod) => {
        if (!cancelled) setEditors(mod)
      })
      .catch(() => {
        if (!cancelled) {
          setFailed(true)
          onReady()
        }
      })
    return () => {
      cancelled = true
    }
  }, [source])

  if (source && onSave) {
    return (
      <div className="flex min-h-0 w-full min-w-0 flex-1 flex-col">
        <WorkspaceCodeEditor
          fileName={fileName}
          value={markdown}
          onChange={onDraft}
          onSave={(next) => void onSave(next)}
          valueReader={valueReader}
        />
      </div>
    )
  }

  if (failed) {
    return (
      <div role="status" className="mt-8">
        <h3 className="m-0 text-[16px] font-semibold text-[#1a1a1a]">{t('knowledge.notes.unavailable.title')}</h3>
        <p className="m-0 mt-2 text-[14px] text-[#8a8a8a]">{t('knowledge.notes.unavailable.description')}</p>
      </div>
    )
  }
  if (!editors) return <div className="min-h-0 flex-1" />
  const Editor = editors.MarkdownEditor
  return (
    <div className="flex min-h-0 w-full min-w-0 flex-1 flex-col">
      <Editor
        content={content}
        preview={preview}
        presentation="note"
        theme="light"
        extensions={preview ? undefined : extensions}
        onChange={(next) => {
          if (preview || next.markdown === markdown) return
          onDraft(next.markdown)
        }}
        onReady={onReady}
        onSave={onSave ? (next) => onSave(next) : undefined}
      />
    </div>
  )
}

const SKIP_REASONS: Record<string, string> = {
  empty: 'knowledge.notes.skip_empty',
  unsupported: 'knowledge.notes.skip_unsupported',
  unparsed: 'knowledge.notes.skip_unparsed',
  over_depth: 'knowledge.notes.skip_over_depth',
  over_count: 'knowledge.notes.skip_over_count',
  over_size: 'knowledge.notes.skip_over_size'
}

function ReadingGroup({
  label,
  count,
  open = true,
  onToggle,
  children
}: {
  label: string
  count?: number
  open?: boolean
  onToggle?: () => void
  children: ReactNode
}) {
  return (
    <section className="mt-4" aria-label={label}>
      <h2 className="m-0">
        <button
          type="button"
          aria-expanded={open}
          aria-label={label}
          className="flex h-7 w-full items-center gap-2 rounded-[8px] px-2.5 text-left text-[12px] font-medium text-[#8a8a8a] outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10"
          onClick={onToggle}>
          <span className="truncate">{label}</span>
          {count != null ? (
            <span className="shrink-0 tabular-nums" aria-hidden>
              {count}
            </span>
          ) : null}
        </button>
      </h2>
      {open ? children : null}
    </section>
  )
}

function NoteReadingRail({
  notes,
  openedName,
  markdown,
  draft,
  dirty,
  tag,
  onOpen,
  onReveal,
  onTag,
  onCreate
}: {
  notes: readonly ReadingNote[]
  openedName: string
  markdown: string
  draft: string
  dirty: boolean
  tag: string | null
  onOpen: (path: string) => void
  onReveal: (text: string) => void
  onTag: (tag: string) => void
  onCreate: (target: string) => void | Promise<void>
}) {
  const { t } = useTranslation()
  const reading = noteReading(notes, openedName, markdown)
  const headings = noteHeadings(draft)
  const coverage = indexCoverage(notes)
  const complete = coverage.read === coverage.listed
  const [openGroups, setOpenGroups] = useState<ReadonlySet<string>>(() => new Set(['outline']))
  const toggleGroup = (id: string) => {
    setOpenGroups((current) => {
      const next = new Set(current)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }
  const hasItems =
    headings.length +
      reading.outgoing.length +
      reading.unresolved.length +
      reading.backlinks.length +
      reading.tags.length +
      reading.properties.length +
      reading.todos.length >
    0
  return (
    <aside
      data-ui="knowledge.reading"
      aria-label={t('knowledge.notes.reading')}
      className={cn(
        'flex h-full min-h-0 flex-col overflow-y-auto border-l border-[#efefef] bg-white px-3 py-3',
        pageSidebarWidthClassName
      )}>
      {complete ? null : (
        <p className="m-0 px-2.5 text-[12px] text-[#8a8a8a]">
          {t('knowledge.notes.index_read', { read: coverage.read, listed: coverage.listed })}
        </p>
      )}
      {dirty ? (
        <>
          <p className="m-0 mt-1 px-2.5 text-[12px] text-[#8a8a8a]">{t('knowledge.notes.saved_basis_outline')}</p>
          <p className="m-0 mt-1 px-2.5 text-[12px] text-[#8a8a8a]">{t('knowledge.notes.saved_basis_saved')}</p>
        </>
      ) : null}
      {headings.length > 0 ? (
        <ReadingGroup
          label={t('knowledge.notes.outline')}
          count={headings.length}
          open={openGroups.has('outline')}
          onToggle={() => toggleGroup('outline')}>
          {headings.map((heading) => (
            <button
              key={`${heading.level}:${heading.text}`}
              type="button"
              className={rowClass(false)}
              style={{ paddingLeft: 10 + (heading.level - 1) * 12 }}
              onClick={() => onReveal(heading.text)}>
              <span className="truncate">{heading.text}</span>
            </button>
          ))}
        </ReadingGroup>
      ) : null}
      {reading.outgoing.length > 0 ? (
        <ReadingGroup
          label={t('knowledge.notes.links')}
          count={reading.outgoing.length}
          open={openGroups.has('links')}
          onToggle={() => toggleGroup('links')}>
          {reading.outgoing.map((target) => (
            <button key={target.path} type="button" className={rowClass(false)} onClick={() => onOpen(target.path)}>
              <span className="truncate">{target.label}</span>
            </button>
          ))}
        </ReadingGroup>
      ) : null}
      {reading.unresolved.length > 0 ? (
        <ReadingGroup
          label={t('knowledge.notes.unresolved')}
          count={reading.unresolved.length}
          open={openGroups.has('unresolved')}
          onToggle={() => toggleGroup('unresolved')}>
          {reading.unresolved.map((item) => (
            <div key={item.target} className="flex min-w-0 items-center gap-1">
              <p className="m-0 min-w-0 flex-1 truncate px-2.5 py-1 text-[13px] text-[#8a8a8a]">{item.label}</p>
              <PopConfirm
                tone="default"
                title={t('knowledge.notes.create_unresolved_title')}
                description={item.target}
                confirmLabel={t('knowledge.notes.create_unresolved')}
                cancelLabel={t('common.cancel')}
                onConfirm={() => onCreate(item.target)}>
                <button type="button" className="h-7 shrink-0 rounded-[8px] px-2 text-[12px] text-[#171717]">
                  {t('knowledge.notes.create_short')}
                </button>
              </PopConfirm>
            </div>
          ))}
        </ReadingGroup>
      ) : null}
      {reading.backlinks.length > 0 ? (
        <ReadingGroup
          label={t('knowledge.notes.backlinks')}
          count={reading.backlinks.length}
          open={openGroups.has('backlinks')}
          onToggle={() => toggleGroup('backlinks')}>
          {reading.backlinks.map((target) => (
            <button
              key={target.path}
              type="button"
              aria-label={target.label}
              className={rowClass(false)}
              onClick={() => onOpen(target.path)}>
              <span className="min-w-0">
                <span className="block truncate">{target.label}</span>
                {target.line ? <span className="block truncate text-[12px] text-[#8a8a8a]">{target.line}</span> : null}
              </span>
            </button>
          ))}
        </ReadingGroup>
      ) : null}
      {reading.tags.length > 0 ? (
        <ReadingGroup
          label={t('knowledge.flowix.tags')}
          count={reading.tags.length}
          open={openGroups.has('tags')}
          onToggle={() => toggleGroup('tags')}>
          {reading.tags.map((item) => (
            <button
              key={item}
              type="button"
              aria-pressed={tag === item}
              className={rowClass(tag === item)}
              onClick={() => onTag(item)}>
              <span className="truncate">{item}</span>
            </button>
          ))}
        </ReadingGroup>
      ) : null}
      {reading.todos.length > 0 ? (
        <ReadingGroup
          label={t('knowledge.flowix.todos')}
          count={reading.todos.length}
          open={openGroups.has('todos')}
          onToggle={() => toggleGroup('todos')}>
          {reading.todos.map((line) => (
            <button key={line} type="button" className={rowClass(false)} onClick={() => onReveal(line)}>
              <span className="truncate">{line}</span>
            </button>
          ))}
        </ReadingGroup>
      ) : null}
      {reading.properties.length > 0 ? (
        <ReadingGroup
          label={t('knowledge.flowix.properties')}
          count={reading.properties.length}
          open={openGroups.has('properties')}
          onToggle={() => toggleGroup('properties')}>
          {reading.properties.map((item) => (
            <p key={item.key} className="m-0 truncate px-2.5 py-1 text-[13px] text-[#171717]">
              {item.key}: {item.value}
            </p>
          ))}
        </ReadingGroup>
      ) : null}
      {!hasItems && complete ? (
        <p className="m-0 mt-4 px-2.5 text-[13px] text-[#8a8a8a]">{t('knowledge.notes.none')}</p>
      ) : null}
    </aside>
  )
}

function ConceptReadingRail({
  markdown,
  fromFile,
  files,
  onOpen
}: {
  markdown: string
  fromFile: string
  files: ReadonlySet<string>
  onOpen: (relative: string) => void
}) {
  const { t } = useTranslation()
  const document = conceptDocument(markdown)
  const sources = document.sources.flatMap((source) => {
    const relative = resolveBundleReference(fromFile, source.resource)
    return relative ? [{ label: source.title || relative, relative }] : []
  })
  const outgoing = conceptOutlinks(markdown, fromFile, files)
  const diagnostics = conceptDiagnostics(markdown, fromFile, files)
  return (
    <aside
      data-ui="knowledge.concept"
      aria-label={t('knowledge.notes.concept_reading')}
      className={cn(
        'flex h-full min-h-0 flex-col overflow-y-auto border-l border-[#efefef] bg-white px-3 py-3',
        pageSidebarWidthClassName
      )}>
      {sources.length > 0 ? (
        <ReadingGroup label={t('knowledge.notes.concept_sources')}>
          {sources.map((source) => (
            <button
              key={source.relative}
              type="button"
              className={rowClass(false)}
              onClick={() => onOpen(source.relative)}>
              <span className="truncate">{source.label}</span>
            </button>
          ))}
        </ReadingGroup>
      ) : null}
      {outgoing.length > 0 ? (
        <ReadingGroup label={t('knowledge.notes.concept_links')}>
          {outgoing.map((link) => (
            <button key={link.href} type="button" className={rowClass(false)} onClick={() => onOpen(link.href)}>
              <span className="truncate">{link.label}</span>
            </button>
          ))}
        </ReadingGroup>
      ) : null}
      {diagnostics.length > 0 ? (
        <ReadingGroup label={t('knowledge.notes.concept_diagnostics')}>
          {diagnostics.map((label) => (
            <p key={label} className="m-0 truncate px-2.5 py-1 text-[13px] text-[#8a8a8a]">
              {label}
            </p>
          ))}
        </ReadingGroup>
      ) : null}
    </aside>
  )
}

function NoteColumn({ ready, children }: { ready: boolean; children: ReactNode }) {
  return (
    <div className="relative flex h-full min-h-0 w-full min-w-0 flex-1 flex-col px-6 pt-1">
      {children}
      <PageEntry id="knowledge.note" ready={ready} />
    </div>
  )
}

function WikiSuggestMenu({ state }: { state: WikiSuggestState }) {
  const { t } = useTranslation()
  const listRef = useRef<HTMLUListElement>(null)
  const [place, setPlace] = useState<{ top: number; left: number } | null>(null)
  useLayoutEffect(() => {
    const rect = state.rect
    const node = listRef.current
    const height = node?.offsetHeight ?? 0
    const margin = 4
    if (!rect) {
      setPlace({ top: 8, left: 8 })
      return
    }
    const below = rect.bottom + margin
    const spaceBelow = window.innerHeight - below
    const flip = height > 0 && spaceBelow < height && rect.top - margin - height > 8
    setPlace({
      top: flip ? rect.top - margin - height : below,
      left: Math.max(8, Math.min(rect.left, window.innerWidth - 240))
    })
  }, [state.items, state.rect, state.selected])
  return createPortal(
    <ul
      ref={listRef}
      role="listbox"
      aria-label={t('knowledge.notes.wiki_suggestions')}
      className="fixed z-50 m-0 w-56 list-none rounded-[10px] border border-[#ececec] bg-white p-1 shadow-[0_8px_18px_rgb(0_0_0/0.08)]"
      style={{ top: place?.top ?? 8, left: place?.left ?? 8 }}>
      {state.items.map((item, index) => (
        <li key={item.path}>
          <button
            type="button"
            role="option"
            aria-selected={index === state.selected}
            className={cn(
              'h-8 w-full truncate rounded-[8px] px-2 text-left text-[13px]',
              index === state.selected ? 'bg-[#f2f3f5] text-[#171717]' : 'text-[#171717] hover:bg-[#f7f7f7]'
            )}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => state.select(item)}>
            {noteTitle(item.name)}
          </button>
        </li>
      ))}
    </ul>,
    document.body
  )
}

export function KnowledgeNotesPage({ corner, vaultDirectory }: { corner?: ReactNode; vaultDirectory?: string } = {}) {
  const { t } = useTranslation()
  const [workspaceStored] = usePreference('app.workspace.path')
  const workspaceRoot = displayedWorkspacePath(
    typeof workspaceStored === 'string' ? workspaceStored : '',
    readStudioHomePath()
  )
  const vaultRoot = vaultDirectory || personalVaultPath(workspaceRoot)
  const embedded = Boolean(vaultDirectory)
  const centerId = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const teamSelected = centerId !== PERSONAL_CENTER
  const [library, setLibrary] = useState<LibraryId>('local')
  const [query, setQuery] = useState('')

  const [tagFilter, setTagFilter] = useState<string | null>(null)
  const [reveal, setReveal] = useState<{ text: string; token: number } | null>(null)
  const [createError, setCreateError] = useState('')
  const [wikiSuggest, setWikiSuggest] = useState<WikiSuggestState | null>(null)
  const [personalTree, setPersonalTree] = useState<VaultTreeNode[]>([])
  const [personalNotes, setPersonalNotes] = useState<LoadedNote[]>([])
  const [bundleNotes, setBundleNotes] = useState<LoadedNote[]>([])
  const [cloudBundleId, setCloudBundleId] = useState<string | null>(null)
  const [installed, setInstalled] = useState<ReadonlySet<string>>(() => new Set())
  const [installEpoch, setInstallEpoch] = useState(0)
  const [busyId, setBusyId] = useState<string | null>(null)
  const [installFailed, setInstallFailed] = useState(false)
  const [removeFailed, setRemoveFailed] = useState(false)
  const [download, setDownload] = useState<(CloudDownloadProgress & { id: string }) | null>(null)
  const [openedId, setOpenedId] = useState<string | null>(() => readHeldNoteDraft()?.path ?? null)
  const [editorReady, setEditorReady] = useState(false)
  const [phase, setPhase] = useState<'idle' | 'loading' | 'ready' | 'unavailable' | 'too-large'>(() =>
    readHeldNoteDraft() ? 'ready' : 'idle'
  )
  const [text, setText] = useState(() => readHeldNoteDraft()?.text ?? '')
  const [oversized, setOversized] = useState(0)
  const draftReader = useRef<(() => string) | null>(null)
  const [customTitle, setCustomTitle] = useState<string | null>(() => readHeldNoteDraft()?.title ?? null)
  const [savedAtByNote, setSavedAtByNote] = useState<ReadonlyMap<string, Date>>(() => new Map())
  const [vaultEpoch, setVaultEpoch] = useState(0)
  const [savedBodies, setSavedBodies] = useState<ReadonlyMap<string, string | null>>(() => new Map())
  const [compileOpen, setCompileOpen] = useState(false)
  const [issueFilter, setIssueFilter] = useState<'skipped' | 'failed' | null>(null)
  const [sidebarPane, setSidebarPane] = useState<'notes' | 'concepts'>('notes')
  const [graphOpen, setGraphOpen] = useState(false)
  const [indexMarkdown, setIndexMarkdown] = useState<string | null>(null)
  const [conceptPath, setConceptPath] = useState<string | null>(null)
  const [conceptMarkdown, setConceptMarkdown] = useState<string | null>(null)
  const [conceptPhase, setConceptPhase] = useState<'idle' | 'loading' | 'ready' | 'unavailable'>('idle')
  const [conceptBodies, setConceptBodies] = useState<ReadonlyMap<string, string | null>>(() => new Map())
  const [refreshingVault, setRefreshingVault] = useState(false)
  const refreshingVaultRef = useRef(false)
  const [vaultListed, setVaultListed] = useState(false)
  const carriedDraft = useRef<{ to: string; text: string; title: string | null } | null>(null)
  const heldSeeded = useRef(false)
  if (!heldSeeded.current) {
    heldSeeded.current = true
    const held = readHeldNoteDraft()
    if (held) carriedDraft.current = { to: held.path, text: held.text, title: held.title }
  }
  const [inlineEdit, setInlineEdit] = useState<VaultDraft | null>(null)
  const [menu, setMenu] = useState<VaultMenuTarget | null>(null)
  const [deleteError, setDeleteError] = useState('')
  const [moveError, setMoveError] = useState('')
  const committingRef = useRef(false)
  if ((embedded || !teamSelected) && library !== 'local') {
    setLibrary('local')
    setOpenedId(null)
    setCloudBundleId(null)
    setBundleNotes([])
    setPhase('idle')
    setQuery('')
    setText('')
  }

  const markEditorReady = useCallback(() => setEditorReady(true), [])

  useEffect(() => {
    if (!workspaceRoot) return
    const reveal = (path: string) => {
      if (!isPersonalNotePath(workspaceRoot, path)) return
      clearPendingPersonalNote(path)
      setLibrary('local')
      setOpenedId(path)
      setPhase('loading')
      setVaultEpoch((current) => current + 1)
    }
    const pending = consumePendingPersonalNote()
    if (pending) reveal(pending)
    const onOpen = (event: Event) => {
      const path = (event as CustomEvent<OpenPersonalNoteDetail>).detail?.path
      if (path) reveal(path)
    }
    window.addEventListener(OPEN_PERSONAL_NOTE_EVENT, onOpen)
    return () => window.removeEventListener(OPEN_PERSONAL_NOTE_EVENT, onOpen)
  }, [workspaceRoot])

  useEffect(() => {
    setEditorReady(false)
    if (!openedId) return
    const timer = window.setTimeout(() => setEditorReady(true), 2500)
    return () => window.clearTimeout(timer)
  }, [openedId])

  useEffect(() => {
    if (!workspaceRoot) {
      if (refreshingVaultRef.current) {
        refreshingVaultRef.current = false
        setRefreshingVault(false)
      }
      return
    }
    let cancelled = false
    void (async () => {
      try {
        const tree = await listVaultTree(workspaceRoot, vaultRoot)
        const next = flattenVaultFiles(tree).map(listedNote)
        if (cancelled) return
        setPersonalTree(tree)
        setPersonalNotes((current) => {
          const previous = new Map(current.map((note) => [note.path, note]))
          const held = readHeldNoteDraft()
          return next.map((note) => {
            const saved = previous.get(note.path)
            if (saved?.markdown != null) {
              return { ...note, markdown: saved.markdown, unavailable: saved.unavailable }
            }
            if (held && held.path === note.path) return { ...note, markdown: held.savedMarkdown, unavailable: false }
            return note
          })
        })
      } catch {
        if (!cancelled) {
          setPersonalTree([])
          setPersonalNotes([])
        }
      } finally {
        if (!cancelled) setVaultListed(true)
        if (!cancelled && refreshingVaultRef.current) {
          refreshingVaultRef.current = false
          setRefreshingVault(false)
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [workspaceRoot, vaultRoot, vaultEpoch])

  const refreshLocalVault = () => {
    if (!workspaceRoot || refreshingVaultRef.current) return
    refreshingVaultRef.current = true
    setRefreshingVault(true)
    setVaultEpoch((current) => current + 1)
  }

  useEffect(() => {
    if (!menu) return
    const close = (event: PointerEvent) => {
      const target = event.target
      if (target instanceof Element && target.closest('[data-knowledge-menu], .ui-popconfirm')) return
      setMenu(null)
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setMenu(null)
    }
    window.addEventListener('pointerdown', close)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('pointerdown', close)
      window.removeEventListener('keydown', onKey)
    }
  }, [menu])

  useEffect(() => {
    if (library !== 'cloud' || !workspaceRoot) return
    let cancelled = false
    void (async () => {
      try {
        const listing = await listLocalEntries(workspaceRoot, cloudVaultPath(workspaceRoot))
        if (cancelled) return
        setInstalled(
          new Set(
            listing.entries
              .filter((entry) => entry.kind === 'directory' && cloudKnowledgeBundle(entry.name))
              .map((entry) => entry.name)
          )
        )
      } catch (error) {
        if (!cancelled && localEntryStatus(error) === 404) setInstalled(new Set())
      }
    })()
    return () => {
      cancelled = true
    }
  }, [library, workspaceRoot, installEpoch])

  useEffect(() => {
    if (library !== 'cloud' || !cloudBundleId || !workspaceRoot) return
    let cancelled = false
    void (async () => {
      try {
        const next = flattenVaultFiles(
          await listVaultTree(workspaceRoot, cloudBundlePath(workspaceRoot, cloudBundleId))
        ).map(listedNote)
        if (cancelled) return
        setBundleNotes(next)
        const index = next.find((note) => note.name.toLowerCase() === 'index.md') ?? next[0] ?? null
        setOpenedId(index?.id ?? null)
        if (!index) setPhase('idle')
      } catch {
        if (!cancelled) {
          setBundleNotes([])
          setOpenedId(null)
          setPhase('unavailable')
        }
      }
    })()
    return () => {
      cancelled = true
    }
  }, [library, cloudBundleId, workspaceRoot])

  const personalVault = isPersonalVaultDirectory(vaultRoot)
  const showConcepts = library === 'local' && personalVault
  const showCompile = showConcepts && !embedded
  const compileRun = useKnowledgeCompileRun({
    showCompile,
    workspaceRoot,
    vaultRoot,
    onAccepted: () => setVaultEpoch((current) => current + 1)
  })
  const compileSessionId = compileRun.session?.sessionId
  const compilePending = compileRun.pending
  useEffect(() => {
    if (!compileSessionId) return
    if (compilePending && compilePending.length === 0) return
    const text =
      compilePending && compilePending.length > 0
        ? t('knowledge.notes.compile_user_turn_pending', { paths: compilePending.join('\n') })
        : t('knowledge.notes.compile_user_turn')
    void dispatchKnowledgeCompileTurn(compileSessionId, text).then((sent) => {
      if (!sent) compileRun.fail()
    })
  }, [compilePending, compileRun.fail, compileSessionId, t])
  const localNotes = useMemo(
    () => (showConcepts ? personalNotes.filter((note) => !isVaultArtifact(vaultRoot, note.path)) : personalNotes),
    [personalNotes, showConcepts, vaultRoot]
  )
  const notes = library === 'local' ? localNotes : bundleNotes
  const opened = notes.find((note) => note.id === openedId) ?? null
  const openedPath = opened?.path ?? ''
  const notesRef = useRef(notes)
  notesRef.current = notes
  const indexedPaths = notes.map((note) => note.path).join('\n')
  const shouldIndex = Boolean(openedId) || query.trim().length > 0 || graphOpen

  useEffect(() => {
    if (!workspaceRoot || !shouldIndex) return
    let cancelled = false
    const targets = notesRef.current.slice(0, NOTE_INDEX_LIMIT)
    void (async () => {
      const next = new Map<string, string | null>()
      for (const note of targets) {
        if (cancelled) return
        try {
          const bytes = await readLocalDocumentBytes(workspaceRoot, note.path, undefined, NOTE_INDEX_BYTES)
          next.set(note.path, await decodeUtf8Async(bytes))
        } catch {
          next.set(note.path, null)
        }
      }
      if (!cancelled) setSavedBodies(next)
    })()
    return () => {
      cancelled = true
    }
  }, [indexedPaths, library, shouldIndex, vaultEpoch, workspaceRoot])

  useEffect(() => {
    if (phase !== 'ready' || !opened || opened.markdown == null) return
    const saved = markdownBody(opened.markdown)
    if (text !== saved) {
      writeHeldNoteDraft({ path: opened.path, text, title: customTitle, savedMarkdown: opened.markdown })
      return
    }
    if (readHeldNoteDraft()?.path === opened.path) writeHeldNoteDraft(null)
  }, [customTitle, opened, phase, text])

  useEffect(() => {
    if (!vaultListed || library !== 'local') return
    const held = readHeldNoteDraft()
    if (!held) return
    if (personalNotes.some((note) => note.path === held.path)) return
    writeHeldNoteDraft(null)
    setOpenedId((current) => (current === held.path ? null : current))
    setText('')
    setCustomTitle(null)
    setPhase('idle')
  }, [library, personalNotes, vaultListed])

  useEffect(() => {
    if (!openedId || !openedPath) return
    const carry = carriedDraft.current
    const restoring = carry && carry.to === openedPath ? carry : null
    if (restoring && opened?.markdown != null) {
      carriedDraft.current = null
      setCustomTitle(restoring.title)
      setText(restoring.text)
      setPhase('ready')
      return
    }
    if (restoring) carriedDraft.current = null
    let cancelled = false
    if (restoring) {
      setCustomTitle(restoring.title)
      setText(restoring.text)
    } else {
      setPhase('loading')
      setText('')
      setCustomTitle(null)
    }
    setOversized(0)
    void (async () => {
      try {
        const bytes = await readLocalDocumentBytes(workspaceRoot, openedPath, undefined, TEXT_READ_BYTES)
        const markdown = await decodeUtf8Async(bytes)
        if (cancelled) return
        if (markdown === null) {
          setPhase('unavailable')
          return
        }
        if (markdown.length > VIRTUAL_TEXT_CHARS) {
          setOversized(bytes.byteLength)
          setPhase('too-large')
          return
        }
        if (restoring) {
          setCustomTitle(restoring.title)
          setText(restoring.text)
        } else {
          setCustomTitle(readStoredTitle(markdown))
          setText(markdownBody(markdown))
        }
        const apply = (current: LoadedNote[]) =>
          current.map((note) => (note.id === openedId ? { ...note, markdown, unavailable: false } : note))
        if (library === 'local') setPersonalNotes(apply)
        else setBundleNotes(apply)
        setPhase('ready')
      } catch (error) {
        if (cancelled) return
        if (error instanceof FileTooLargeError) {
          setOversized(error.bytes)
          setPhase('too-large')
          return
        }
        setPhase('unavailable')
      }
    })()
    return () => {
      cancelled = true
    }
  }, [library, openedId, openedPath, workspaceRoot])

  useEffect(() => {
    if (!workspaceRoot || !showConcepts) {
      setIndexMarkdown(null)
      return
    }
    let cancelled = false
    void (async () => {
      try {
        const bytes = await readLocalDocumentBytes(workspaceRoot, `${vaultRoot}/index.md`, undefined, NOTE_INDEX_BYTES)
        const markdown = await decodeUtf8Async(bytes)
        if (!cancelled) setIndexMarkdown(markdown)
      } catch {
        if (!cancelled) setIndexMarkdown(null)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [showConcepts, vaultEpoch, vaultRoot, workspaceRoot])

  useEffect(() => {
    if (!conceptPath || !workspaceRoot) return
    let cancelled = false
    setConceptPhase('loading')
    setConceptMarkdown(null)
    void (async () => {
      try {
        const bytes = await readLocalDocumentBytes(workspaceRoot, conceptPath, undefined, TEXT_READ_BYTES)
        const markdown = await decodeUtf8Async(bytes)
        if (cancelled) return
        if (markdown == null) {
          setConceptPhase('unavailable')
          return
        }
        setConceptMarkdown(markdown)
        setConceptPhase('ready')
      } catch {
        if (!cancelled) setConceptPhase('unavailable')
      }
    })()
    return () => {
      cancelled = true
    }
  }, [conceptPath, workspaceRoot])

  const cloudHome = library === 'cloud' && !cloudBundleId
  const needle = query.trim().toLowerCase()
  const readingNotes = useMemo<ReadingNote[]>(
    () =>
      notes.map((note) => ({
        name: note.name,
        path: note.path,
        markdown: note.markdown ?? (savedBodies.has(note.path) ? (savedBodies.get(note.path) ?? null) : null)
      })),
    [notes, savedBodies]
  )

  const coverage = indexCoverage(readingNotes)
  const tagPaths = useMemo(() => {
    if (!tagFilter) return null
    const allowed = new Set<string>()
    for (const note of readingNotes) {
      if (note.markdown == null) continue
      if (!noteTags(note.markdown).includes(tagFilter)) continue
      const relative = bundleRelative(vaultRoot, note.path)
      if (relative) allowed.add(relative)
    }
    return allowed
  }, [readingNotes, tagFilter, vaultRoot])
  const catalog = useMemo<IndexCatalog>(
    () => (indexMarkdown ? parseIndexCatalog(indexMarkdown) : { concepts: [], skipped: [], failed: [] }),
    [indexMarkdown]
  )
  const libraryCompiled = catalog.concepts.length
  const librarySkipped = catalog.skipped.length
  const libraryFailed = catalog.failed.length
  const conceptKey = catalog.concepts.map((item) => item.file).join('\n')
  useEffect(() => {
    if (!workspaceRoot || !showConcepts || !query.trim() || !conceptKey) return
    let cancelled = false
    const targets = catalog.concepts.slice(0, NOTE_INDEX_LIMIT)
    void (async () => {
      const next = new Map<string, string | null>()
      for (const item of targets) {
        if (cancelled) return
        try {
          const bytes = await readLocalDocumentBytes(
            workspaceRoot,
            `${vaultRoot}/${item.file}`,
            undefined,
            NOTE_INDEX_BYTES
          )
          next.set(item.file, await decodeUtf8Async(bytes))
        } catch {
          next.set(item.file, null)
        }
      }
      if (!cancelled) setConceptBodies(next)
    })()
    return () => {
      cancelled = true
    }
  }, [catalog.concepts, conceptKey, query, showConcepts, vaultEpoch, vaultRoot, workspaceRoot])
  const passageHits = useMemo(() => {
    const docs = catalog.concepts.map((item) => ({
      file: item.file,
      title: item.title,
      name: item.file.slice(item.file.lastIndexOf('/') + 1),
      markdown: conceptBodies.get(item.file) ?? null
    }))
    return conceptPassageHits(docs, query)
  }, [catalog.concepts, conceptBodies, query])
  const issuePaths = useMemo(() => {
    if (!issueFilter) return null
    const items = issueFilter === 'skipped' ? catalog.skipped : catalog.failed
    return new Set(items.map((item) => item.path))
  }, [catalog.failed, catalog.skipped, issueFilter])
  const noteNodes = useMemo(() => {
    const visible = visibleNoteTree(personalTree)
    const allowed = issuePaths ?? tagPaths
    return allowed ? treeMatchingPaths(visible, vaultRoot, allowed) : visible
  }, [issuePaths, personalTree, tagPaths, vaultRoot])
  const continueNotes = recentNotes(visibleNoteTree(personalTree))
  const conceptQuery = query.trim().toLowerCase()
  const notesVisible = !showConcepts || sidebarPane !== 'concepts' || issueFilter != null
  const conceptsVisible = showConcepts && !issueFilter && sidebarPane === 'concepts'
  const displayNames = useMemo(() => {
    const map = new Map<string, string>()
    for (const note of readingNotes) map.set(note.path, noteDisplayName(note.name, note.markdown))
    return map
  }, [readingNotes])
  const listedConcepts = catalog.concepts.filter((item) => {
    if (!conceptQuery) return true
    const name = item.file.slice(item.file.lastIndexOf('/') + 1)
    return item.title.toLowerCase().includes(conceptQuery) || name.toLowerCase().includes(conceptQuery)
  })
  const listedConceptFiles = new Set(listedConcepts.map((item) => item.file))
  const conceptHits = passageHits.filter((item) => !listedConceptFiles.has(item.file))
  const vaultFiles = useMemo(() => {
    const files = new Set<string>()
    for (const note of personalNotes) {
      const relative = bundleRelative(vaultRoot, note.path)
      if (relative) files.add(relative)
    }
    return files
  }, [personalNotes, vaultRoot])
  const openNote = (path: string) => {
    setConceptPath(null)
    setReveal(null)
    setWikiSuggest(null)
    if (openedId === path && (phase === 'loading' || phase === 'ready')) return
    setOpenedId(path)
    setPhase('loading')
    setText('')
  }
  const createUnresolved = async (target: string) => {
    if (!workspaceRoot || !opened) return
    setCreateError('')
    const plan = planUnresolvedNote(
      notes.map((note) => ({ name: note.name, path: note.path })),
      target
    )
    if (plan.kind === 'refuse') {
      const key =
        plan.error === 'empty'
          ? 'knowledge.notes.name_empty'
          : plan.error === 'reserved'
            ? 'knowledge.notes.name_reserved'
            : 'knowledge.notes.name_invalid'
      setCreateError(t(key))
      return
    }
    if (plan.kind === 'open') {
      openNote(plan.path)
      return
    }
    const parent = parentDir(opened.path)
    try {
      if (!vaultDirectory) await ensurePersonalVault(workspaceRoot)
      const created = await createLocalEntry(workspaceRoot, parent, plan.name, 'file')
      await writeLocalDocumentBytes(workspaceRoot, created.path, new TextEncoder().encode('\n'))
      setReveal(null)
      setOpenedId(created.path)
      setPhase('loading')
      setText('')
      setVaultEpoch((current) => current + 1)
    } catch (error) {
      if (localEntryStatus(error) === 409) {
        setVaultEpoch((current) => current + 1)
        openNote(`${parent}/${plan.name}`)
        return
      }
      setCreateError(t('knowledge.notes.mutate_failed'))
      throw error
    }
  }
  const wikiNotesRef = useRef<WikiNoteRef[]>([])
  const wikiNoteKey = readingNotes.map((note) => `${note.path}\0${note.name}`).join('\n')
  const wikiNoteKeyRef = useRef('')
  if (wikiNoteKeyRef.current !== wikiNoteKey) {
    wikiNoteKeyRef.current = wikiNoteKey
    wikiNotesRef.current.splice(
      0,
      wikiNotesRef.current.length,
      ...readingNotes.map((note) => ({ name: note.name, path: note.path }))
    )
  }
  const wikiExtensionRef = useRef<ReturnType<typeof createKnowledgeWikiLink> | null>(null)
  if (wikiExtensionRef.current == null) {
    wikiExtensionRef.current = createKnowledgeWikiLink({
      notes: wikiNotesRef.current,
      onOpen: (path) => {
        setConceptPath(null)
        setWikiSuggest(null)
        setReveal(null)
        setOpenedId(path)
        setPhase('loading')
        setText('')
      },
      onSuggest: setWikiSuggest
    })
  }
  const wikiExtension = wikiExtensionRef.current
  const editorExtensions = useMemo(() => {
    if (!reveal?.text) return [wikiExtension]
    return [wikiExtension, createKnowledgeReveal(reveal.text)]
  }, [reveal?.text, wikiExtension])
  const openBundleFile = (relative: string) => {
    if (relative.startsWith('concepts/') && !relative.slice('concepts/'.length).includes('/')) {
      setConceptPath(`${vaultRoot}/${relative}`)
      return
    }
    openNote(`${vaultRoot}/${relative}`)
  }
  const visible = notes.filter((note) => {
    if (!needle) return true
    const title = (displayNames.get(note.path) ?? noteTitle(note.name)).toLowerCase()
    return title.includes(needle) || note.name.toLowerCase().includes(needle)
  })

  const selectLibrary = (next: LibraryId) => {
    writeHeldNoteDraft(null)
    setLibrary(next)
    setOpenedId(null)
    setConceptPath(null)
    setCloudBundleId(null)
    setBundleNotes([])
    setPhase('idle')
    setQuery('')
    setText('')
    setGraphOpen(false)
    setSidebarPane('notes')
    compileRun.reset()
    setCompileOpen(false)
  }

  const installBundle = async (bundle: CloudKnowledgeBundle) => {
    if (!workspaceRoot || installed.has(bundle.id) || busyId === bundle.id) return
    setBusyId(bundle.id)
    setDownload({ id: bundle.id, completed: 0, total: bundle.files.length, name: '' })
    try {
      await installCloudKnowledge(workspaceRoot, bundle, (progress) => {
        setDownload({ id: bundle.id, ...progress })
      })
      setInstalled((current) => {
        const next = new Set(current)
        next.add(bundle.id)
        return next
      })
      setInstallEpoch((current) => current + 1)
      setInstallFailed(false)
      setRemoveFailed(false)
    } catch {
      setInstallFailed(true)
    } finally {
      setBusyId(null)
      setDownload(null)
    }
  }

  const removeBundle = async (id: string) => {
    if (!workspaceRoot || busyId) return
    try {
      await removeLocalEntry(workspaceRoot, cloudBundlePath(workspaceRoot, id))
      setInstalled((current) => {
        const next = new Set(current)
        next.delete(id)
        return next
      })
      if (cloudBundleId === id) {
        setCloudBundleId(null)
        setBundleNotes([])
        setOpenedId(null)
        setPhase('idle')
        setText('')
      }
      setInstallEpoch((current) => current + 1)
      setRemoveFailed(false)
    } catch {
      setRemoveFailed(true)
    }
  }

  const save = async (markdown: string) => {
    if (!opened) return
    const stem = noteTitle(opened.name)
    const file = composeNote(markdown, customTitle, stem, opened.markdown ? noteTypeLine(opened.markdown) : null)
    await writeLocalDocumentBytes(workspaceRoot, opened.path, new TextEncoder().encode(file))
    if (readHeldNoteDraft()?.path === opened.path) writeHeldNoteDraft(null)
    setText(markdown)
    setSavedAtByNote((current) => {
      const next = new Map(current)
      next.set(opened.id, new Date())
      return next
    })
    const apply = (current: LoadedNote[]) =>
      current.map((note) => (note.id === opened.id ? { ...note, markdown: file, unavailable: false } : note))
    if (library === 'local') setPersonalNotes(apply)
    else setBundleNotes(apply)
  }

  const commitTitle = async (raw: string): Promise<string> => {
    if (!opened || !workspaceRoot) return t('knowledge.notes.mutate_failed')
    const next = raw.trim()
    if (!next) return t('knowledge.notes.title_empty')
    if (titleChars(next).length > NOTE_TITLE_LIMIT) return t('knowledge.notes.title_too_long')
    const stem = noteTitle(opened.name)
    const shown = customTitle ?? stem
    if (next === shown) return ''
    const stored = next === stem ? null : next
    const file = composeNote(
      draftReader.current?.() ?? text,
      stored,
      stem,
      opened.markdown ? noteTypeLine(opened.markdown) : null
    )
    try {
      await writeLocalDocumentBytes(workspaceRoot, opened.path, new TextEncoder().encode(file))
      setCustomTitle(stored)
      const apply = (current: LoadedNote[]) =>
        current.map((note) => (note.id === opened.id ? { ...note, markdown: file, unavailable: false } : note))
      if (library === 'local') setPersonalNotes(apply)
      else setBundleNotes(apply)
      return ''
    } catch {
      return t('knowledge.notes.mutate_failed')
    }
  }

  const beginCreate = (kind: 'file' | 'folder', parent: string) => {
    if (!workspaceRoot || !parent) return
    const base = kind === 'file' ? t('knowledge.notes.untitled') : t('knowledge.notes.untitled_folder')
    setInlineEdit({
      mode: 'create',
      kind,
      parent,
      name: nextName(siblingNames(personalTree, parent, vaultRoot), base),
      error: '',
      busy: false,
      touched: false
    })
    setMenu(null)
  }

  const beginRename = (path: string) => {
    if (!canRenameVaultPath(personalTree, path)) {
      setMenu(null)
      return
    }
    setInlineEdit({
      mode: 'rename',
      path,
      name: path.slice(path.lastIndexOf('/') + 1),
      error: '',
      busy: false,
      touched: false
    })
    setMenu(null)
  }

  const commitInline = async (shown?: string) => {
    const edit = inlineEdit
    if (!edit || edit.busy || committingRef.current || !workspaceRoot) return
    committingRef.current = true
    const raw = (shown ?? edit.name).trim()
    const invalid =
      !raw || raw === '.' || raw === '..'
        ? 'empty'
        : raw.includes('/') || raw.includes('\\') || raw.startsWith('.')
          ? 'invalid'
          : ''
    if (invalid) {
      setInlineEdit({
        ...edit,
        error: t(invalid === 'empty' ? 'knowledge.notes.name_empty' : 'knowledge.notes.name_invalid')
      })
      committingRef.current = false
      return
    }
    const vault = vaultRoot
    let name = raw
    if (edit.mode === 'create' && edit.kind === 'file') {
      const parsed = markdownFileName(raw)
      if ('error' in parsed) {
        const key =
          parsed.error === 'empty'
            ? 'knowledge.notes.name_empty'
            : parsed.error === 'type'
              ? 'knowledge.notes.markdown_only'
              : 'knowledge.notes.name_invalid'
        setInlineEdit({ ...edit, error: t(key) })
        committingRef.current = false
        return
      }
      name = parsed.name
      if (isReservedNoteName(name)) {
        setInlineEdit({ ...edit, error: t('knowledge.notes.name_reserved') })
        committingRef.current = false
        return
      }
    }
    let renameName = raw
    if (edit.mode === 'rename') {
      const node = findVaultNode(personalTree, edit.path)
      if (!node || !canRenameVaultPath(personalTree, edit.path)) {
        setInlineEdit(null)
        committingRef.current = false
        return
      }
      if (node.kind === 'file') {
        const parsed = markdownFileName(raw)
        if ('error' in parsed) {
          const key =
            parsed.error === 'empty'
              ? 'knowledge.notes.name_empty'
              : parsed.error === 'type'
                ? 'knowledge.notes.markdown_only'
                : 'knowledge.notes.name_invalid'
          setInlineEdit({ ...edit, error: t(key) })
          committingRef.current = false
          return
        }
        if (isReservedNoteName(parsed.name)) {
          setInlineEdit({ ...edit, error: t('knowledge.notes.name_reserved') })
          committingRef.current = false
          return
        }
        renameName = parsed.name
      }
    }
    try {
      if (edit.mode === 'create') {
        if (siblingNames(personalTree, edit.parent, vault).includes(name)) {
          setInlineEdit({ ...edit, error: t('knowledge.notes.name_taken') })
          return
        }
        setInlineEdit({ ...edit, busy: true, error: '' })
        if (!vaultDirectory) await ensurePersonalVault(workspaceRoot)
        const created = await createLocalEntry(
          workspaceRoot,
          edit.parent,
          name,
          edit.kind === 'file' ? 'file' : 'folder'
        )
        if (edit.kind === 'file') {
          await writeLocalDocumentBytes(workspaceRoot, created.path, new TextEncoder().encode('\n'))
          setOpenedId(created.path)
          setPhase('loading')
          setText('')
        }
      } else {
        const currentName = edit.path.slice(edit.path.lastIndexOf('/') + 1)
        const finalName = renameName
        if (finalName === currentName) {
          setInlineEdit(null)
          return
        }
        const parent = parentDir(edit.path)
        if (siblingNames(personalTree, parent, vault).includes(finalName)) {
          setInlineEdit({ ...edit, error: t('knowledge.notes.name_taken') })
          return
        }
        setInlineEdit({ ...edit, busy: true, error: '' })
        const renamed = await renameLocalEntry(workspaceRoot, edit.path, finalName)
        const nextOpened =
          openedId === edit.path
            ? renamed.path
            : openedId && openedId.startsWith(`${edit.path}/`)
              ? `${renamed.path}${openedId.slice(edit.path.length)}`
              : null
        if (nextOpened) carriedDraft.current = { to: nextOpened, text, title: customTitle }
        setPersonalNotes((current) =>
          current.map((note) => {
            if (note.path === edit.path) {
              return { ...note, id: renamed.path, path: renamed.path, name: finalName }
            }
            if (note.path.startsWith(`${edit.path}/`)) {
              const path = `${renamed.path}${note.path.slice(edit.path.length)}`
              return { ...note, id: path, path, name: path.slice(path.lastIndexOf('/') + 1) }
            }
            return note
          })
        )
        setOpenedId((current) => {
          if (!current) return current
          if (current === edit.path) return renamed.path
          if (current.startsWith(`${edit.path}/`)) return `${renamed.path}${current.slice(edit.path.length)}`
          return current
        })
      }
      setInlineEdit(null)
      setVaultEpoch((current) => current + 1)
    } catch (error) {
      const taken = localEntryStatus(error) === 409
      setInlineEdit((current) =>
        current
          ? {
              ...current,
              busy: false,
              error: t(taken ? 'knowledge.notes.name_taken' : 'knowledge.notes.mutate_failed')
            }
          : current
      )
    } finally {
      committingRef.current = false
    }
  }

  const moveNode = async (sourcePath: string, destination: string) => {
    if (!workspaceRoot || !sourcePath || committingRef.current) return
    const vault = vaultRoot
    const inside = (path: string) => path === vault || path.startsWith(`${vault}/`)
    if (!inside(sourcePath) || !inside(destination) || sourcePath === vault) return
    if (parentDir(sourcePath) === destination) {
      setMoveError('')
      return
    }
    if (destination === sourcePath || destination.startsWith(`${sourcePath}/`)) {
      setMoveError(t('knowledge.notes.move_into_self'))
      return
    }
    committingRef.current = true
    try {
      const moved = await moveLocalEntry(workspaceRoot, sourcePath, destination)
      setOpenedId((current) => {
        if (!current) return current
        if (current === sourcePath) return moved.path
        if (current.startsWith(`${sourcePath}/`)) return `${moved.path}${current.slice(sourcePath.length)}`
        return current
      })
      if (openedId === sourcePath || (openedId && openedId.startsWith(`${sourcePath}/`))) setPhase('loading')
      setMoveError('')
      setVaultEpoch((current) => current + 1)
    } catch (error) {
      const taken = localEntryStatus(error) === 409
      setMoveError(t(taken ? 'knowledge.notes.name_taken' : 'knowledge.notes.mutate_failed'))
    } finally {
      committingRef.current = false
    }
  }

  const confirmDelete = async (path: string) => {
    if (!path || !workspaceRoot) return
    setDeleteError('')
    try {
      await removeLocalEntry(workspaceRoot, path)
      setOpenedId((current) => {
        if (!current) return current
        if (current === path || current.startsWith(`${path}/`)) return null
        return current
      })
      if (openedId === path || (openedId && openedId.startsWith(`${path}/`))) {
        setPhase('idle')
        setText('')
      }
      const held = readHeldNoteDraft()
      if (held && (held.path === path || held.path.startsWith(`${path}/`))) writeHeldNoteDraft(null)
      setVaultEpoch((current) => current + 1)
    } catch (error) {
      setDeleteError(t('knowledge.notes.mutate_failed'))
      throw error
    }
  }

  return (
    <div data-ui="knowledge.view" className="flex h-full min-h-0 flex-col overflow-hidden bg-white">
      {embedded ? null : (
        <div className="shrink-0 border-b border-[#efefef] bg-white">
          <div className="flex min-h-12 items-center gap-3 px-4 py-2">
            <h1 className="m-0 shrink-0 text-[24px] leading-tight font-bold text-foreground">
              {t('knowledge.notes.title')}
            </h1>
            <p className="m-0 min-w-0 truncate text-[13px] text-[#8a8a8a]">{t('knowledge.notes.subtitle')}</p>
            <div className="ml-auto flex max-w-[72%] min-w-0 flex-wrap items-center justify-end gap-1">
              {library === 'local' ? (
                <KnowledgeSegmentBar label={t('knowledge.notes.toolbar')}>
                  <KnowledgeSegment
                    className="gap-1"
                    selected={graphOpen}
                    aria-pressed={graphOpen}
                    onClick={() => setGraphOpen((current) => !current)}>
                    <Waypoints className="size-3.5" strokeWidth={1.7} aria-hidden />
                    {t('knowledge.notes.graph')}
                  </KnowledgeSegment>
                </KnowledgeSegmentBar>
              ) : null}
              {showCompile ? (
                <Button
                  type="button"
                  variant="emphasis"
                  size="sm"
                  className="h-[30px] rounded-[8px]"
                  onClick={() => setCompileOpen(true)}>
                  {t('knowledge.hub.compile_action')}
                </Button>
              ) : null}
              {teamSelected ? (
                /* 本地/云端切换暂隐(保持 local 默认视图);测试仍靠此控件驱动云端场景,故 class 隐藏而非删除 */
                <SegmentedControl<LibraryId>
                  aria-label={t('knowledge.notes.place')}
                  size="sm"
                  className="hidden h-[30px] shrink-0"
                  value={library}
                  options={[
                    { value: 'local', label: t('knowledge.notes.local') },
                    { value: 'cloud', label: t('knowledge.notes.cloud') }
                  ]}
                  onValueChange={selectLibrary}
                />
              ) : null}
              {corner}
            </div>
          </div>
          <WikiRetrievalStatus />
        </div>
      )}
      <div className="flex min-h-0 flex-1">
        {cloudHome ? (
          <CloudKnowledgeHome
            query={query}
            onQueryChange={setQuery}
            installed={installed}
            installFailed={installFailed}
            removeFailed={removeFailed}
            download={download}
            onInstall={(bundle) => {
              void installBundle(bundle)
            }}
            onOpen={(id) => {
              setQuery('')
              setCloudBundleId(id)
              setOpenedId(null)
              setPhase('loading')
              setText('')
            }}
            onRemove={(id) => {
              void removeBundle(id)
            }}
          />
        ) : (
          <>
            <aside
              className={cn(
                'flex h-full min-h-0 flex-col overflow-hidden bg-white px-3 pt-1 pb-4',
                pageSidebarWidthClassName,
                pageSidebarEdgeClassName
              )}>
              <div className="mb-2 flex w-full items-center gap-1.5">
                <FileTreeSearchField
                  className="min-w-0 flex-1"
                  value={query}
                  onChange={setQuery}
                  placeholder={t('knowledge.notes.search')}
                  searchLabel={t('knowledge.notes.search')}
                  refreshLabel={t('knowledge.notes.refresh')}
                  refreshing={false}
                  showRefresh={false}
                  onRefresh={() => undefined}
                />
              </div>
              {tagFilter ? (
                <p className="m-0 mb-2 flex items-center gap-2 px-2.5 text-[12px] text-[#8a8a8a]">
                  <button
                    type="button"
                    aria-pressed
                    className="h-7 rounded-[8px] bg-[#f2f3f5] px-2 text-[12px] text-[#171717]"
                    onClick={() => setTagFilter(null)}>
                    {tagFilter}
                  </button>
                  {coverage.read < coverage.listed
                    ? t('knowledge.notes.index_read', { read: coverage.read, listed: coverage.listed })
                    : null}
                </p>
              ) : null}
              {showConcepts ? (
                <div
                  role="radiogroup"
                  aria-label={t('knowledge.notes.sidebar')}
                  className="mb-1 flex shrink-0 border-b border-[#efefef]">
                  <button
                    type="button"
                    role="radio"
                    aria-checked={sidebarPane === 'notes'}
                    className={cn(
                      'h-8 flex-1 border-b-2 text-[13px]',
                      sidebarPane === 'notes'
                        ? 'border-[#171717] font-medium text-[#171717]'
                        : 'border-transparent text-[#8a8a8a]'
                    )}
                    onClick={() => {
                      setIssueFilter(null)
                      setSidebarPane('notes')
                    }}>
                    {t('knowledge.notes.list')}
                  </button>
                  <button
                    type="button"
                    role="radio"
                    aria-checked={sidebarPane === 'concepts'}
                    className={cn(
                      'h-8 flex-1 border-b-2 text-[13px]',
                      sidebarPane === 'concepts'
                        ? 'border-[#171717] font-medium text-[#171717]'
                        : 'border-transparent text-[#8a8a8a]'
                    )}
                    onClick={() => {
                      setIssueFilter(null)
                      setSidebarPane('concepts')
                    }}>
                    {t('knowledge.notes.concepts')}
                  </button>
                </div>
              ) : null}
              {issueFilter ? (
                <div className="mb-2 flex shrink-0 flex-col gap-1">
                  <div className="flex items-center justify-between gap-2 px-1">
                    <span className="truncate text-[12px] text-[#8a8a8a]">
                      {t(
                        issueFilter === 'skipped'
                          ? 'knowledge.notes.compile_skipped'
                          : 'knowledge.notes.compile_failed',
                        {
                          count: (issueFilter === 'skipped' ? catalog.skipped : catalog.failed).length
                        }
                      )}
                    </span>
                    <button
                      type="button"
                      className="h-7 shrink-0 rounded-[8px] bg-[#f2f3f5] px-2 text-[12px] font-medium text-[#171717]"
                      onClick={() => setIssueFilter(null)}>
                      {t('knowledge.notes.show_all')}
                    </button>
                  </div>
                  <ul
                    aria-label={t('knowledge.notes.compile_reasons')}
                    className="flex max-h-40 flex-col gap-1 overflow-y-auto">
                    {(issueFilter === 'skipped' ? catalog.skipped : catalog.failed).map((item) => {
                      const reasonKey = SKIP_REASONS[item.reason]
                      const reason = reasonKey ? t(reasonKey) : item.reason
                      const label = t('knowledge.notes.compile_reason_item', { name: noteTitle(item.path), reason })
                      const relative = item.path.replace(/\\/g, '/')
                      const absolute = relative.startsWith('/') ? relative : `${vaultRoot}/${relative}`
                      const openable =
                        !relative.split('/').includes('..') && notes.some((note) => note.path === absolute)
                      return openable ? (
                        <li key={`${issueFilter}:${item.path}`}>
                          <button
                            type="button"
                            className="h-7 w-full truncate rounded-[8px] px-2.5 text-left text-[12px] text-[#171717] hover:bg-[#f7f7f7]"
                            onClick={() => openNote(absolute)}>
                            {label}
                          </button>
                        </li>
                      ) : (
                        <li key={`${issueFilter}:${item.path}`} className="truncate px-2.5 text-[12px] text-[#8a8a8a]">
                          {label}
                        </li>
                      )
                    })}
                  </ul>
                </div>
              ) : null}
              {notesVisible ? (
                <nav
                  aria-label={t('knowledge.notes.list')}
                  className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
                  {library === 'local' ? (
                    <>
                      <VaultFileTree
                        rootName={t('knowledge.notes.vault')}
                        rootPath={vaultRoot}
                        nodes={noteNodes}
                        names={displayNames}
                        query={query}
                        openedId={conceptPath ? null : openedId}
                        draft={inlineEdit}
                        reloading={refreshingVault}
                        onOpenFile={openNote}
                        onCreate={beginCreate}
                        onRename={beginRename}
                        onReload={refreshLocalVault}
                        onMove={(source, destination) => {
                          void moveNode(source, destination)
                        }}
                        onMenu={setMenu}
                        onDraftName={(name) =>
                          setInlineEdit((current) =>
                            current ? { ...current, name, error: '', touched: true } : current
                          )
                        }
                        onDraftCommit={(shown) => {
                          void commitInline(shown)
                        }}
                        onDraftCancel={() => setInlineEdit(null)}
                      />
                      {moveError ? (
                        <p className="m-0 px-2.5 py-1 text-[12px] text-[#8a8a8a]" role="alert">
                          {moveError}
                        </p>
                      ) : null}
                    </>
                  ) : visible.length === 0 ? (
                    <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]">{t('knowledge.notes.empty')}</p>
                  ) : (
                    visible.map((note) => (
                      <button
                        key={note.id}
                        type="button"
                        aria-current={note.id === openedId ? 'page' : undefined}
                        onClick={() => openNote(note.id)}
                        className={rowClass(note.id === openedId)}>
                        <FileText className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                        <span className="truncate">{displayNames.get(note.path) ?? noteTitle(note.name)}</span>
                      </button>
                    ))
                  )}
                </nav>
              ) : null}
              {conceptsVisible ? (
                <nav
                  aria-label={t('knowledge.notes.concepts')}
                  className={cn(
                    'flex min-h-0 flex-col gap-0.5 overflow-y-auto',
                    notesVisible ? 'mt-2 max-h-48 shrink-0' : 'flex-1'
                  )}>
                  <h2 className="m-0 flex items-center gap-2 px-2.5 pb-1 text-[12px] font-medium text-[#8a8a8a]">
                    <span>{t('knowledge.notes.concepts')}</span>
                    {catalog.concepts.length > 0 ? (
                      <span className="font-normal">{t('knowledge.notes.readonly')}</span>
                    ) : null}
                  </h2>
                  {catalog.concepts.length > 0 ? (
                    <p className="m-0 px-2.5 pb-1 text-[12px] text-[#8a8a8a]">
                      {t('knowledge.notes.concepts_from_compile')}
                    </p>
                  ) : null}
                  {listedConcepts.map((item) => {
                    const path = `${vaultRoot}/${item.file}`
                    return (
                      <button
                        key={item.file}
                        type="button"
                        aria-label={item.title}
                        aria-current={conceptPath === path ? 'page' : undefined}
                        className={rowClass(conceptPath === path)}
                        onClick={() => setConceptPath(path)}>
                        <FileText className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                        <span className="truncate">{item.title}</span>
                      </button>
                    )
                  })}
                  {conceptHits.map((item) => {
                    const path = `${vaultRoot}/${item.file}`
                    return (
                      <button
                        key={item.file}
                        type="button"
                        aria-label={item.title}
                        aria-current={conceptPath === path ? 'page' : undefined}
                        className={rowClass(conceptPath === path)}
                        onClick={() => setConceptPath(path)}>
                        <FileText className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                        <span className="truncate">{item.title}</span>
                      </button>
                    )
                  })}
                  {catalog.concepts.length === 0 && !conceptQuery ? (
                    <p className="m-0 px-2.5 py-1 text-[13px] text-[#8a8a8a]">{t('knowledge.notes.concepts_empty')}</p>
                  ) : null}
                </nav>
              ) : null}
            </aside>
            <section className="flex min-h-0 min-w-0 flex-1 overflow-hidden bg-white">
              <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
                {graphOpen && library === 'local' && (Boolean(conceptPath) || !opened || phase === 'idle') ? (
                  <KnowledgeGraphView
                    notes={readingNotes}
                    openedPath={openedPath}
                    label={t('knowledge.notes.graph')}
                    coverage={
                      coverage.read < coverage.listed
                        ? t('knowledge.notes.index_read', { read: coverage.read, listed: coverage.listed })
                        : null
                    }
                    onOpen={openNote}
                  />
                ) : conceptPath ? (
                  <NoteColumn ready={conceptPhase === 'unavailable' || conceptPhase === 'ready'}>
                    {conceptPhase === 'unavailable' ? (
                      <div role="status" className="mt-8">
                        <h3 className="m-0 text-[16px] font-semibold text-[#1a1a1a]">
                          {t('knowledge.notes.concept_unavailable.title')}
                        </h3>
                        <p className="m-0 mt-2 text-[14px] text-[#8a8a8a]">
                          {t('knowledge.notes.concept_unavailable.description')}
                        </p>
                      </div>
                    ) : conceptPhase === 'ready' && conceptMarkdown ? (
                      <>
                        <p className="m-0 mb-1 px-1.5 text-[13px] leading-none text-[#8a8a8a]">
                          {conceptDocument(conceptMarkdown).type}
                        </p>
                        <NoteTitleField
                          title={conceptDocument(conceptMarkdown).title ?? noteTitle(conceptPath)}
                          editable={false}
                          savedAt={null}
                          onCommit={async () => ''}
                        />
                        <NoteEditor
                          key={conceptPath}
                          fileName={conceptPath.slice(conceptPath.lastIndexOf('/') + 1)}
                          markdown={conceptProse(conceptMarkdown)}
                          preview
                          onDraft={() => undefined}
                          onReady={markEditorReady}
                        />
                      </>
                    ) : (
                      <div className="min-h-0 flex-1" />
                    )}
                  </NoteColumn>
                ) : opened && phase !== 'idle' ? (
                  <NoteColumn
                    ready={phase === 'unavailable' || phase === 'too-large' || (phase === 'ready' && editorReady)}>
                    <NoteTitleField
                      title={customTitle ?? noteTitle(opened.name)}
                      editable={phase === 'ready'}
                      savedAt={savedAtByNote.get(opened.id) ?? null}
                      unsaved={phase === 'ready' && opened.markdown != null && text !== markdownBody(opened.markdown)}
                      onCommit={commitTitle}
                    />
                    {phase === 'unavailable' ? (
                      <div role="status" className="mt-8">
                        <h3 className="m-0 text-[16px] font-semibold text-[#1a1a1a]">
                          {t('knowledge.notes.unavailable.title')}
                        </h3>
                        <p className="m-0 mt-2 text-[14px] text-[#8a8a8a]">
                          {t('knowledge.notes.unavailable.description')}
                        </p>
                      </div>
                    ) : phase === 'too-large' ? (
                      <div role="status" className="mt-8">
                        <h3 className="m-0 text-[16px] font-semibold text-[#1a1a1a]">
                          {t('documents.open.too_large.title')}
                        </h3>
                        <p className="m-0 mt-2 text-[14px] text-[#8a8a8a]">
                          {t('documents.open.too_large.description', { size: formatDocumentSize(oversized) })}
                        </p>
                        <button
                          type="button"
                          onClick={() => {
                            if (!workspaceRoot) return
                            void openWorkspaceFile(workspaceRoot, opened.path).catch(() => undefined)
                          }}
                          className="mt-4 h-[30px] rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] text-[#171717]">
                          {t('file_preview.unsupported.action')}
                        </button>
                      </div>
                    ) : phase === 'ready' ? (
                      <>
                        {createError ? (
                          <p className="m-0 px-1.5 text-[13px] text-[#8a8a8a]" role="alert">
                            {createError}
                          </p>
                        ) : null}
                        <NoteEditor
                          key={`${opened.id}:${reveal?.token ?? 0}`}
                          fileName={opened.name}
                          markdown={text}
                          extensions={editorExtensions}
                          onDraft={setText}
                          onReady={markEditorReady}
                          onSave={save}
                          valueReader={draftReader}
                        />
                      </>
                    ) : (
                      <div className="min-h-0 flex-1" />
                    )}
                  </NoteColumn>
                ) : phase === 'loading' ? (
                  <div className="relative min-h-0 flex-1">
                    <PageEntry id="knowledge.library" ready={false} />
                  </div>
                ) : !vaultListed && library === 'local' ? (
                  <div className="flex flex-1 items-center justify-center px-6">
                    <p role="status" className="m-0 text-[13px] text-[#8a8a8a]">
                      {t('knowledge.notes.vault_loading')}
                    </p>
                  </div>
                ) : continueNotes.length > 0 && library === 'local' ? (
                  <div className="flex flex-1 flex-col items-center justify-center px-6 py-12 text-center">
                    <h3 className="m-0 text-sm font-normal text-muted-foreground">{t('knowledge.notes.continue')}</h3>
                    <nav
                      aria-label={t('knowledge.notes.continue')}
                      className="mt-4 flex w-full max-w-md flex-col gap-1">
                      {continueNotes.map((note) => {
                        const label = displayNames.get(note.path) ?? noteTitle(note.name)
                        const clock = note.modifiedMs == null ? '' : formatSavedClock(new Date(note.modifiedMs))
                        return (
                          <button
                            key={note.path}
                            type="button"
                            className="flex h-9 items-center gap-3 rounded-[8px] px-3 text-left text-[13px] text-[#171717] hover:bg-[#f7f7f7]"
                            onClick={() => openNote(note.path)}>
                            <span className="min-w-0 flex-1 truncate">{label}</span>
                            {clock ? (
                              <time className="shrink-0 text-[12px] text-[#8a8a8a] tabular-nums" aria-hidden>
                                {clock}
                              </time>
                            ) : null}
                          </button>
                        )
                      })}
                    </nav>
                    <div className="mt-4 flex items-center gap-2">
                      <button
                        type="button"
                        className="h-[30px] rounded-[8px] border border-[#ececec] bg-white px-3 text-[13px] text-[#171717]"
                        onClick={() => beginCreate('file', vaultRoot)}>
                        {t('knowledge.notes.new_file')}
                      </button>
                      <button
                        type="button"
                        className="h-[30px] rounded-[8px] border border-[#ececec] bg-white px-3 text-[13px] text-[#171717]"
                        onClick={() => beginCreate('folder', vaultRoot)}>
                        {t('knowledge.notes.new_folder')}
                      </button>
                    </div>
                  </div>
                ) : (
                  <EmptyState
                    illustration="book"
                    title={t('knowledge.notes.pane_empty.title')}
                    description={t('knowledge.notes.pane_empty.description')}
                    actionLabel={library === 'local' ? t('knowledge.notes.new_file') : undefined}
                    onAction={library === 'local' ? () => beginCreate('file', vaultRoot) : undefined}
                    secondaryLabel={library === 'local' ? t('knowledge.notes.new_folder') : undefined}
                    onSecondary={library === 'local' ? () => beginCreate('folder', vaultRoot) : undefined}
                  />
                )}
              </div>
              {graphOpen && library === 'local' && !conceptPath && opened && phase !== 'idle' ? (
                <div className="flex h-full w-[42%] min-w-[320px] shrink-0 border-l border-[#efefef]">
                  <KnowledgeGraphView
                    notes={readingNotes}
                    openedPath={openedPath}
                    label={t('knowledge.notes.graph')}
                    coverage={
                      coverage.read < coverage.listed
                        ? t('knowledge.notes.index_read', { read: coverage.read, listed: coverage.listed })
                        : null
                    }
                    onOpen={openNote}
                  />
                </div>
              ) : null}
              {!graphOpen && conceptPath && conceptPhase === 'ready' && conceptMarkdown ? (
                <ConceptReadingRail
                  markdown={conceptMarkdown}
                  fromFile={bundleRelative(vaultRoot, conceptPath) ?? ''}
                  files={vaultFiles}
                  onOpen={openBundleFile}
                />
              ) : !graphOpen && opened && phase === 'ready' && opened.markdown != null ? (
                <NoteReadingRail
                  notes={readingNotes}
                  openedName={opened.name}
                  markdown={opened.markdown}
                  draft={text}
                  dirty={text !== markdownBody(opened.markdown)}
                  tag={tagFilter}
                  onOpen={openNote}
                  onReveal={(heading) => setReveal({ text: heading, token: Date.now() })}
                  onTag={(item) => setTagFilter((current) => (current === item ? null : item))}
                  onCreate={createUnresolved}
                />
              ) : null}
            </section>
          </>
        )}
      </div>
      {compileRun.session ? <KnowledgeCompileTurn sessionId={compileRun.session.sessionId} /> : null}
      {showCompile ? (
        <KnowledgeCompileDialog
          open={compileOpen}
          phase={compileRun.phase}
          session={compileRun.session}
          compiledCount={libraryCompiled}
          skippedCount={librarySkipped}
          failedCount={libraryFailed}
          accepted={compileRun.accepted}
          issueFilter={issueFilter}
          message={compileRun.error}
          noteTotal={notes.length}
          startedAt={compileRun.startedAt}
          planned={compileRun.planned}
          pending={compileRun.pending}
          workspaceRoot={workspaceRoot ?? ''}
          vaultRoot={vaultRoot}
          onOpenChange={(next) => {
            setCompileOpen(next)
            if (!next) compileRun.reset()
          }}
          onCompile={() => {
            void compileRun.begin()
          }}
          onSettled={compileRun.settle}
          onFailed={compileRun.fail}
          onFilter={(kind) => {
            setIssueFilter((current) => (current === kind ? null : kind))
            setCompileOpen(false)
          }}
        />
      ) : null}
      {wikiSuggest && wikiSuggest.items.length > 0 ? <WikiSuggestMenu state={wikiSuggest} /> : null}
      {menu
        ? createPortal(
            <div
              role="menu"
              aria-label={t('knowledge.notes.menu')}
              data-knowledge-menu="true"
              className="fixed z-50 w-44 rounded-[10px] border border-[#ececec] bg-white p-1 shadow-[0_8px_18px_rgb(0_0_0/0.08)]"
              style={{ left: Math.max(8, menu.x), top: Math.max(8, menu.y) }}
              onPointerDown={(event) => event.stopPropagation()}>
              {menu.kind === 'file' ? (
                <VaultMenuButton
                  icon={<PanelRightOpen className="size-4" strokeWidth={1.7} />}
                  label={t('knowledge.notes.open')}
                  onClick={() => {
                    openNote(menu.path)
                    setMenu(null)
                  }}
                />
              ) : null}
              <VaultMenuButton
                icon={<FilePlus className="size-4" strokeWidth={1.7} />}
                label={t('knowledge.notes.new_file')}
                onClick={() => beginCreate('file', menu.kind === 'file' ? parentDir(menu.path) : menu.path)}
              />
              <VaultMenuButton
                icon={<FolderPlus className="size-4" strokeWidth={1.7} />}
                label={t('knowledge.notes.new_folder')}
                onClick={() => beginCreate('folder', menu.kind === 'file' ? parentDir(menu.path) : menu.path)}
              />
              {menu.kind === 'directory' || menu.kind === 'file' ? (
                <VaultMenuButton
                  icon={<Pencil className="size-4" strokeWidth={1.7} />}
                  label={t('knowledge.notes.rename')}
                  onClick={() => beginRename(menu.path)}
                />
              ) : null}
              {menu.kind === 'root' ? null : (
                <PopConfirm
                  className="flex w-full"
                  tone="danger"
                  title={t('knowledge.notes.delete_title')}
                  description={
                    deleteError ||
                    `${menu.path.slice(menu.path.lastIndexOf('/') + 1)} ${t('knowledge.notes.delete_hint')}`
                  }
                  confirmLabel={t('knowledge.notes.delete')}
                  cancelLabel={t('common.cancel')}
                  onOpenChange={(open) => {
                    if (!open) setMenu(null)
                  }}
                  onConfirm={() => confirmDelete(menu.path)}>
                  <VaultMenuButton
                    icon={<Trash2 className="size-4" strokeWidth={1.7} />}
                    label={t('knowledge.notes.delete')}
                    onClick={() => setDeleteError('')}
                  />
                </PopConfirm>
              )}
            </div>,
            document.body
          )
        : null}
    </div>
  )
}
