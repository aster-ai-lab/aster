import { EmptyState, PopConfirm, Tooltip } from '@a3s-lab/ui'
import {
  ArrowUp,
  BookOpen,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Clock,
  Download,
  ExternalLink,
  FilePlus,
  FolderPlus,
  FolderTree,
  Pencil,
  RefreshCw,
  ScanText,
  Search,
  Trash2
} from 'lucide-react'
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type DragEvent,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode
} from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { useRightPanelPresentationMaximized } from '@renderer/components/chat/panes/Shell'
import { FileTreeSearchField } from '@renderer/components/FileTreeSearchField'
import { DocumentFileIcon, DocumentFolderIcon, documentFileIconType } from '@renderer/components/icons/DocumentFileIcon'
import ImageViewer from '@renderer/components/ImageViewer'
import { driveApi } from '@renderer/drive/driveTransport'
import { PAGE_SIDEBAR_DEFAULT_WIDTH, pageSidebarEdgeClassName } from '@renderer/layout/pageSidebar'
import { useWorkspaceImageSrc } from '@renderer/pages/agents/workspaceImagePreview'
import { blankOfficeFileBytes, type OfficeCreateKind } from '@renderer/pages/documents/blankOfficeFile'
import { isCloudWorkspace } from '@renderer/pages/documents/cloudWorkspace'
import { formatDocumentActivity, formatDocumentSize } from '@renderer/pages/documents/documentLibrary'
import { DocumentPipelineStatus } from '@renderer/pages/documents/DocumentPipelineStatus'
import { fileExtension, isWorkspaceFileEditorKind, previewKind } from '@renderer/pages/documents/documentPreview'
import { DriveFileCheck, DriveHandoffButton, DriveUploadButton } from '@renderer/pages/documents/DriveListActions'
import { OfficeCreateMenu } from '@renderer/pages/documents/OfficeCreateMenu'
import type { OfficeCreateOutcome } from '@renderer/pages/documents/officeCreateName'
import { openFileEditorWindow } from '@renderer/pages/documents/openFileEditorWindow'
import { TeamFileEncyclopediaDialog } from '@renderer/pages/documents/TeamFileEncyclopediaDialog'
import { TeamPipelineQueue } from '@renderer/pages/documents/TeamPipelineQueue'
import {
  createLocalEntry,
  importDroppedFiles,
  importLocalFile,
  listLocalEntries,
  localEntryStatus,
  moveLocalEntry,
  openWorkspaceFile,
  readLocalDocumentBytes,
  removeLocalEntry,
  writeLocalDocumentBytes,
  renameLocalEntry,
  searchWorkspaceEntries,
  subscribeLocalDocumentChanges,
  type LocalDocumentChange,
  type LocalEntry,
  type WorkspaceSearchHit
} from '@renderer/pages/documents/useLocalDocuments'
import { cn } from '@renderer/utils/style'

import { queueWorkbenchComposer } from './workbenchComposerBridge'
import { WorkbenchFileEditor } from './WorkbenchFileEditor'
import { useOptionalWorkbenchWorkspaceTabs } from './workbenchWorkspaceTabs'

const RECENT_SKIP = new Set(['.git', 'node_modules', 'target', 'dist', '.next', 'coverage'])
const RECENT_LIMIT = 8
const RECENT_DEPTH = 4
const RECENT_BEAM = 8

type Branch = {
  capped: boolean
  entries: LocalEntry[]
  message?: string
  phase: 'loading' | 'ready' | 'error'
}

type InlineEdit =
  | {
      mode: 'create'
      kind: 'file' | 'folder'
      parent: string
      name: string
      error: string
      busy: boolean
      touched: boolean
    }
  | {
      mode: 'rename'
      path: string
      name: string
      error: string
      busy: boolean
      touched: boolean
    }

function compileInProgress(status: LocalEntry['compileStatus']): boolean {
  return status === 'waiting' || status === 'queued' || status === 'running'
}

function TreeDisclosure({ open, label, onClick }: { open: boolean; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-expanded={open}
      aria-label={label}
      data-workspace-session-disclosure="true"
      className="grid size-4 shrink-0 place-items-center rounded-[4px] text-[#8a8a8a] outline-none hover:bg-[#f5f5f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10"
      onPointerDown={(event) => event.stopPropagation()}
      onClick={(event) => {
        event.preventDefault()
        event.stopPropagation()
        onClick()
      }}>
      <ChevronRight
        className={cn('size-3.5 shrink-0 text-current transition-transform', open && 'rotate-90')}
        strokeWidth={1.7}
        aria-hidden
      />
    </button>
  )
}

function navClass(selected: boolean, depth: number) {
  return cn(
    'flex w-full items-center gap-2.5 rounded-[8px] text-left text-[13px] transition-colors outline-none',
    depth === 0 ? 'h-9 px-2.5' : 'h-8 pr-2.5',
    'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
    selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
  )
}

function indentStyle(depth: number): CSSProperties | undefined {
  if (depth <= 0) return undefined
  return { paddingInlineStart: `${32 + (depth - 1) * 14}px` }
}

function parentOf(path: string, root: string): string {
  const normalized = path.replace(/[/\\]+$/u, '')
  const index = Math.max(normalized.lastIndexOf('/'), normalized.lastIndexOf('\\'))
  if (index <= 0) return root
  const parent = normalized.slice(0, index)
  return parent.length < root.length ? root : parent
}

function directoryCrumbs(directory: string, root: string): Array<{ label: string; path: string }> {
  if (!root || !directory || directory === root) return []
  const separator = directory.includes('\\') && !directory.includes('/') ? '\\' : '/'
  const prefix = root.replace(/[/\\]+$/u, '') + separator
  if (!directory.startsWith(prefix)) return []
  const segments = directory.slice(prefix.length).split(/[/\\]/u).filter(Boolean)
  return segments.map((name, index) => ({
    label: name,
    path: prefix + segments.slice(0, index + 1).join(separator)
  }))
}

function folderAttr(path: string, root: string): string {
  const parent = parentOf(path, root)
  if (parent === root) return ''
  return parent.split(/[/\\]/u).pop() ?? ''
}

function invalidName(name: string): 'empty' | 'separator' | 'hidden' | 'bundle' | undefined {
  const trimmed = name.trim()
  if (!trimmed || trimmed === '.' || trimmed === '..') return 'empty'
  if (trimmed.includes('/') || trimmed.includes('\\')) return 'separator'
  if (trimmed.startsWith('.')) return 'hidden'
  if (trimmed.toLowerCase() === 'wiki') return 'bundle'
  return undefined
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

const TREE_ICON = 'h-5 w-4'

function previewDialogEvent(event: { target: EventTarget | null }): boolean {
  return (
    event.target instanceof Element &&
    Boolean(event.target.closest('[data-testid="image-preview-dialog"], [data-slot="dialog-overlay"]'))
  )
}

/** Raster pictures use the message image preview. SVG stays a source file. */
function opensWithMessageImagePreview(name: string): boolean {
  return previewKind(name) === 'image' && fileExtension(name) !== 'svg'
}

function EntryIcon({
  entry,
  imageTicket,
  onOpenImage,
  open = false,
  pendingImagePath,
  root,
  className = TREE_ICON
}: {
  entry: { kind: string; name: string; path?: string }
  imageTicket: number
  onOpenImage: (image: { alt: string; id: string; src: string }) => void
  open?: boolean
  pendingImagePath: { current: string | null }
  root: string
  className?: string
}) {
  const path = entry.kind === 'file' ? entry.path : undefined
  const src = useWorkspaceImageSrc(root, path, entry.name)
  const [broken, setBroken] = useState(false)
  useEffect(() => {
    setBroken(false)
  }, [src])
  useEffect(() => {
    if (!src || !path || pendingImagePath.current !== path) return
    pendingImagePath.current = null
    onOpenImage({ alt: entry.name, id: path, src })
  }, [entry.name, imageTicket, onOpenImage, path, pendingImagePath, src])
  if (entry.kind === 'directory') return <DocumentFolderIcon open={open} className={className} />
  if (src && path && !broken) {
    return (
      <span className={cn(className, 'relative inline-block shrink-0 overflow-hidden rounded-[3px]')}>
        <img
          alt=""
          className="absolute inset-0 size-full object-contain"
          data-workspace-image={path}
          decoding="async"
          draggable={false}
          onClick={(event) => {
            event.stopPropagation()
            onOpenImage({ alt: entry.name, id: path, src })
          }}
          onDoubleClick={(event) => event.stopPropagation()}
          onError={() => setBroken(true)}
          src={src}
        />
      </span>
    )
  }
  return <DocumentFileIcon type={documentFileIconType(entry.name)} className={className} />
}

/**
 * Session-folder file manager for the task workspace.
 * Rows use the documents sidebar chrome. Browsing, search, create, rename,
 * move, and delete follow the work workspace file tree, rooted at this session.
 * Disk changes from the terminal, the agent, or another app arrive on one
 * shell event stream. Open directories are re-listed in place.
 */
export function WorkbenchSessionFiles({
  root,
  layout = 'pane',
  rootLabel,
  omitRootNames
}: {
  root?: string
  /** `split` is the full-page file manager: folder tree beside the file list. */
  layout?: 'pane' | 'split'
  rootLabel?: string
  /** Directory names hidden at the workspace root. The compiled encyclopedia lives in `wiki`. */
  omitRootNames?: readonly string[]
}) {
  const { t } = useTranslation()
  const paneMaximized = useRightPanelPresentationMaximized()
  const maximized = layout === 'split' || paneMaximized
  const workspaceTabs = useOptionalWorkbenchWorkspaceTabs()
  const [listSort, setListSort] = useState<'asc' | 'desc'>('desc')
  const [listSection, setListSection] = useState<'folder' | 'recent'>('folder')
  const workspace = root?.trim() ?? ''
  const label = rootLabel ?? t('workbench.workspace.session_root', '会话工作区')
  const [openedFile, setOpenedFile] = useState<{ path: string; name: string } | null>(null)
  const pendingImagePath = useRef<string | null>(null)
  const [imageTicket, setImageTicket] = useState(0)
  const [imagePreview, setImagePreview] = useState<{ alt: string; id: string; src: string } | null>(null)
  const openMessageImage = useCallback((image: { alt: string; id: string; src: string }) => {
    setImagePreview(image)
  }, [])
  const [branches, setBranches] = useState<Map<string, Branch>>(() => new Map())
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set())
  const [treeOpen, setTreeOpen] = useState(true)
  const [recentOpen, setRecentOpen] = useState(true)
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [checkedPaths, setCheckedPaths] = useState<Set<string>>(() => new Set())
  const [query, setQuery] = useState('')
  const [diskTick, setDiskTick] = useState(0)
  const [searchHits, setSearchHits] = useState<WorkspaceSearchHit[]>([])
  const [searchPhase, setSearchPhase] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle')
  const [recents, setRecents] = useState<LocalEntry[]>([])
  const [recentsPhase, setRecentsPhase] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle')
  const [nav, setNav] = useState<{ history: string[]; index: number }>({ history: [], index: 0 })
  const [inlineEdit, setInlineEdit] = useState<InlineEdit | null>(null)
  const [menu, setMenu] = useState<{ path?: string; x: number; y: number; tree?: boolean } | null>(null)
  const [pendingDelete, setPendingDelete] = useState<string | null>(null)
  const [deleteError, setDeleteError] = useState('')
  const deleteAnchor = useRef({ top: 96, left: 24, width: 180, height: 28 })
  const [notice, setNotice] = useState('')
  const [officeCreateBusy, setOfficeCreateBusy] = useState(false)
  const officeCreateLock = useRef(false)
  const [dropTarget, setDropTarget] = useState<string | null>(null)
  const [externalDrop, setExternalDrop] = useState<string | null>(null)
  const externalDepth = useRef(0)
  const [ghost, setGhost] = useState<{ label: string; x: number; y: number } | null>(null)
  const controllersRef = useRef(new Map<string, AbortController>())
  const recentsEpoch = useRef(0)
  const autoRef = useRef(new Set<string>())
  const expandedRef = useRef(expanded)
  const branchesRef = useRef(branches)
  const selectedRef = useRef(selectedPath)
  const queryRef = useRef(query)
  const blurMode = useRef<'blur' | 'submit' | 'cancel'>('blur')
  const inlineOpenedAt = useRef(0)
  const committingRef = useRef(false)
  const skipClick = useRef(false)
  const suppressNav = useRef(false)
  const navWorkspace = useRef(workspace)
  const dragStop = useRef<(() => void) | null>(null)
  const moveNodeRef = useRef<(source: string, destination: string) => void>(() => {})
  expandedRef.current = expanded
  branchesRef.current = branches
  selectedRef.current = selectedPath
  queryRef.current = query

  const draggedClick = () => {
    if (!skipClick.current) return false
    skipClick.current = false
    return true
  }

  const destinationAt = (x: number, y: number) => {
    const hit = document.elementFromPoint(x, y)
    if (!(hit instanceof Element)) return null
    return hit.closest('[data-workspace-drop]')?.getAttribute('data-workspace-drop') || null
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
      setExpanded((current) => new Set(current).add(destination))
      moveNodeRef.current(path, destination)
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

  useEffect(
    () => () => {
      dragStop.current?.()
      document.body.style.cursor = ''
    },
    []
  )

  const abortAll = useCallback(() => {
    for (const controller of controllersRef.current.values()) controller.abort()
    controllersRef.current.clear()
  }, [])

  const loadBranch = useCallback(
    (path: string, mode: 'show' | 'quiet' | 'resync' = 'show') => {
      if (!workspace || !path) return
      controllersRef.current.get(path)?.abort()
      const controller = new AbortController()
      controllersRef.current.set(path, controller)
      setBranches((current) => {
        const previous = current.get(path)
        const preserve = mode !== 'show' && previous?.phase === 'ready'
        const next = new Map(current)
        next.set(path, {
          capped: previous?.capped ?? false,
          entries: previous?.entries ?? [],
          phase: preserve ? 'ready' : 'loading'
        })
        return next
      })
      return listLocalEntries(workspace, path, controller.signal).then(
        (listing) => {
          if (controller.signal.aborted) return
          const entries =
            path === workspace && omitRootNames?.length
              ? listing.entries.filter((entry) => !omitRootNames.includes(entry.name))
              : listing.entries
          setBranches((current) => {
            const next = new Map(current)
            next.set(path, { capped: listing.capped, entries, phase: 'ready' })
            return next
          })
          setExpanded((current) => pruneExpanded(current, path, entries))
          if (mode === 'quiet') {
            recentsEpoch.current += 1
            setRecents((current) => {
              const next = mergeDirectRecents(current, path, listing.entries)
              return sameRecents(current, next) ? current : next
            })
          }
          if (path !== workspace) return
          for (const entry of entries) {
            if (entry.kind !== 'directory' || autoRef.current.has(entry.path)) continue
            autoRef.current.add(entry.path)
            setExpanded((current) => new Set(current).add(entry.path))
            loadBranch(entry.path)
          }
        },
        (error: unknown) => {
          if (controller.signal.aborted) return
          setBranches((current) => {
            const previous = current.get(path)
            const next = new Map(current)
            next.set(path, {
              capped: false,
              entries: previous?.entries ?? [],
              message: error instanceof Error ? error.message : '',
              phase: 'error'
            })
            return next
          })
        }
      )
    },
    [omitRootNames, workspace]
  )

  const refreshTicket = useRef(0)
  const [refreshing, setRefreshing] = useState(false)
  const reloadVisible = () => {
    if (!workspace) return
    const ticket = ++refreshTicket.current
    setRefreshing(true)
    const directory = browseDirectoryFor(selectedRef.current, branchesRef.current, workspace)
    const jobs = [loadBranch(directory, 'show')]
    for (const path of expandedRef.current) {
      if (path !== directory) jobs.push(loadBranch(path, 'quiet'))
    }
    const recent = new AbortController()
    controllersRef.current.get('recent')?.abort()
    controllersRef.current.set('recent', recent)
    jobs.push(
      loadRecents(recent.signal).catch(() => {
        if (!recent.signal.aborted) setRecentsPhase('error')
      })
    )
    void Promise.all(jobs).finally(() => {
      if (ticket === refreshTicket.current) setRefreshing(false)
    })
  }

  const loadRecents = useCallback(
    async (signal: AbortSignal) => {
      if (!workspace) return
      const epoch = recentsEpoch.current
      const stale = () => signal.aborted || recentsEpoch.current !== epoch
      const files: LocalEntry[] = []
      let frontier = [workspace]
      for (let depth = 0; depth < RECENT_DEPTH && frontier.length > 0; depth += 1) {
        if (stale()) return
        const batch = frontier.slice(0, RECENT_BEAM)
        const listings = await Promise.all(
          batch.map(async (path) => {
            try {
              return await listLocalEntries(workspace, path, signal)
            } catch {
              return undefined
            }
          })
        )
        if (stale()) return
        const next: LocalEntry[] = []
        for (const listing of listings) {
          if (!listing) continue
          for (const entry of listing.entries) {
            if (entry.kind === 'file') files.push(entry)
            else if (!RECENT_SKIP.has(entry.name)) next.push(entry)
          }
        }
        files.sort((left, right) => right.modifiedMs - left.modifiedMs)
        if (stale()) return
        setRecents(files.slice(0, RECENT_LIMIT))
        setRecentsPhase('ready')
        frontier = next.slice(0, RECENT_BEAM).map((entry) => entry.path)
      }
    },
    [workspace]
  )

  useEffect(() => {
    abortAll()
    autoRef.current = new Set()
    setBranches(new Map())
    setExpanded(new Set())
    setSelectedPath(null)
    setListSection('folder')
    setInlineEdit(null)
    setMenu(null)
    setRecents([])
    setNotice('')
    if (!workspace) {
      setRecentsPhase('idle')
      return
    }
    loadBranch(workspace)
    if (layout === 'split') {
      setRecentsPhase('idle')
      return () => abortAll()
    }
    setRecentsPhase('loading')
    const recentController = new AbortController()
    controllersRef.current.set('recent', recentController)
    void loadRecents(recentController.signal).catch(() => {
      if (!recentController.signal.aborted) setRecentsPhase('error')
    })
    return () => abortAll()
  }, [abortAll, layout, loadBranch, loadRecents, workspace])

  useEffect(() => {
    if (!workspace) return
    const controller = new AbortController()
    let after = 0
    let stopped = false

    const refreshVisible = (mode: 'quiet' | 'resync') => {
      loadBranch(workspace, mode)
      for (const path of expandedRef.current) loadBranch(path, mode)
      const browse = browseDirectoryFor(selectedRef.current, branchesRef.current, workspace)
      if (browse && browse !== workspace) loadBranch(browse, mode)
    }

    const touchSearch = () => {
      if (queryRef.current.trim()) setDiskTick((value) => value + 1)
    }

    const applyChange = (change: LocalDocumentChange) => {
      if (!change.fullScan && change.paths.length === 0) return
      if (change.fullScan) {
        refreshVisible('resync')
        const recent = new AbortController()
        controllersRef.current.get('recent')?.abort()
        controllersRef.current.set('recent', recent)
        void loadRecents(recent.signal).catch(() => {
          if (!recent.signal.aborted) setRecentsPhase('error')
        })
      } else {
        const targets = refreshTargets(workspace, change.paths, branchesRef.current)
        if (targets.length === 0) refreshVisible('quiet')
        else for (const path of targets) loadBranch(path, 'quiet')
      }
      touchSearch()
    }

    const run = async () => {
      let failures = 0
      while (!stopped) {
        try {
          await subscribeLocalDocumentChanges(workspace, after, controller.signal, (change) => {
            if (stopped) return
            after = change.revision
            failures = 0
            applyChange(change)
          })
          if (stopped) return
          await sleep(1000, controller.signal)
        } catch (error) {
          if (stopped || isAbort(error)) return
          failures += 1
          try {
            await sleep(2000, controller.signal)
          } catch {
            return
          }
          if (stopped) return
          if (failures >= 2) refreshVisible('quiet')
        }
      }
    }

    void run()
    return () => {
      stopped = true
      controller.abort()
    }
  }, [loadBranch, loadRecents, workspace])

  useEffect(() => {
    const trimmed = query.trim()
    if (!workspace || !trimmed) {
      setSearchHits([])
      setSearchPhase('idle')
      return
    }
    setSearchPhase('loading')
    const controller = new AbortController()
    const timer = window.setTimeout(() => {
      void searchWorkspaceEntries(workspace, trimmed, controller.signal).then(
        (hits) => {
          if (controller.signal.aborted) return
          setSearchHits(hits)
          setSearchPhase('ready')
        },
        () => {
          if (controller.signal.aborted) return
          setSearchHits([])
          setSearchPhase('error')
        }
      )
    }, 180)
    return () => {
      window.clearTimeout(timer)
      controller.abort()
    }
  }, [diskTick, query, workspace])

  useEffect(() => {
    if (!maximized || !workspace) return
    const directory = browseDirectoryFor(selectedPath, branches, workspace)
    if (!directory || branches.get(directory)) return
    loadBranch(directory)
  }, [branches, loadBranch, maximized, selectedPath, workspace])

  useEffect(() => {
    if (!menu) return
    const close = (event: PointerEvent) => {
      const target = event.target
      if (target instanceof Element && target.closest('[data-workspace-session-menu], .ui-popconfirm')) return
      setMenu(null)
    }
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setMenu(null)
    }
    window.addEventListener('pointerdown', close)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('pointerdown', close)
      window.removeEventListener('keydown', onKey)
    }
  }, [menu])

  const siblingNames = (parent: string) => branches.get(parent)?.entries.map((entry) => entry.name) ?? []

  const beginCreate = (kind: 'file' | 'folder', parentPath = '') => {
    if (!workspace) return
    const selected = selectedPath ? findEntry(branches, selectedPath) : undefined
    const parent =
      parentPath || (selected?.kind === 'directory' ? selected.path : parentOf(selectedPath ?? workspace, workspace))
    if (parent && parent !== workspace) {
      setExpanded((current) => new Set(current).add(parent))
      if (branches.get(parent)?.phase !== 'ready') loadBranch(parent)
    }
    if (maximized) {
      setListSection('folder')
      setSelectedPath(parent === workspace ? null : parent)
    }
    setTreeOpen(true)
    inlineOpenedAt.current = Date.now()
    const base =
      kind === 'file'
        ? t('workbench.workspace.new_file_name', '未命名.txt')
        : t('workbench.workspace.new_folder_name', '未命名文件夹')
    setInlineEdit({
      mode: 'create',
      kind,
      parent: parent || workspace,
      name: nextName(siblingNames(parent || workspace), base),
      error: '',
      busy: false,
      touched: false
    })
    setMenu(null)
    setNotice('')
  }

  const beginRename = (path: string) => {
    setSelectedPath(path)
    inlineOpenedAt.current = Date.now()
    setInlineEdit({
      mode: 'rename',
      path,
      name: path.split(/[/\\]/u).pop() ?? path,
      error: '',
      busy: false,
      touched: false
    })
    setMenu(null)
  }

  const reloadParent = (path: string) => {
    const parent = parentOf(path, workspace)
    loadBranch(parent || workspace)
  }

  const commitInline = async (source: 'submit' | 'blur' = 'submit', shown?: string) => {
    if (!inlineEdit || inlineEdit.busy || committingRef.current || !workspace) return
    const name = (shown ?? inlineEdit.name).trim()
    const invalid = invalidName(name)
    if (source === 'blur' && (Boolean(invalid) || !inlineEdit.touched)) {
      setInlineEdit(null)
      return
    }
    if (invalid === 'empty') {
      setInlineEdit({ ...inlineEdit, error: t('workbench.workspace.name_empty', '名称不能为空。') })
      return
    }
    if (invalid === 'bundle') {
      setInlineEdit({
        ...inlineEdit,
        error: t('workbench.workspace.name_reserved', '不能使用百科目录名 wiki。')
      })
      return
    }
    if (invalid === 'separator' || invalid === 'hidden') {
      setInlineEdit({
        ...inlineEdit,
        error: t('workbench.workspace.name_invalid', '名称不能包含路径分隔符或隐藏前缀。')
      })
      return
    }
    const parent = inlineEdit.mode === 'create' ? inlineEdit.parent : parentOf(inlineEdit.path, workspace)
    const currentName = inlineEdit.mode === 'rename' ? (inlineEdit.path.split(/[/\\]/u).pop() ?? '') : ''
    if (siblingNames(parent).includes(name) && (inlineEdit.mode === 'create' || name !== currentName)) {
      setInlineEdit({ ...inlineEdit, error: t('workbench.workspace.name_taken', '当前位置已存在同名文件或文件夹。') })
      return
    }
    if (inlineEdit.mode === 'rename' && name === currentName) {
      setInlineEdit(null)
      return
    }
    setInlineEdit({ ...inlineEdit, busy: true, error: '' })
    committingRef.current = true
    try {
      if (inlineEdit.mode === 'rename') {
        const renamed = await renameLocalEntry(workspace, inlineEdit.path, name)
        setExpanded((current) => replaceExpanded(current, inlineEdit.path, renamed.path))
        setSelectedPath(renamed.path)
        loadBranch(parent)
        if (renamed.kind === 'directory') loadBranch(renamed.path)
      } else {
        const created = await createLocalEntry(
          workspace,
          parent === workspace ? workspace : parent,
          name,
          inlineEdit.kind
        )
        if (parent !== workspace) setExpanded((current) => new Set(current).add(parent))
        setSelectedPath(created.path)
        loadBranch(parent || workspace)
      }
      setInlineEdit(null)
      setNotice('')
    } catch (error) {
      const taken = localEntryStatus(error) === 409
      setInlineEdit((current) =>
        current
          ? {
              ...current,
              busy: false,
              error: taken
                ? t('workbench.workspace.name_taken', '当前位置已存在同名文件或文件夹。')
                : t('workbench.workspace.mutate_failed', '这个操作没有完成。')
            }
          : current
      )
    } finally {
      committingRef.current = false
    }
  }

  const cancelInline = () => {
    blurMode.current = 'cancel'
    setInlineEdit(null)
  }

  const confirmDelete = async (path: string) => {
    if (!path || !workspace) return
    setDeleteError('')
    try {
      await removeLocalEntry(workspace, path)
      setSelectedPath((current) => (current && (current === path || current.startsWith(`${path}/`)) ? null : current))
      setExpanded((current) => {
        const next = new Set<string>()
        for (const item of current) {
          if (item !== path && !item.startsWith(`${path}/`)) next.add(item)
        }
        return next
      })
      reloadParent(path)
    } catch (error) {
      setDeleteError(t('workbench.workspace.mutate_failed', '这个操作没有完成。'))
      throw error
    }
  }

  const moveNode = async (sourcePath: string, destination: string) => {
    if (!workspace || !sourcePath || parentOf(sourcePath, workspace) === destination) return
    if (destination === sourcePath || destination.startsWith(`${sourcePath}/`)) {
      setNotice(t('workbench.workspace.move_into_self', '不能把文件夹移动到自身或其子文件夹。'))
      return
    }
    try {
      const moved = await moveLocalEntry(workspace, sourcePath, destination)
      if (destination !== workspace) setExpanded((current) => new Set(current).add(destination))
      setSelectedPath(moved.path)
      loadBranch(parentOf(sourcePath, workspace))
      if (destination !== parentOf(sourcePath, workspace)) loadBranch(destination)
      if (moved.kind === 'directory') loadBranch(moved.path)
      setNotice('')
    } catch (error) {
      const taken = localEntryStatus(error) === 409
      setNotice(
        taken
          ? t('workbench.workspace.name_taken', '当前位置已存在同名文件或文件夹。')
          : t('workbench.workspace.mutate_failed', '这个操作没有完成。')
      )
    }
  }
  moveNodeRef.current = moveNode

  const openWorkspaceImage = (path: string) => {
    pendingImagePath.current = path
    setImageTicket((value) => value + 1)
  }

  const saveCopy = async (entry: LocalEntry) => {
    setMenu(null)
    setNotice('')
    try {
      const bytes = await readLocalDocumentBytes(workspace, entry.path)
      const saved = await window.api.file.save(entry.name, bytes)
      if (navWorkspace.current === workspace && saved) setNotice(`${t('common.saved')} · ${saved}`)
    } catch {
      if (navWorkspace.current === workspace) setNotice(t('common.save_failed'))
    }
  }

  const openInWorkspace = (path: string, name: string, fileId?: string) => {
    if (!workspace) return
    if (opensWithMessageImagePreview(name)) {
      openWorkspaceImage(path)
      return
    }
    if (layout === 'split') {
      if (!isWorkspaceFileEditorKind(previewKind(name))) {
        void openWorkspaceFile(workspace, path).catch(() => undefined)
        return
      }
      void openFileEditorWindow({ root: workspace, path, title: name, fileId }).then((opened) => {
        if (!opened) setOpenedFile({ path, name })
      })
      return
    }
    if (previewKind(name) !== 'unsupported') {
      workspaceTabs?.openEditor(path, name)
      return
    }
    void openWorkspaceFile(workspace, path).catch(() => undefined)
  }

  const toggleDirectory = (path: string) => {
    setExpanded((current) => {
      const next = new Set(current)
      if (next.has(path)) next.delete(path)
      else {
        next.add(path)
        loadBranch(path)
      }
      return next
    })
  }

  const visiblePaths = (): string[] => {
    const paths: string[] = []
    const walk = (directory: string) => {
      for (const entry of branches.get(directory)?.entries ?? []) {
        paths.push(entry.path)
        if (entry.kind === 'directory' && expanded.has(entry.path)) walk(entry.path)
      }
    }
    if (treeOpen) walk(workspace)
    return paths
  }

  const onTreeKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return
    if (inlineEdit || menu || pendingDelete) return
    if (event.key === 'F2' && selectedPath) {
      event.preventDefault()
      beginRename(selectedPath)
      return
    }
    if ((event.key === 'Delete' || event.key === 'Backspace') && selectedPath) {
      event.preventDefault()
      const row = event.currentTarget.querySelector(`button[title="${CSS.escape(selectedPath)}"]`)
      const rect = row?.getBoundingClientRect()
      if (rect)
        deleteAnchor.current = { top: rect.top, left: rect.left, width: Math.max(rect.width, 28), height: rect.height }
      setDeleteError('')
      setPendingDelete(selectedPath)
      return
    }
    const paths = visiblePaths()
    const index = selectedPath ? paths.indexOf(selectedPath) : -1
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      if (paths.length === 0) return
      event.preventDefault()
      const next =
        event.key === 'ArrowDown'
          ? (paths[Math.min(paths.length - 1, index + 1)] ?? paths[0])
          : paths[Math.max(0, index < 0 ? 0 : index - 1)]
      if (next) setSelectedPath(next)
      return
    }
    const selected = selectedPath ? findEntry(branches, selectedPath) : undefined
    if (event.key === 'ArrowRight' && selected?.kind === 'directory') {
      event.preventDefault()
      setExpanded((current) => new Set(current).add(selected.path))
      loadBranch(selected.path)
      return
    }
    if (event.key === 'ArrowLeft' && selectedPath) {
      event.preventDefault()
      if (selected?.kind === 'directory' && expanded.has(selected.path)) {
        setExpanded((current) => {
          const next = new Set(current)
          next.delete(selected.path)
          return next
        })
        return
      }
      const parent = parentOf(selectedPath, workspace)
      setSelectedPath(parent === workspace ? null : parent)
      return
    }
    if (event.key === 'Enter' && selected) {
      event.preventDefault()
      if (selected.kind === 'file') openInWorkspace(selected.path, selected.name)
      else toggleDirectory(selected.path)
    }
  }

  const onListedFileKeyDown = (event: KeyboardEvent<HTMLButtonElement>, path: string) => {
    // Split layout lists files only in the table. The folder tree's F2 uses the
    // selection, and activating the tree root clears that selection.
    if (inlineEdit || menu || pendingDelete) return
    if (event.key === 'F2') {
      event.preventDefault()
      beginRename(path)
      return
    }
    if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
      event.preventDefault()
      const rect = event.currentTarget.getBoundingClientRect()
      setSelectedPath(path)
      setMenu({ path, x: rect.left + 8, y: rect.bottom + 4 })
      return
    }
    if (event.key !== 'Delete' && event.key !== 'Backspace') return
    event.preventDefault()
    const rect = event.currentTarget.getBoundingClientRect()
    deleteAnchor.current = {
      top: rect.top,
      left: rect.left,
      width: Math.max(rect.width, 28),
      height: rect.height
    }
    setDeleteError('')
    setPendingDelete(path)
  }

  const searching = query.trim().length > 0
  const rootBranch = workspace ? branches.get(workspace) : undefined
  const showEmpty =
    Boolean(workspace) &&
    treeOpen &&
    !searching &&
    rootBranch?.phase === 'ready' &&
    rootBranch.entries.length === 0 &&
    inlineEdit?.parent !== workspace

  const renderInline = (edit: InlineEdit, depth: number, folder: boolean) => (
    <form
      key={edit.mode === 'rename' ? edit.path : `create:${edit.parent}`}
      className={navClass(true, depth)}
      style={indentStyle(depth)}
      data-workspace-session-inline={edit.mode}
      onSubmit={(event) => {
        event.preventDefault()
        blurMode.current = 'submit'
        const field = event.currentTarget.querySelector('input')
        void commitInline('submit', field instanceof HTMLInputElement ? field.value : undefined)
      }}
      onBlur={(event) => {
        const next = event.relatedTarget
        if (next instanceof Node && event.currentTarget.contains(next)) return
        const mode = blurMode.current
        blurMode.current = 'blur'
        if (mode !== 'blur') return
        if (Date.now() - inlineOpenedAt.current < 300) return
        const field = event.currentTarget.querySelector('input')
        void commitInline('blur', field instanceof HTMLInputElement ? field.value : undefined)
      }}>
      {folder ? (
        <DocumentFolderIcon className={TREE_ICON} />
      ) : (
        <DocumentFileIcon type={documentFileIconType(edit.name)} className={TREE_ICON} />
      )}
      <input
        aria-label={
          edit.mode === 'rename' ? t('workbench.workspace.rename', '重命名') : t('workbench.workspace.name', '名称')
        }
        disabled={edit.busy}
        spellCheck={false}
        value={edit.name}
        className="min-w-0 flex-1 bg-transparent text-[13px] text-[#171717] outline-none"
        onChange={(event) => setInlineEdit({ ...edit, name: event.currentTarget.value, error: '', touched: true })}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault()
            event.stopPropagation()
            cancelInline()
          }
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
      {edit.error ? (
        <span className="sr-only" role="alert">
          {edit.error}
        </span>
      ) : null}
    </form>
  )

  const browseDirectory = browseDirectoryFor(selectedPath, branches, workspace)

  const createOfficeFile = async (kind: OfficeCreateKind, fileName: string): Promise<OfficeCreateOutcome> => {
    if (!workspace || officeCreateLock.current) return 'failed'
    officeCreateLock.current = true
    setOfficeCreateBusy(true)
    setNotice('')
    const parent = browseDirectory || workspace
    try {
      const bytes = await blankOfficeFileBytes(kind)
      const created = await createLocalEntry(workspace, parent, fileName, 'file')
      try {
        await writeLocalDocumentBytes(workspace, created.path, bytes)
      } catch (error) {
        await removeLocalEntry(workspace, created.path).catch(() => undefined)
        throw error
      }
      if (parent !== workspace) setExpanded((current) => new Set(current).add(parent))
      setSelectedPath(created.path)
      setListSection('folder')
      loadBranch(parent)
      openInWorkspace(created.path, created.name)
      return 'ok'
    } catch (error) {
      return localEntryStatus(error) === 409 ? 'taken' : 'failed'
    } finally {
      officeCreateLock.current = false
      setOfficeCreateBusy(false)
    }
  }

  useEffect(() => {
    if (!workspace) {
      navWorkspace.current = ''
      setNav({ history: [], index: 0 })
      return
    }
    if (suppressNav.current) {
      suppressNav.current = false
      return
    }
    const current = browseDirectory || workspace
    const reset = navWorkspace.current !== workspace
    navWorkspace.current = workspace
    setNav((state) => {
      const base = reset ? [] : state.history.slice(0, state.index + 1)
      if (!reset && base[base.length - 1] === current) return state
      const history = [...base, current]
      return { history, index: history.length - 1 }
    })
  }, [browseDirectory, workspace])

  const visitDirectory = (path: string, record: boolean) => {
    if (!record) suppressNav.current = true
    setOpenedFile(null)
    setListSection('folder')
    const next = !path || path === workspace ? null : path
    setSelectedPath(next)
    setTreeOpen(true)
    if (next) {
      setExpanded((current) => {
        const copy = new Set(current)
        copy.add(next)
        return copy
      })
      loadBranch(next)
    }
  }

  const stepHistory = (offset: -1 | 1) => {
    const index = nav.index + offset
    const path = nav.history[index]
    if (!path) return
    visitDirectory(path, false)
    setNav((state) => ({ ...state, index }))
  }

  const locationDirectory = browseDirectory || workspace
  const locationCrumbs = directoryCrumbs(locationDirectory, workspace)
  const canGoUp = Boolean(workspace) && locationDirectory !== workspace
  const canGoBack = nav.index > 0
  const canGoForward = nav.index < nav.history.length - 1

  const renderEntries = (directory: string, depth: number, directoriesOnly = false): ReactNode => {
    const branch = branches.get(directory)
    const creating = inlineEdit?.mode === 'create' && inlineEdit.parent === directory ? inlineEdit : null
    return (
      <>
        {creating && (!directoriesOnly || creating.kind === 'folder')
          ? renderInline(creating, depth, creating.kind === 'folder')
          : null}
        {(branch?.entries ?? [])
          .filter((entry) => !directoriesOnly || entry.kind === 'directory')
          .map((entry) => {
            const renaming = inlineEdit?.mode === 'rename' && inlineEdit.path === entry.path ? inlineEdit : null
            const isOpen = entry.kind === 'directory' && expanded.has(entry.path)
            const directory = entry.kind === 'directory'
            return (
              <div key={entry.path}>
                <div
                  className="flex items-center gap-0.5"
                  style={indentStyle(depth)}
                  data-workspace-drop={directory ? entry.path : undefined}>
                  {renaming ? (
                    renderInline(renaming, 0, directory)
                  ) : (
                    <>
                      {directory ? (
                        <TreeDisclosure
                          open={isOpen}
                          label={isOpen ? t('workbench.workspace.collapse', '折叠') : t('common.expand', '展开')}
                          onClick={() => toggleDirectory(entry.path)}
                        />
                      ) : (
                        <span className="size-4 shrink-0" aria-hidden />
                      )}
                      <button
                        type="button"
                        aria-expanded={directory ? isOpen : undefined}
                        aria-current={
                          (directoriesOnly ? directory && browseDirectory === entry.path : selectedPath === entry.path)
                            ? 'page'
                            : undefined
                        }
                        data-workspace-session-folder={directory ? entry.name : undefined}
                        data-workspace-session-file={entry.kind === 'file' ? entry.name : undefined}
                        data-workspace-session-file-folder={
                          entry.kind === 'file' ? folderAttr(entry.path, workspace) : undefined
                        }
                        title={entry.path}
                        className={cn(
                          navClass(selectedPath === entry.path, depth),
                          'min-w-0 flex-1',
                          directory &&
                            (dropTarget === entry.path || externalDrop === entry.path) &&
                            'ring-2 ring-black/10 ring-inset'
                        )}
                        onPointerDown={(event) => beginPointerDrag(event, entry.path, entry.name)}
                        onClick={(event) => {
                          if (draggedClick()) return
                          setListSection('folder')
                          setSelectedPath(entry.path)
                          if (!directory) return
                          setOpenedFile(null)
                          if (event.detail > 1) return
                          toggleDirectory(entry.path)
                        }}
                        onDoubleClick={(event) => {
                          event.preventDefault()
                          event.stopPropagation()
                          if (previewDialogEvent(event)) return
                          if (entry.kind === 'file') openInWorkspace(entry.path, entry.name)
                        }}
                        onKeyDown={(event) => {
                          if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return
                          event.preventDefault()
                          event.stopPropagation()
                          const rect = event.currentTarget.getBoundingClientRect()
                          setSelectedPath(entry.path)
                          setMenu({ path: entry.path, x: rect.left + 8, y: rect.bottom + 4, tree: true })
                        }}
                        onContextMenu={(event) => {
                          event.preventDefault()
                          event.stopPropagation()
                          setSelectedPath(entry.path)
                          setMenu({ path: entry.path, x: event.clientX, y: event.clientY, tree: true })
                        }}>
                        <EntryIcon
                          entry={entry}
                          imageTicket={imageTicket}
                          onOpenImage={openMessageImage}
                          open={isOpen}
                          pendingImagePath={pendingImagePath}
                          root={workspace}
                        />
                        <span className="min-w-0 flex-1 truncate">{entry.name}</span>
                      </button>
                    </>
                  )}
                </div>
                {renaming?.error ? (
                  <p className="m-0 px-2.5 py-1 text-[12px] text-[#8a8a8a]">{renaming.error}</p>
                ) : null}
                {isOpen ? renderEntries(entry.path, depth + 1, directoriesOnly) : null}
              </div>
            )
          })}
        {creating?.error ? <p className="m-0 px-2.5 py-1 text-[12px] text-[#8a8a8a]">{creating.error}</p> : null}
        {branch?.capped ? (
          <p className="m-0 px-2.5 py-1 text-[12px] text-[#8a8a8a]">
            {t('workbench.workspace.capped', '只显示了前 500 项。')}
          </p>
        ) : null}
      </>
    )
  }

  const menuEntry = menu?.path ? findEntry(branches, menu.path) : undefined

  const folderBranch = browseDirectory ? branches.get(browseDirectory) : undefined
  const folderFiles = (folderBranch?.entries ?? []).filter((entry) => entry.kind === 'file')
  const listedFiles = (listSection === 'recent' ? recents : folderFiles)
    .slice()
    .sort((left, right) =>
      listSort === 'desc' ? right.modifiedMs - left.modifiedMs : left.modifiedMs - right.modifiedMs
    )
  const checkedListed = listedFiles.filter((entry) => checkedPaths.has(entry.path))
  const toggleChecked = (path: string) => {
    setCheckedPaths((current) => {
      const next = new Set(current)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }
  const uploadNamedFile = (file: File) => {
    const destination = browseDirectory || workspace
    return importLocalFile(workspace, destination, file).then(() => {
      setNotice(t('workbench.workspace.imported', { count: 1 }))
      if (destination && destination !== workspace) {
        setExpanded((current) => new Set(current).add(destination))
      }
      loadBranch(destination)
    })
  }
  const creatingFile =
    inlineEdit?.mode === 'create' &&
    inlineEdit.kind === 'file' &&
    inlineEdit.parent === (browseDirectory || workspace) &&
    listSection === 'folder'
      ? inlineEdit
      : null
  const renamingFile =
    inlineEdit?.mode === 'rename' && findEntry(branches, inlineEdit.path)?.kind === 'file' ? inlineEdit : null

  const acceptsExternalFiles = (event: DragEvent<HTMLElement>) => {
    if (layout !== 'split' || !workspace) return false
    const transfer = event.dataTransfer
    if (!transfer) return false
    if (transfer.files?.length) return true
    const types = Array.from(transfer.types ?? [])
    // Finder hides the file list until drop, so an empty type list is still a file drag.
    return types.length === 0 || types.includes('Files')
  }

  const importDestination = (event: DragEvent<HTMLElement>) => {
    const node = event.target instanceof Element ? event.target.closest('[data-workspace-drop]') : null
    return node?.getAttribute('data-workspace-drop') || browseDirectory || workspace || ''
  }

  const receiveExternalFiles = (event: DragEvent<HTMLElement>) => {
    if (!acceptsExternalFiles(event) || !workspace) return
    event.preventDefault()
    externalDepth.current = 0
    const destination = importDestination(event) || workspace
    setExternalDrop(null)
    void importDroppedFiles(workspace, destination, event.dataTransfer).then(
      (count) => {
        if (count === 0) return
        setNotice(t('workbench.workspace.imported', { count }))
        if (destination !== workspace) setExpanded((current) => new Set(current).add(destination))
        loadBranch(destination)
        const parent = parentOf(destination, workspace)
        if (parent !== destination) loadBranch(parent)
      },
      () => setNotice(t('workbench.workspace.mutate_failed', '这个操作没有完成。'))
    )
  }

  const [encyclopediaFile, setEncyclopediaFile] = useState<{ id: string; name: string } | null>(null)
  const [parsingId, setParsingId] = useState<string | null>(null)
  const teamDocuments = isCloudWorkspace(workspace)
  const compilePending = teamDocuments && listedFiles.some((entry) => compileInProgress(entry.compileStatus))
  useEffect(() => {
    if (!compilePending || !browseDirectory) return
    const timer = window.setInterval(() => loadBranch(browseDirectory, 'quiet'), 2000)
    return () => window.clearInterval(timer)
  }, [browseDirectory, compilePending, loadBranch])
  const listColumns = teamDocuments ? 5 : 4
  const parseTeamFile = (id: string, name: string) => {
    setNotice('')
    setParsingId(id)
    void driveApi.reparse(id).then(
      () => {
        setParsingId((current) => (current === id ? null : current))
        if (browseDirectory) loadBranch(browseDirectory, 'quiet')
      },
      () => {
        setParsingId((current) => (current === id ? null : current))
        setNotice(t('documents.pipeline.parse_failed'))
        if (browseDirectory) loadBranch(browseDirectory, 'quiet')
      }
    )
  }

  return (
    <div
      className={maximized ? 'flex h-full min-h-0 w-full min-w-0 flex-1 bg-white' : 'contents'}
      data-workspace-session-layout={maximized ? 'split' : undefined}
      onDragEnter={(event) => {
        if (!acceptsExternalFiles(event)) return
        event.preventDefault()
        externalDepth.current += 1
        setExternalDrop(importDestination(event))
      }}
      onDragOver={(event) => {
        if (!acceptsExternalFiles(event)) return
        event.preventDefault()
        event.dataTransfer.dropEffect = 'copy'
        const next = importDestination(event)
        setExternalDrop((current) => (current === next ? current : next))
      }}
      onDragLeave={(event) => {
        if (!acceptsExternalFiles(event)) return
        externalDepth.current = Math.max(0, externalDepth.current - 1)
        if (externalDepth.current === 0) setExternalDrop(null)
      }}
      onDrop={receiveExternalFiles}>
      {imagePreview ? (
        <ImageViewer
          key={imagePreview.id}
          alt={imagePreview.alt}
          className="sr-only"
          initialOpen
          onPreviewOpenChange={(open) => {
            if (!open) setImagePreview(null)
          }}
          preview={{ items: [imagePreview] }}
          src={imagePreview.src}
        />
      ) : null}
      <nav
        aria-label={t('workbench.workspace.session_folder', '会话文件夹')}
        data-workspace-session-tree="true"
        data-workspace-session-root={workspace || undefined}
        className={
          maximized
            ? cn(
                'flex h-full min-h-0 shrink-0 flex-col gap-0.5 bg-white px-3 pt-3 pb-4',
                layout === 'split' ? 'overflow-hidden' : 'overflow-y-auto',
                pageSidebarEdgeClassName
              )
            : 'flex h-full min-h-0 flex-col gap-0.5 overflow-y-auto bg-white px-3 pt-3 pb-4'
        }
        style={maximized ? { width: PAGE_SIDEBAR_DEFAULT_WIDTH } : undefined}
        onContextMenu={(event) => {
          if (event.target !== event.currentTarget) return
          event.preventDefault()
          setSelectedPath(null)
          setMenu({ x: event.clientX, y: event.clientY, tree: true })
        }}>
        {(layout === 'split' || !maximized) && (
          <div className="mb-2 flex shrink-0 items-center gap-1.5">
            <FileTreeSearchField
              className="flex-1"
              value={query}
              onChange={setQuery}
              placeholder={t('workbench.workspace.search_placeholder', '文件、文件夹…')}
              searchLabel={t('workbench.workspace.search', '搜索文件')}
              refreshLabel={t('workbench.workspace.refresh', '刷新')}
              refreshing={refreshing}
              onRefresh={reloadVisible}
              inputProps={{ 'data-workspace-session-search': 'true' }}
              refreshProps={{ 'data-workspace-files-action': 'refresh' }}
            />
            {!maximized && (
              <Tooltip content={t('workbench.workspace.new_file', '新建文件')} asChild>
                <button
                  type="button"
                  aria-label={t('workbench.workspace.new_file', '新建文件')}
                  data-workspace-files-action="new-file"
                  className="grid size-[30px] shrink-0 place-items-center rounded-[8px] text-[#3f3f3f] transition-colors outline-none hover:bg-[#f5f5f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset"
                  onClick={() => beginCreate('file', workspace)}>
                  <FilePlus className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
                </button>
              </Tooltip>
            )}
            {!maximized && (
              <Tooltip content={t('workbench.workspace.new_folder', '新建文件夹')} asChild>
                <button
                  type="button"
                  aria-label={t('workbench.workspace.new_folder', '新建文件夹')}
                  data-workspace-files-action="new-folder"
                  className="grid size-[30px] shrink-0 place-items-center rounded-[8px] text-[#3f3f3f] transition-colors outline-none hover:bg-[#f5f5f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset"
                  onClick={() => beginCreate('folder', workspace)}>
                  <FolderPlus className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
                </button>
              </Tooltip>
            )}
          </div>
        )}
        {searching && !maximized ? (
          <div
            role="tree"
            aria-label={t('workbench.workspace.search_results', '搜索结果')}
            className="flex flex-col gap-0.5">
            {searchPhase === 'loading' && searchHits.length === 0 ? (
              <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]">
                {t('workbench.workspace.loading', '正在加载…')}
              </p>
            ) : null}
            {searchPhase === 'error' ? (
              <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]" role="alert">
                {t('workbench.workspace.session_error', '会话文件夹没有加载出来')}
              </p>
            ) : null}
            {searchPhase === 'ready' && searchHits.length === 0 ? (
              <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]">
                {t('workbench.workspace.search_empty', '没有匹配的文件。')}
              </p>
            ) : null}
            {searchHits.map((hit) => (
              <button
                key={hit.path}
                type="button"
                data-workspace-session-search-hit={hit.relativePath}
                title={hit.path}
                className={navClass(selectedPath === hit.path, 1)}
                style={indentStyle(1)}
                data-workspace-drop={hit.isDirectory ? hit.path : undefined}
                onPointerDown={(event) =>
                  beginPointerDrag(event, hit.path, hit.relativePath.split('/').pop() ?? hit.relativePath)
                }
                onClick={() => {
                  if (draggedClick()) return
                  setSelectedPath(hit.path)
                }}
                onDoubleClick={(event) => {
                  event.preventDefault()
                  if (previewDialogEvent(event) || hit.isDirectory) return
                  openInWorkspace(hit.path, hit.relativePath.split('/').pop() ?? hit.relativePath)
                }}>
                <EntryIcon
                  entry={{
                    kind: hit.isDirectory ? 'directory' : 'file',
                    name: hit.relativePath.split('/').pop() ?? hit.relativePath,
                    path: hit.path
                  }}
                  imageTicket={imageTicket}
                  onOpenImage={openMessageImage}
                  pendingImagePath={pendingImagePath}
                  root={workspace}
                />
                <span className="min-w-0 flex-1 truncate">{hit.relativePath.split('/').pop()}</span>
              </button>
            ))}
          </div>
        ) : (
          <div
            role="tree"
            tabIndex={0}
            aria-label={label}
            className={cn('flex flex-col gap-0.5 outline-none', layout === 'split' && 'min-h-0 flex-1 overflow-hidden')}
            onKeyDown={onTreeKeyDown}>
            {layout !== 'split' ? (
              <div className="flex items-center gap-0.5">
                {maximized ? (
                  <span className="size-4 shrink-0" aria-hidden />
                ) : (
                  <TreeDisclosure
                    open={recentOpen}
                    label={recentOpen ? t('workbench.workspace.collapse', '折叠') : t('common.expand', '展开')}
                    onClick={() => setRecentOpen((current) => !current)}
                  />
                )}
                <button
                  type="button"
                  aria-expanded={maximized ? undefined : recentOpen}
                  aria-current={maximized && listSection === 'recent' ? 'page' : undefined}
                  className={cn(navClass(maximized && listSection === 'recent', 0), 'min-w-0 flex-1')}
                  onClick={() => {
                    setListSection('recent')
                    setRecentOpen(true)
                  }}>
                  <Clock className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                  <span className="min-w-0 flex-1 truncate">{t('workbench.workspace.recent', '最近')}</span>
                </button>
              </div>
            ) : null}
            {layout !== 'split' && !maximized && recentOpen && recentsPhase === 'loading' && recents.length === 0 ? (
              <p className="m-0 px-2.5 py-1 text-[13px] text-[#8a8a8a]">
                {t('workbench.workspace.loading', '正在加载…')}
              </p>
            ) : null}
            {layout !== 'split' && !maximized && recentOpen && recentsPhase === 'ready' && recents.length === 0 ? (
              <p className="m-0 px-2.5 py-1 text-[13px] text-[#8a8a8a]">
                {t('workbench.workspace.recent_empty', '工作区中还没有可显示的最近文件。')}
              </p>
            ) : null}
            {layout !== 'split' && !maximized && recentOpen
              ? recents.map((entry) => (
                  <button
                    key={entry.path}
                    type="button"
                    data-workspace-session-recent={entry.name}
                    title={entry.path}
                    className={navClass(selectedPath === entry.path, 1)}
                    style={indentStyle(1)}
                    onPointerDown={(event) => beginPointerDrag(event, entry.path, entry.name)}
                    onClick={() => {
                      if (draggedClick()) return
                      setSelectedPath(entry.path)
                    }}
                    onDoubleClick={(event) => {
                      event.preventDefault()
                      if (previewDialogEvent(event)) return
                      if (entry.kind === 'file') openInWorkspace(entry.path, entry.name)
                    }}>
                    <EntryIcon
                      entry={entry}
                      imageTicket={imageTicket}
                      onOpenImage={openMessageImage}
                      pendingImagePath={pendingImagePath}
                      root={workspace}
                    />
                    <span className="min-w-0 flex-1 truncate">{entry.name}</span>
                  </button>
                ))
              : null}
            <div className="flex shrink-0 items-center gap-0.5" data-workspace-drop={workspace || undefined}>
              <TreeDisclosure
                open={treeOpen}
                label={treeOpen ? t('workbench.workspace.collapse', '折叠') : t('common.expand', '展开')}
                onClick={() => setTreeOpen((current) => !current)}
              />
              <button
                type="button"
                aria-expanded={treeOpen}
                aria-current={
                  maximized
                    ? listSection === 'folder' && browseDirectory === workspace
                      ? 'page'
                      : undefined
                    : selectedPath
                      ? undefined
                      : 'page'
                }
                data-ui="workspace.session-tree"
                className={cn(
                  navClass(!selectedPath, 0),
                  'min-w-0 flex-1',
                  (dropTarget === workspace || externalDrop === workspace) && 'ring-2 ring-black/10 ring-inset'
                )}
                onClick={() => {
                  if (draggedClick()) return
                  setListSection('folder')
                  setSelectedPath(null)
                  setTreeOpen(true)
                }}>
                <FolderTree className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                <span className="min-w-0 flex-1 truncate">{label}</span>
              </button>
            </div>
            <div
              className={cn(layout === 'split' && 'min-h-0 flex-1 overflow-y-auto')}
              data-workspace-session-tree-scroll={layout === 'split' ? 'true' : undefined}>
              {!maximized && notice ? (
                <p className="m-0 px-2.5 py-1 text-[13px] text-[#8a8a8a]" role="alert">
                  {notice}
                </p>
              ) : null}
              {!maximized && !workspace ? (
                <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]">
                  {t('workbench.workspace.session_missing', '没有会话文件夹')}
                </p>
              ) : null}
              {!maximized && workspace && !rootBranch ? (
                <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]">
                  {t('workbench.workspace.loading', '正在加载…')}
                </p>
              ) : null}
              {!maximized && workspace && rootBranch?.phase === 'error' && rootBranch.entries.length === 0 ? (
                <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]" role="alert">
                  {t('workbench.workspace.session_error', '会话文件夹没有加载出来')}
                </p>
              ) : null}
              {!maximized && showEmpty ? (
                <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]" data-workspace-session-empty="true">
                  {t('workbench.workspace.session_empty', '这个文件夹还没有文件')}
                </p>
              ) : null}
              {treeOpen && workspace ? (
                <div data-workspace-drop={workspace}>{renderEntries(workspace, 1, maximized)}</div>
              ) : null}
            </div>
          </div>
        )}
        {menu
          ? createPortal(
              <div
                role="menu"
                aria-label={t('workbench.workspace.file_menu', '文件')}
                data-workspace-session-menu="true"
                className="fixed z-50 w-44 rounded-[10px] border border-[#ececec] bg-white p-1 shadow-[0_8px_18px_rgb(0_0_0/0.08)]"
                style={{ left: Math.max(8, menu.x), top: Math.max(8, menu.y) }}
                onPointerDown={(event) => event.stopPropagation()}>
                {menuEntry?.kind === 'file' ? (
                  <>
                    <MenuButton
                      label={t('workbench.workspace.open_file', '打开')}
                      icon={<ExternalLink size={14} strokeWidth={1.7} aria-hidden />}
                      onClick={() => {
                        openInWorkspace(menuEntry.path, menuEntry.name)
                        setMenu(null)
                      }}
                    />
                    {!isCloudWorkspace(workspace) ? (
                      <MenuButton
                        label={t('preview.save_as')}
                        icon={<Download size={14} strokeWidth={1.7} aria-hidden />}
                        onClick={() => void saveCopy(menuEntry)}
                      />
                    ) : null}
                  </>
                ) : null}
                {teamDocuments && menu.tree ? null : (
                  <MenuButton
                    label={t('workbench.workspace.new_file', '新建文件')}
                    icon={<FilePlus size={14} strokeWidth={1.7} aria-hidden />}
                    onClick={() => beginCreate('file', menuEntry?.kind === 'directory' ? menuEntry.path : '')}
                  />
                )}
                <MenuButton
                  label={t('workbench.workspace.new_folder', '新建文件夹')}
                  icon={<FolderPlus size={14} strokeWidth={1.7} aria-hidden />}
                  onClick={() => beginCreate('folder', menuEntry?.kind === 'directory' ? menuEntry.path : '')}
                />
                {menu.path ? (
                  <MenuButton
                    label={t('workbench.workspace.rename', '重命名')}
                    icon={<Pencil size={14} strokeWidth={1.7} aria-hidden />}
                    onClick={() => beginRename(menu.path!)}
                  />
                ) : null}
                {menu.path ? (
                  <PopConfirm
                    className="flex w-full"
                    tone="danger"
                    title={t('workbench.workspace.delete_title', '删除这个文件或文件夹？')}
                    description={
                      deleteError ||
                      `${menu.path.split(/[/\\]/u).pop() ?? menu.path}${t('workbench.workspace.delete_hint', ' 会移到废纸篓。')}`
                    }
                    confirmLabel={t('workbench.workspace.delete', '删除')}
                    cancelLabel={t('common.cancel', '取消')}
                    onOpenChange={(open) => {
                      if (!open) setMenu(null)
                    }}
                    onConfirm={() => confirmDelete(menu.path!)}>
                    <MenuButton
                      label={t('workbench.workspace.delete', '删除')}
                      danger
                      icon={<Trash2 size={14} strokeWidth={1.7} aria-hidden />}
                      onClick={() => setDeleteError('')}
                    />
                  </PopConfirm>
                ) : null}
              </div>,
              document.body
            )
          : null}
        {pendingDelete
          ? createPortal(
              <span
                className="pointer-events-none fixed"
                style={{
                  top: deleteAnchor.current.top,
                  left: deleteAnchor.current.left,
                  width: deleteAnchor.current.width,
                  height: deleteAnchor.current.height
                }}>
                <PopConfirm
                  className="block size-full"
                  defaultOpen
                  tone="danger"
                  title={t('workbench.workspace.delete_title', '删除这个文件或文件夹？')}
                  description={
                    deleteError ||
                    `${pendingDelete.split(/[/\\]/u).pop() ?? pendingDelete}${t('workbench.workspace.delete_hint', ' 会移到废纸篓。')}`
                  }
                  confirmLabel={t('workbench.workspace.delete', '删除')}
                  cancelLabel={t('common.cancel', '取消')}
                  onOpenChange={(open) => {
                    if (!open) setPendingDelete(null)
                  }}
                  onConfirm={() => confirmDelete(pendingDelete)}>
                  <span className="block size-full" />
                </PopConfirm>
              </span>,
              document.body
            )
          : null}
      </nav>
      {maximized && openedFile ? (
        <section aria-label={openedFile.name} className="flex min-h-0 min-w-0 flex-1 flex-col bg-white">
          <div className="px-6 pt-3">
            <button
              type="button"
              onClick={() => setOpenedFile(null)}
              className="flex h-[30px] items-center gap-1 rounded-[8px] bg-[#f5f5f5] px-3 text-[13px] text-[#171717] outline-none hover:bg-[#ececec] focus-visible:ring-2 focus-visible:ring-black/10">
              <ChevronRight className="size-3.5 rotate-180 text-current" strokeWidth={1.7} aria-hidden />
              {t('documents.preview.back', '返回')}
            </button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-8">
            <WorkbenchFileEditor root={workspace} path={openedFile.path} />
          </div>
        </section>
      ) : maximized ? (
        <section
          aria-label={t('workbench.workspace.file_list', '文件列表')}
          data-workspace-drop={locationDirectory || undefined}
          data-workspace-session-list="true"
          data-workspace-session-list-section={searching ? 'search' : listSection}
          className="relative flex min-h-0 min-w-0 flex-1 flex-col bg-white">
          {layout === 'split' && externalDrop ? (
            <div
              className="pointer-events-none absolute inset-0 z-10 grid place-items-center bg-white/80"
              data-workspace-file-drop="true">
              <p className="m-0 text-[14px] text-[#171717]">
                {t('workbench.workspace.drop_upload', '松开即可上传到这个文件夹')}
              </p>
            </div>
          ) : null}
          <div className="flex items-center justify-between gap-3 px-6 pt-3 pb-2">
            {layout === 'split' && (
              <div className="flex min-w-0 flex-1 items-center gap-1">
                <div className="flex shrink-0 items-center gap-0.5" data-file-navigation="true">
                  <button
                    type="button"
                    aria-label={t('workbench.workspace.back', '后退')}
                    className="grid size-[30px] place-items-center rounded-[8px] text-[#8a8a8a] outline-none hover:bg-[#f5f5f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:pointer-events-none disabled:opacity-[0.35]"
                    disabled={!canGoBack}
                    onClick={() => stepHistory(-1)}>
                    <ChevronLeft className="size-4" strokeWidth={1.7} aria-hidden />
                  </button>
                  <button
                    type="button"
                    aria-label={t('workbench.workspace.forward', '前进')}
                    className="grid size-[30px] place-items-center rounded-[8px] text-[#8a8a8a] outline-none hover:bg-[#f5f5f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:pointer-events-none disabled:opacity-[0.35]"
                    disabled={!canGoForward}
                    onClick={() => stepHistory(1)}>
                    <ChevronRight className="size-4" strokeWidth={1.7} aria-hidden />
                  </button>
                  <button
                    type="button"
                    aria-label={t('workbench.workspace.up', '上一级')}
                    className="grid size-[30px] place-items-center rounded-[8px] text-[#8a8a8a] outline-none hover:bg-[#f5f5f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:pointer-events-none disabled:opacity-[0.35]"
                    disabled={!canGoUp}
                    onClick={() => visitDirectory(parentOf(locationDirectory, workspace), true)}>
                    <ArrowUp className="size-4" strokeWidth={1.7} aria-hidden />
                  </button>
                </div>
                <nav
                  aria-label={t('workbench.workspace.location', '当前位置')}
                  className="flex min-w-0 items-center overflow-hidden"
                  data-file-manager-breadcrumb="true">
                  <button
                    type="button"
                    aria-current={locationCrumbs.length === 0 ? 'page' : undefined}
                    className={cn(
                      'inline-flex h-8 min-w-0 items-center rounded-[6px] px-1.5 text-[13px] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10',
                      locationCrumbs.length === 0 ? 'font-medium text-[#171717]' : 'text-[#8a8a8a] hover:text-[#171717]'
                    )}
                    onClick={() => visitDirectory(workspace, true)}>
                    <span className="truncate">{label}</span>
                  </button>
                  {locationCrumbs.map((crumb, index) => (
                    <span key={crumb.path} className="flex min-w-0 items-center">
                      <ChevronRight className="size-3 shrink-0 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden />
                      <button
                        type="button"
                        aria-current={index === locationCrumbs.length - 1 ? 'page' : undefined}
                        className={cn(
                          'inline-flex h-8 max-w-[160px] min-w-0 items-center rounded-[6px] px-1.5 text-[13px] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10',
                          index === locationCrumbs.length - 1
                            ? 'font-medium text-[#171717]'
                            : 'text-[#8a8a8a] hover:text-[#171717]'
                        )}
                        onClick={() => visitDirectory(crumb.path, true)}>
                        <span className="truncate">{crumb.label}</span>
                      </button>
                    </span>
                  ))}
                </nav>
              </div>
            )}
            {layout === 'split' ? null : (
              <label className="flex h-[30px] w-[220px] max-w-full items-center gap-1.5 rounded-[8px] bg-[#f5f5f5] px-2.5">
                <Search className="size-3.5 shrink-0 text-[#3f3f3f]" strokeWidth={1.7} aria-hidden />
                <input
                  aria-label={t('workbench.workspace.search', '搜索文件')}
                  data-workspace-session-search="true"
                  className="min-w-0 flex-1 bg-transparent text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a]"
                  placeholder={t('workbench.workspace.search_placeholder', '文件、文件夹…')}
                  spellCheck={false}
                  type="search"
                  value={query}
                  onChange={(event) => setQuery(event.currentTarget.value)}
                />
              </label>
            )}
            <div className="flex shrink-0 items-center gap-2">
              {teamDocuments ? (
                <TeamPipelineQueue parsingId={parsingId} onParse={parseTeamFile} onOpen={setEncyclopediaFile} />
              ) : null}
              {layout === 'split' ? (
                <OfficeCreateMenu
                  busy={officeCreateBusy}
                  existingNames={siblingNames(browseDirectory || workspace)}
                  onCreate={createOfficeFile}
                />
              ) : (
                <button
                  type="button"
                  data-workspace-files-action="new-file"
                  className="flex h-[30px] items-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white hover:bg-[#2a2a2a]"
                  onClick={() => beginCreate('file', browseDirectory || workspace)}>
                  <FilePlus className="size-4 shrink-0 text-white" color="#ffffff" strokeWidth={2} aria-hidden />
                  {t('workbench.workspace.new_file', '新建文件')}
                </button>
              )}
              <button
                type="button"
                data-workspace-files-action="new-folder"
                className="flex h-[30px] items-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10"
                onClick={() => beginCreate('folder', browseDirectory || workspace)}>
                <FolderPlus className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
                {t('workbench.workspace.new_folder', '新建文件夹')}
              </button>
              <Tooltip content={t('workbench.workspace.refresh', '刷新')} asChild>
                <button
                  type="button"
                  aria-label={t('workbench.workspace.refresh', '刷新')}
                  aria-busy={refreshing || undefined}
                  data-workspace-files-action="refresh"
                  className="grid size-8 place-items-center rounded-[8px] text-[#171717] hover:bg-[#f5f5f5]"
                  onClick={reloadVisible}>
                  <RefreshCw
                    className={cn('size-4', refreshing && 'animate-spin')}
                    style={refreshing ? { animationDuration: '0.7s' } : undefined}
                    strokeWidth={1.7}
                    aria-hidden
                  />
                </button>
              </Tooltip>
            </div>
          </div>
          {layout === 'split' ? (
            <div className="flex flex-wrap items-center gap-2 px-6 pb-2">
              <DriveUploadButton onUpload={uploadNamedFile} />
              <DriveHandoffButton
                count={checkedListed.length}
                names={checkedListed.map((entry) => entry.name)}
                onHandoff={(sentence) => {
                  queueWorkbenchComposer(sentence)
                  setNotice(sentence)
                }}
              />
            </div>
          ) : null}
          <div className="min-h-0 flex-1 overflow-y-auto">
            {notice ? (
              <p
                className={cn(
                  'm-0 px-6 py-2 text-[13px]',
                  notice === t('documents.pipeline.parse_failed') ? 'text-[#b42318]' : 'text-[#8a8a8a]'
                )}
                role="alert">
                {notice}
              </p>
            ) : null}
            {!workspace ? (
              <p className="m-0 px-6 py-16 text-center text-sm text-[#8a8a8a]">
                {t('workbench.workspace.session_missing', '没有会话文件夹')}
              </p>
            ) : null}
            {searching ? (
              <div className="px-6">
                {searchPhase === 'loading' && searchHits.length === 0 ? (
                  <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]">
                    {t('workbench.workspace.loading', '正在加载…')}
                  </p>
                ) : null}
                {searchPhase === 'error' ? (
                  <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]" role="alert">
                    {t('workbench.workspace.session_error', '会话文件夹没有加载出来')}
                  </p>
                ) : null}
                {searchPhase === 'ready' && searchHits.length === 0 ? (
                  <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]">
                    {t('workbench.workspace.search_empty', '没有匹配的文件。')}
                  </p>
                ) : null}
              </div>
            ) : listSection === 'recent' ? (
              recentsPhase === 'loading' && recents.length === 0 ? (
                <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]">
                  {t('workbench.workspace.loading', '正在加载…')}
                </p>
              ) : recentsPhase === 'ready' && recents.length === 0 ? (
                <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]">
                  {t('workbench.workspace.recent_empty', '工作区中还没有可显示的最近文件。')}
                </p>
              ) : null
            ) : (
              <>
                {workspace && (!folderBranch || (folderBranch.phase === 'loading' && folderFiles.length === 0)) ? (
                  <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]">
                    {t('workbench.workspace.loading', '正在加载…')}
                  </p>
                ) : null}
                {folderBranch?.phase === 'error' && folderFiles.length === 0 ? (
                  <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]" role="alert">
                    {t('workbench.workspace.session_error', '会话文件夹没有加载出来')}
                  </p>
                ) : null}
                {folderBranch?.phase === 'ready' && folderFiles.length === 0 && !creatingFile ? (
                  <div className="flex min-h-full items-center justify-center" data-workspace-session-empty="true">
                    <EmptyState title={t('workbench.workspace.session_empty', '这个文件夹还没有文件')} />
                  </div>
                ) : null}
              </>
            )}
            {workspace &&
            (searching ? searchHits.length > 0 : listedFiles.length > 0 || creatingFile || renamingFile) ? (
              <table className="w-full table-fixed border-collapse text-left">
                <thead className="sticky top-0 z-[1] bg-white">
                  <tr className="border-b border-[#efefef] text-[12px] font-normal text-[#8a8a8a]">
                    <th className="h-9 pl-6 font-normal">{t('documents.column.name', '文件名')}</th>
                    {teamDocuments ? (
                      <th className="h-9 w-[112px] pr-3 font-normal">{t('documents.column.pipeline')}</th>
                    ) : null}
                    <th className="h-9 w-[132px] pr-3 font-normal">
                      <button
                        type="button"
                        className="inline-flex items-center gap-0.5"
                        onClick={() => setListSort((current) => (current === 'desc' ? 'asc' : 'desc'))}>
                        {t('documents.column.updated', '更新时间')}
                        <ChevronDown
                          className={cn('size-3.5 shrink-0 transition-transform', listSort === 'asc' && 'rotate-180')}
                        />
                      </button>
                    </th>
                    <th className="h-9 w-[72px] pr-3 text-right font-normal">
                      {t('documents.column.retrievals', '检索')}
                    </th>
                    <th className="h-9 w-[88px] pr-6 text-right font-normal">{t('documents.column.size', '大小')}</th>
                  </tr>
                </thead>
                <tbody>
                  {!searching && creatingFile ? (
                    <tr>
                      <td colSpan={listColumns} className="px-6 py-1">
                        {renderInline(creatingFile, 0, false)}
                      </td>
                    </tr>
                  ) : null}
                  {(searching
                    ? searchHits.map((hit) => ({
                        name: hit.relativePath.split('/').pop() ?? hit.relativePath,
                        path: hit.path,
                        kind: hit.isDirectory ? ('directory' as const) : ('file' as const),
                        sizeBytes: 0,
                        modifiedMs: 0,
                        relativePath: hit.relativePath
                      }))
                    : listedFiles.map((entry) => ({ ...entry, relativePath: undefined }))
                  ).map((entry) => {
                    const renaming =
                      renamingFile?.mode === 'rename' && renamingFile.path === entry.path ? renamingFile : null
                    if (renaming) {
                      return (
                        <tr key={entry.path}>
                          <td colSpan={listColumns} className="px-6 py-1">
                            {renderInline(renaming, 0, false)}
                          </td>
                        </tr>
                      )
                    }
                    return (
                      <tr
                        key={entry.path}
                        className={cn('group', selectedPath === entry.path ? 'bg-[#f2f3f5]' : 'hover:bg-[#f7f7f7]')}
                        onContextMenu={(event) => {
                          event.preventDefault()
                          setSelectedPath(entry.path)
                          setMenu({ path: entry.path, x: event.clientX, y: event.clientY })
                        }}>
                        <td className="h-12 pr-2 pl-6">
                          <div className="flex min-w-0 items-center gap-2">
                            {layout === 'split' && entry.kind === 'file' ? (
                              <DriveFileCheck
                                name={entry.name}
                                checked={checkedPaths.has(entry.path)}
                                onToggle={() => toggleChecked(entry.path)}
                              />
                            ) : null}
                            <button
                              type="button"
                              title={entry.path}
                              data-workspace-drop={entry.kind === 'directory' ? entry.path : undefined}
                              data-workspace-session-file={entry.kind === 'file' ? entry.name : undefined}
                              data-workspace-session-folder={entry.kind === 'directory' ? entry.name : undefined}
                              data-workspace-session-file-folder={
                                entry.kind === 'file' ? folderAttr(entry.path, workspace) : undefined
                              }
                              data-workspace-session-search-hit={
                                searching && 'relativePath' in entry ? entry.relativePath : undefined
                              }
                              className={cn(
                                'flex min-w-0 flex-1 items-center gap-2.5 text-left outline-none',
                                entry.kind === 'directory' &&
                                  (dropTarget === entry.path || externalDrop === entry.path) &&
                                  'ring-2 ring-black/10 ring-inset'
                              )}
                              onPointerDown={(event) => beginPointerDrag(event, entry.path, entry.name)}
                              onKeyDown={(event) => onListedFileKeyDown(event, entry.path)}
                              onClick={(event) => {
                                if (draggedClick() || previewDialogEvent(event)) return
                                if (entry.kind === 'directory') {
                                  setOpenedFile(null)
                                  setQuery('')
                                  setListSection('folder')
                                  setSelectedPath(entry.path)
                                  setExpanded((current) => new Set(current).add(entry.path))
                                  loadBranch(entry.path)
                                  return
                                }
                                setSelectedPath(entry.path)
                                if (layout === 'split')
                                  openInWorkspace(
                                    entry.path,
                                    entry.name,
                                    'driveFileId' in entry ? entry.driveFileId : undefined
                                  )
                              }}
                              onDoubleClick={(event) => {
                                event.preventDefault()
                                event.stopPropagation()
                                if (previewDialogEvent(event)) return
                                if (entry.kind === 'file')
                                  openInWorkspace(
                                    entry.path,
                                    entry.name,
                                    'driveFileId' in entry ? entry.driveFileId : undefined
                                  )
                              }}>
                              {entry.kind === 'directory' ? (
                                <DocumentFolderIcon className="h-9 w-8" />
                              ) : (
                                <EntryIcon
                                  className="h-9 w-8"
                                  entry={entry}
                                  imageTicket={imageTicket}
                                  onOpenImage={openMessageImage}
                                  pendingImagePath={pendingImagePath}
                                  root={workspace}
                                />
                              )}
                              <span className="truncate text-[13px] font-medium text-[#1a1a1a]">{entry.name}</span>
                            </button>
                            {teamDocuments &&
                            !searching &&
                            entry.kind === 'file' &&
                            'driveFileId' in entry &&
                            entry.driveFileId ? (
                              <span className="flex shrink-0 items-center gap-0.5 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100">
                                {'parseStatus' in entry && entry.parseStatus === 'parsed' ? null : (
                                  <Tooltip content={t('documents.encyclopedia.parse')} asChild>
                                    <button
                                      type="button"
                                      aria-label={t('documents.encyclopedia.parse')}
                                      data-ui="documents.encyclopedia.parse"
                                      className="flex size-6 shrink-0 items-center justify-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset disabled:opacity-40"
                                      disabled={parsingId === entry.driveFileId}
                                      onClick={() => parseTeamFile(entry.driveFileId, entry.name)}>
                                      <ScanText className="size-3.5" strokeWidth={1.7} aria-hidden />
                                    </button>
                                  </Tooltip>
                                )}
                                {'parseStatus' in entry &&
                                entry.parseStatus === 'parsed' &&
                                entry.compileStatus === 'done' ? (
                                  <Tooltip content={t('documents.encyclopedia.open')} asChild>
                                    <button
                                      type="button"
                                      aria-label={t('documents.encyclopedia.open')}
                                      data-ui="documents.encyclopedia.open"
                                      className="flex size-6 shrink-0 items-center justify-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset"
                                      onClick={() => setEncyclopediaFile({ id: entry.driveFileId, name: entry.name })}>
                                      <BookOpen className="size-3.5" strokeWidth={1.7} aria-hidden />
                                    </button>
                                  </Tooltip>
                                ) : null}
                              </span>
                            ) : null}
                          </div>
                        </td>
                        {teamDocuments && !searching && entry.kind === 'file' ? (
                          <td className="pr-3">
                            <DocumentPipelineStatus
                              parseStatus={'parseStatus' in entry ? entry.parseStatus : undefined}
                              compileStatus={'compileStatus' in entry ? entry.compileStatus : undefined}
                              parsing={'driveFileId' in entry && parsingId === entry.driveFileId}
                            />
                          </td>
                        ) : teamDocuments ? (
                          <td />
                        ) : null}
                        <td className="pr-3 text-[13px] whitespace-nowrap text-[#8a8a8a]">
                          {entry.modifiedMs ? formatDocumentActivity(entry.modifiedMs) : ''}
                        </td>
                        <td
                          className="pr-3 text-right text-[13px] whitespace-nowrap text-[#8a8a8a]"
                          data-agent-retrievals={
                            entry.kind === 'file' && typeof entry.agentRetrievals === 'number'
                              ? entry.agentRetrievals
                              : undefined
                          }>
                          {entry.kind === 'file' && typeof entry.agentRetrievals === 'number'
                            ? entry.agentRetrievals
                            : ''}
                        </td>
                        <td className="pr-6 text-right text-[13px] whitespace-nowrap text-[#8a8a8a]">
                          {entry.kind === 'file' ? formatDocumentSize(entry.sizeBytes) : ''}
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            ) : null}
          </div>
        </section>
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
      <TeamFileEncyclopediaDialog file={encyclopediaFile} onClose={() => setEncyclopediaFile(null)} />
    </div>
  )
}

function MenuButton({
  label,
  onClick,
  danger = false,
  icon
}: {
  label: string
  onClick: () => void
  danger?: boolean
  icon?: ReactNode
}) {
  return (
    <button
      type="button"
      role="menuitem"
      className={cn(
        'flex min-h-8 w-full items-center gap-2 rounded-[7px] px-2 text-left text-[12px] text-[#171717] hover:bg-[#f3f3f3]',
        danger && 'text-[#171717]'
      )}
      onClick={onClick}>
      {icon ?? <span className="size-3.5" aria-hidden />}
      <span>{label}</span>
    </button>
  )
}

function childWithin(path: string, directory: string): string | null {
  const dir = directory.replace(/[/\\]+$/u, '')
  if (!dir || path === dir) return null
  const prefix = `${dir}/`
  if (!path.startsWith(prefix)) return null
  const rest = path.slice(prefix.length)
  return rest || null
}

function isDirectChild(path: string, directory: string): boolean {
  const rest = childWithin(path, directory)
  return !!rest && !rest.includes('/') && !rest.includes('\\')
}

function ancestorChild(path: string, directory: string): string | null {
  const rest = childWithin(path, directory)
  if (!rest) return null
  const segment = rest.split(/[/\\]/u)[0]
  if (!segment) return null
  return `${directory.replace(/[/\\]+$/u, '')}/${segment}`
}

function pruneExpanded(current: Set<string>, directory: string, entries: readonly LocalEntry[]): Set<string> {
  const live = new Set(entries.filter((entry) => entry.kind === 'directory').map((entry) => entry.path))
  let changed = false
  const next = new Set<string>()
  for (const item of current) {
    const child = ancestorChild(item, directory)
    if (child !== null && !live.has(child)) {
      changed = true
      continue
    }
    next.add(item)
  }
  return changed ? next : current
}

function mergeDirectRecents(
  current: readonly LocalEntry[],
  directory: string,
  entries: readonly LocalEntry[]
): LocalEntry[] {
  const directFiles = entries.filter((entry) => entry.kind === 'file')
  const kept = current.filter((item) => !isDirectChild(item.path, directory))
  const merged = [...kept, ...directFiles]
  merged.sort((left, right) => right.modifiedMs - left.modifiedMs || left.path.localeCompare(right.path))
  return merged.slice(0, RECENT_LIMIT)
}

function sameRecents(left: readonly LocalEntry[], right: readonly LocalEntry[]): boolean {
  return (
    left.length === right.length &&
    left.every(
      (item, index) =>
        item.path === right[index]?.path &&
        item.modifiedMs === right[index]?.modifiedMs &&
        item.name === right[index]?.name
    )
  )
}

function refreshTargets(
  workspace: string,
  relativePaths: readonly string[],
  branches: ReadonlyMap<string, Branch>
): string[] {
  const targets = new Set<string>()
  for (const relative of relativePaths) {
    if (!relative) {
      targets.add(workspace)
      continue
    }
    let current = workspace
    let resolved = true
    for (const part of relative.split('/')) {
      const child = branches.get(current)?.entries.find((entry) => entry.kind === 'directory' && entry.name === part)
      if (!child) {
        targets.add(current)
        resolved = false
        break
      }
      current = child.path
    }
    if (resolved) targets.add(current)
  }
  return [...targets]
}

function isAbort(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError'
}

function sleep(ms: number, signal: AbortSignal): Promise<void> {
  if (signal.aborted) return Promise.reject(new DOMException('aborted', 'AbortError'))
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => {
      signal.removeEventListener('abort', onAbort)
      resolve()
    }, ms)
    const onAbort = () => {
      window.clearTimeout(timer)
      reject(new DOMException('aborted', 'AbortError'))
    }
    signal.addEventListener('abort', onAbort, { once: true })
  })
}

function browseDirectoryFor(
  selectedPath: string | null,
  branches: ReadonlyMap<string, Branch>,
  workspace: string
): string {
  if (!workspace) return ''
  if (!selectedPath) return workspace
  const entry = findEntry(branches, selectedPath)
  if (entry?.kind === 'file') return parentOf(selectedPath, workspace)
  return selectedPath
}

function findEntry(branches: ReadonlyMap<string, Branch>, path: string): LocalEntry | undefined {
  for (const branch of branches.values()) {
    const found = branch.entries.find((entry) => entry.path === path)
    if (found) return found
  }
  return undefined
}

function replaceExpanded(current: Set<string>, previous: string, nextPath: string): Set<string> {
  const next = new Set<string>()
  for (const item of current) {
    if (item === previous || item.startsWith(`${previous}/`)) next.add(nextPath + item.slice(previous.length))
    else next.add(item)
  }
  return next
}
