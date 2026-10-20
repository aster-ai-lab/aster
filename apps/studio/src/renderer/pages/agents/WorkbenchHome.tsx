import { PopConfirm, SearchInput, Tooltip } from '@a3s-lab/ui'
import {
  Archive,
  ArrowUp,
  Check,
  ChevronLeft,
  ChevronRight,
  Clock,
  Folder,
  FolderOpen,
  Mic,
  MessageSquarePlus,
  Pencil,
  Pin,
  PinOff,
  Plug,
  Plus,
  Search,
  Sparkles,
  X
} from 'lucide-react'
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type DragEvent as ReactDragEvent,
  type KeyboardEvent,
  type ReactNode
} from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { useMemberProfile } from '@renderer/components/account/memberProfile'
import { A3SBotAvatar, type A3SBotAvatarActivity } from '@renderer/components/bot/A3SBotAvatar'
import { taskListBotAnimated } from '@renderer/components/bot/botAvatarActivity'
import { ComposerFileDropOverlay, dragEventCarriesFiles } from '@renderer/components/composer/ComposerFileDropOverlay'
import { ComposerScreenshotControl } from '@renderer/components/composer/screenshot/ComposerScreenshotControl'
import { queueWorkbenchScreenshot } from '@renderer/components/composer/screenshot/screenshotCapture'
import {
  getAgentDraftCacheKey,
  writeAgentDraftCache
} from '@renderer/components/composer/variants/agent/agentDraftCache'
import { queueWorkbenchDroppedFiles } from '@renderer/components/composer/workbenchDroppedFiles'
import { useRestingDialog } from '@renderer/components/restingDialog'
import { StudioProductMark } from '@renderer/components/StudioProductMark'
import { useUpdateAgent } from '@renderer/hooks/agent/useAgent'
import { useCommandHandler } from '@renderer/hooks/command'
import {
  PAGE_SIDEBAR_DEFAULT_WIDTH,
  readWorkbenchSidebarCollapsed,
  writeWorkbenchSidebarCollapsed
} from '@renderer/layout/pageSidebar'
import type { CapabilityKind } from '@renderer/pages/capabilities/capabilitiesData'
import CapabilitiesPage from '@renderer/pages/capabilities/CapabilitiesPage'
import TasksSettings from '@renderer/pages/settings/TasksSettings'
import { OPEN_MAIN_ROUTE_EVENT, type OpenMainRouteEvent } from '@renderer/services/mainWindowNavigation'
import { toast } from '@renderer/services/toast'
import { studioProductName } from '@renderer/tenantBrand'
import type { PermissionMode } from '@renderer/types/agent'
import { cn } from '@renderer/utils/style'

import { writeCodeSession } from './codeMode'
import {
  type HomeComposerSendOutcome,
  CodeWorkspacePlacement,
  HomeComposerModeChips,
  HomeComposerModeTrack,
  requestHomeComposerSend,
  useHomeComposerModeState
} from './homeComposerMode'
import { HomeComposerTaskBar } from './HomeComposerTaskBar'
import { OFFICIAL_WRITING_EVENT } from './officialWriting'
import { PAPERLOOM_OPEN, type PaperloomOpenRequest } from './paperloom/paperloomApi'
import { PaperloomPanel } from './paperloom/PaperloomPanel'
import { codeWorkspaceErrorText, placeCodeSend } from './prepareCodeWorkspace'
import { armPresentationDeck } from './presentationDeck'
import { useHomeComposerOptions } from './useHomeComposerOptions'
import { WorkbenchCommandPalette } from './WorkbenchCommandPalette'
import {
  clearWorkbenchComposer,
  fillWorkbenchComposer,
  focusWorkbenchComposer,
  holdSessionDraftWrites,
  releaseSessionDraftWrites
} from './workbenchComposerBridge'
import { markPendingHomeSubmit, type WorkbenchHomeTaskBridge } from './workbenchComposerHost'
import { WorkbenchFeatureStrip } from './WorkbenchFeatureStrip'
import { WorkbenchComposerOptions } from './WorkbenchHomeComposerExtras'
import { WorkbenchPermissionPicker } from './WorkbenchPermissionPicker'
import {
  cycleTaskPermissionMode,
  displayedTaskPermissionMode,
  readTaskPermissionRevision,
  storedPermissionMode,
  subscribeTaskPermission,
  writePendingPermissionMode,
  writeTaskPermission
} from './workbenchPermissionPostures'
import { WorkbenchProjectCreateDialog } from './WorkbenchProjectCreateDialog'
import { writePendingProject } from './WorkbenchProjectTray'

import './workbenchComposer.css'

export type WorkbenchSession = {
  id: string
  name: string
  updatedAt: string
  pinned: boolean
  projectId?: string
  projectName?: string
  /** Document-expert face for this task. A settled task omits it and rests completed. */
  avatar?: A3SBotAvatarActivity
}

type SessionPlace = 'project' | 'recent' | 'history' | 'settled'

/** Ended tasks stay out of the active lists across reloads. Undo itself is only in memory. */
export const WORKBENCH_SETTLED_SESSIONS_KEY = 'workbench.settled.sessions.v1'
const SETTLE_UNDO_MS = 5_000

function readSettledIds(): string[] {
  try {
    const raw = localStorage.getItem(WORKBENCH_SETTLED_SESSIONS_KEY)
    const parsed: unknown = raw ? JSON.parse(raw) : []
    if (!Array.isArray(parsed)) return []
    return parsed.filter((id): id is string => typeof id === 'string' && id.trim().length > 0)
  } catch {
    return []
  }
}

function writeSettledIds(ids: readonly string[]) {
  try {
    localStorage.setItem(WORKBENCH_SETTLED_SESSIONS_KEY, JSON.stringify(ids))
  } catch {
    // Private mode can reject the write. The in-memory list still holds this visit.
  }
}

type ProjectGroup = {
  id: string
  name: string
  sessions: WorkbenchSession[]
}

export type WorkbenchCatalog = CapabilityKind

export type WorkbenchProjectRef = {
  id: string
  name: string
}

const MAXIMUM_RECENT_TASK_ITEMS = 50
const PROJECT_TASK_PREVIEW = 4
/** Recent projects kept in the sidebar. The rest stay behind 「更多」. */
export const SIDEBAR_PROJECT_WINDOW = 8
export const WORKBENCH_SIDEBAR_DEFAULT_WIDTH = PAGE_SIDEBAR_DEFAULT_WIDTH

/** Half of the 26px seam button, so the collapsed control stays inside the page. */
const SIDEBAR_TOGGLE_INSET = 13

/** The rail opens in place. Inline none beats the seam class, which only eases the chevron. */
export const WORKBENCH_SIDEBAR_SLIDE = 'none'

function workbenchSidebarSlide(): string {
  return 'none'
}

/** A 260px rail plus the category column leaves a phone-width catalog with no card. */
function useNarrowWindow() {
  const [narrow, setNarrow] = useState(false)
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return
    const query = window.matchMedia('(max-width: 760px)')
    const apply = () => setNarrow(query.matches)
    apply()
    query.addEventListener?.('change', apply)
    return () => query.removeEventListener?.('change', apply)
  }, [])
  return narrow
}

const WORKBENCH_CANVAS_CLASS =
  'workbench-sidebar-slide pointer-events-auto flex min-h-0 min-w-0 flex-1 flex-col bg-white'

function SidebarSeamToggle({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  const { t } = useTranslation()
  const label = t(collapsed ? 'workbench.sidebar.expand' : 'workbench.sidebar.collapse')
  return (
    <button
      type="button"
      data-ui="workbench.sidebar.toggle"
      aria-label={label}
      title={label}
      aria-pressed={!collapsed}
      aria-controls="workbench-sidebar"
      onClick={onToggle}
      className="workbench-sidebar-slide pointer-events-auto absolute top-1/2 z-30 grid size-[26px] -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full bg-white text-[#8a8a8a] shadow-[0_0_0_1px_rgba(255,255,255,0.8),0_1px_3px_rgba(23,23,23,0.12)] outline-none hover:text-[#171717] hover:shadow-none focus-visible:ring-2 focus-visible:ring-black/10"
      style={{
        left: collapsed ? SIDEBAR_TOGGLE_INSET : WORKBENCH_SIDEBAR_DEFAULT_WIDTH,
        transition: workbenchSidebarSlide()
      }}>
      <ChevronLeft
        aria-hidden
        className={cn('workbench-sidebar-slide size-3.5', collapsed && 'rotate-180')}
        strokeWidth={1.75}
      />
    </button>
  )
}
const MINUTE_MS = 60_000
const HOUR_MS = 60 * MINUTE_MS
const DAY_MS = 24 * HOUR_MS

type Translate = (key: string, options?: { count: number }) => string

function sessionTimestamp(updatedAt: string): number {
  const value = Date.parse(updatedAt)
  return Number.isFinite(value) ? value : 0
}

function assignedProjectId(session: WorkbenchSession): string | null {
  const id = session.projectId?.trim()
  return id ? id : null
}

function dedupedSessions(sessions: WorkbenchSession[]): WorkbenchSession[] {
  const byId = new Map<string, WorkbenchSession>()
  for (const session of sessions) {
    if (!session.id.trim()) continue
    const existing = byId.get(session.id)
    if (!existing || sessionTimestamp(session.updatedAt) >= sessionTimestamp(existing.updatedAt)) {
      byId.set(session.id, { ...session, pinned: session.pinned || existing?.pinned === true })
    } else if (session.pinned) {
      byId.set(session.id, { ...existing, pinned: true })
    }
  }
  return [...byId.values()]
}

function byRecency(left: WorkbenchSession, right: WorkbenchSession): number {
  return sessionTimestamp(right.updatedAt) - sessionTimestamp(left.updatedAt)
}

function byPinThenRecency(left: WorkbenchSession, right: WorkbenchSession): number {
  if (left.pinned !== right.pinned) return left.pinned ? -1 : 1
  return byRecency(left, right)
}

function latestActivity(sessions: WorkbenchSession[]): number {
  return sessions.reduce((latest, session) => Math.max(latest, sessionTimestamp(session.updatedAt)), 0)
}

function projectGroups(sessions: WorkbenchSession[], fallbackName: string): ProjectGroup[] {
  const groups = new Map<string, ProjectGroup>()
  for (const session of dedupedSessions(sessions)) {
    const id = assignedProjectId(session)
    if (!id) continue
    const name = session.projectName?.trim() || fallbackName
    const group = groups.get(id) ?? { id, name, sessions: [] }
    if (session.projectName?.trim()) group.name = session.projectName.trim()
    group.sessions.push(session)
    groups.set(id, group)
  }
  const list = [...groups.values()]
  for (const group of list) group.sessions.sort(byPinThenRecency)
  list.sort((left, right) => latestActivity(right.sessions) - latestActivity(left.sessions))
  return list
}

/**
 * Shortcut window for the workbench sidebar.
 *
 * `projects` stays in its existing order. The open project stays in the
 * window when it would otherwise fall past the recent slice.
 */
export function sidebarProjectWindow<T extends { id: string }>(
  projects: readonly T[],
  pinnedId: string | undefined,
  limit = SIDEBAR_PROJECT_WINDOW
): { visible: readonly T[]; hidden: readonly T[] } {
  if (limit <= 0) return { visible: [], hidden: [...projects] }
  if (projects.length <= limit) return { visible: [...projects], hidden: [] }
  const pinned = pinnedId ? projects.find((project) => project.id === pinnedId) : undefined
  const head = projects.slice(0, limit)
  if (!pinned || head.some((project) => project.id === pinned.id)) {
    return { visible: head, hidden: projects.slice(limit) }
  }
  const rest = projects.filter((project) => project.id !== pinned.id)
  const visible = [pinned, ...rest.slice(0, limit - 1)]
  const visibleIds = new Set(visible.map((project) => project.id))
  return { visible, hidden: projects.filter((project) => !visibleIds.has(project.id)) }
}

function withExplicitProjects(
  groups: ProjectGroup[],
  explicit: readonly WorkbenchProjectRef[] | undefined
): ProjectGroup[] {
  if (!explicit?.length) return groups
  const merged = groups.map((group) => ({ ...group, sessions: [...group.sessions] }))
  const byId = new Map(merged.map((group) => [group.id, group]))
  for (const project of explicit) {
    const id = project.id.trim()
    if (!id) continue
    const name = project.name.trim()
    const existing = byId.get(id)
    if (existing) {
      if (name) existing.name = name
      continue
    }
    const created = { id, name: name || id, sessions: [] }
    merged.push(created)
    byId.set(id, created)
  }
  return merged
}

function inboxSessions(sessions: WorkbenchSession[]): { pinned: WorkbenchSession[]; recent: WorkbenchSession[] } {
  const pinned: WorkbenchSession[] = []
  const recent: WorkbenchSession[] = []
  for (const session of dedupedSessions(sessions)) {
    if (assignedProjectId(session)) continue
    if (session.pinned) pinned.push(session)
    else recent.push(session)
  }
  return {
    pinned: pinned.toSorted(byRecency),
    recent: recent.toSorted(byRecency).slice(0, MAXIMUM_RECENT_TASK_ITEMS)
  }
}

function localDayStart(timestamp: number): number {
  const date = new Date(timestamp)
  if (Number.isNaN(date.getTime())) return Number.NEGATIVE_INFINITY
  date.setHours(0, 0, 0, 0)
  return date.getTime()
}

function calendarDayKey(timestamp: number): string {
  if (!Number.isFinite(timestamp)) return 'unknown'
  const date = new Date(timestamp)
  if (Number.isNaN(date.getTime())) return 'unknown'
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

function calendarDayLabel(timestamp: number, now: number, t: Translate, locale: string): string {
  const start = localDayStart(timestamp)
  if (!Number.isFinite(start)) return t('workbench.day.unknown')
  if (start === localDayStart(now)) return t('workbench.day.today')
  const yesterday = new Date(now)
  yesterday.setHours(0, 0, 0, 0)
  yesterday.setDate(yesterday.getDate() - 1)
  if (start === yesterday.getTime()) return t('workbench.day.yesterday')
  const date = new Date(timestamp)
  const sameYear = date.getFullYear() === new Date(now).getFullYear()
  return new Intl.DateTimeFormat(locale, {
    ...(sameYear ? {} : { year: 'numeric' }),
    month: 'short',
    day: 'numeric'
  }).format(date)
}

function groupByCalendarDay(sessions: WorkbenchSession[], now: number, t: Translate, locale: string) {
  const groups = new Map<string, { id: string; label: string; dayStartMs: number; sessions: WorkbenchSession[] }>()
  for (const session of sessions) {
    const timestamp = sessionTimestamp(session.updatedAt)
    const id = calendarDayKey(timestamp)
    const existing = groups.get(id)
    if (existing) {
      existing.sessions.push(session)
      continue
    }
    groups.set(id, {
      id,
      label: calendarDayLabel(timestamp, now, t, locale),
      dayStartMs: localDayStart(timestamp),
      sessions: [session]
    })
  }
  return [...groups.values()].sort((left, right) => right.dayStartMs - left.dayStartMs)
}

/** Case-insensitive literal match. Wildcard characters stay ordinary text. */
export function matchesQuery(name: string, query: string): boolean {
  const needle = query.trim().toLowerCase()
  if (!needle) return true
  return name.toLowerCase().includes(needle)
}

/** Every matched span, in the name's own letters. An empty query leaves the name unmarked. */
export function highlightQuery(name: string, query: string): ReactNode {
  const needle = query.trim().toLowerCase()
  if (!needle) return name
  const lower = name.toLowerCase()
  const nodes: ReactNode[] = []
  let cursor = 0
  let found = false
  while (cursor < lower.length) {
    const index = lower.indexOf(needle, cursor)
    if (index < 0) break
    found = true
    if (index > cursor) nodes.push(name.slice(cursor, index))
    const end = index + needle.length
    nodes.push(
      <mark key={`${index}-${end}`} className="rounded-[2px] bg-[#fff3b0] text-inherit">
        {name.slice(index, end)}
      </mark>
    )
    cursor = end
  }
  if (!found) return name
  if (cursor < name.length) nodes.push(name.slice(cursor))
  return <>{nodes}</>
}

function pickerKeyDown(
  event: KeyboardEvent<HTMLElement>,
  options: {
    count: number
    query: string
    onMove: (direction: 1 | -1) => void
    onChoose: () => void
    onClear: () => void
    onClose: () => void
  }
) {
  if (event.nativeEvent.isComposing) return
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    if (options.query.trim()) options.onClear()
    else options.onClose()
    return
  }
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    if (options.count === 0) return
    options.onMove(event.key === 'ArrowDown' ? 1 : -1)
    return
  }
  if (event.key !== 'Enter' || options.count === 0) return
  event.preventDefault()
  options.onChoose()
}

function relativeTime(updatedAt: string, now: number, t: Translate, locale: string): string {
  const timestamp = sessionTimestamp(updatedAt)
  const elapsed = Math.max(0, now - timestamp)
  if (elapsed < MINUTE_MS) return t('workbench.time.just_now')
  if (elapsed < HOUR_MS) return t('workbench.time.minutes', { count: Math.floor(elapsed / MINUTE_MS) })
  if (elapsed < DAY_MS) return t('workbench.time.hours', { count: Math.floor(elapsed / HOUR_MS) })
  if (elapsed < 7 * DAY_MS) return t('workbench.time.days', { count: Math.floor(elapsed / DAY_MS) })
  return new Intl.DateTimeFormat(locale, { month: 'numeric', day: 'numeric' }).format(new Date(timestamp))
}

const PICKER_FIELD_CLASS =
  'h-[30px] rounded-[8px] border-transparent bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:ring-0'

const SECTION_ACTION_CLASS =
  'grid size-6 shrink-0 place-items-center rounded-[6px] text-[#8a8a8a] outline-none transition-colors hover:bg-black/[0.05] hover:text-[#171717] focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-black/10'

const ROW_ACTION_CLASS =
  'grid size-6 place-items-center rounded-[6px] text-[#3f3f3f] outline-none transition-colors hover:bg-black/[0.08] focus-visible:ring-2 focus-visible:ring-black/10'

function navClass(selected: boolean, nested = false) {
  return cn(
    'flex w-full items-center gap-2.5 rounded-[8px] text-left text-[13px] transition-colors outline-none',
    nested ? 'h-8 pr-2.5 pl-8' : 'h-9 px-2.5',
    'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
    selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
  )
}

const DRAG_START_DISTANCE = 5

type RowDrag = {
  sessionId: string
  projectId?: string
  name: string
  pointerId: number
  startX: number
  startY: number
  active: boolean
  source: HTMLElement
}

/** Hit-test the project row or the task list under the pointer. */
function sessionDropTarget(x: number, y: number) {
  const hit = document.elementFromPoint(x, y)
  if (!(hit instanceof Element)) return null
  const project = hit.closest('[data-ui="workbench.project"]')
  if (project instanceof HTMLElement && project.dataset.projectId) {
    return { kind: 'project' as const, node: project, projectId: project.dataset.projectId }
  }
  const tasks = hit.closest('[data-ui="workbench.tasks"]')
  if (tasks instanceof HTMLElement) return { kind: 'inbox' as const, node: tasks }
  return null
}

type WorkbenchProjectDraft = {
  name: string
}

type WorkbenchHomeProps = {
  sessions: WorkbenchSession[]
  projects?: readonly WorkbenchProjectRef[]
  activeSessionId: string | null
  homeActive: boolean
  /** Which catalog fills the work canvas. Absent keeps the home composer. */
  catalog?: WorkbenchCatalog | null
  useLiveComposer?: boolean
  onComposerHostChange?: (node: HTMLElement | null) => void
  /** The assistant whose permission mode the fallback chip commits. */
  permissionAgentId?: string | null
  /** Fallback sends that open a task workspace use the same bridge as the live composer. */
  homeTask?: WorkbenchHomeTaskBridge | null
  onNewTask: () => void
  onOpenCatalog?: (kind: WorkbenchCatalog) => void
  onUseSkill?: (request: { name: string; prompt: string }) => void
  onOpenScheduledTasks?: () => void
  onCreateScheduledWithAgent?: () => void
  onCreateProject?: (input: WorkbenchProjectDraft) => Promise<void> | void
  onCreateTaskInProject?: (projectId: string) => void
  onMoveSessionToProject?: (sessionId: string, projectId: string) => void
  onMoveSessionToInbox?: (sessionId: string) => void
  onOpenSession: (id: string) => void
  onTogglePin: (id: string) => void
  onDeleteSession: (id: string) => void
  onRenameSession: (id: string, name: string) => void
  /** Controlled by the work page so the chat padding tracks the sidebar. */
  sidebarCollapsed?: boolean
  onSidebarCollapsedChange?: (collapsed: boolean) => void
}

/** Fallback homepage toolbar. The live composer owns the same menu once it attaches. */
function HomeFallbackComposerOptions() {
  const { options, changeOptions } = useHomeComposerOptions()
  return <WorkbenchComposerOptions options={options} onChange={changeOptions} />
}

export function WorkbenchHome({
  sessions,
  projects: explicitProjects,
  activeSessionId,
  homeActive,
  catalog = null,
  useLiveComposer = false,
  onComposerHostChange,
  permissionAgentId = null,
  homeTask = null,
  onNewTask,
  onOpenCatalog,
  onUseSkill,
  onOpenScheduledTasks,
  onCreateScheduledWithAgent,
  onCreateProject,
  onCreateTaskInProject,
  onMoveSessionToProject,
  onMoveSessionToInbox,
  onOpenSession,
  onTogglePin,
  onDeleteSession,
  onRenameSession,
  sidebarCollapsed: sidebarCollapsedProp,
  onSidebarCollapsedChange
}: WorkbenchHomeProps) {
  const { t, i18n } = useTranslation()
  const composerMode = useHomeComposerModeState()
  const [paperloomRequest, setPaperloomRequest] = useState<PaperloomOpenRequest | null>(null)
  useEffect(() => {
    if (!homeActive) {
      setPaperloomRequest(null)
      return
    }
    const open = (event: Event) => {
      event.preventDefault()
      setPaperloomRequest((event as CustomEvent<PaperloomOpenRequest>).detail)
    }
    window.addEventListener(PAPERLOOM_OPEN, open)
    return () => window.removeEventListener(PAPERLOOM_OPEN, open)
  }, [homeActive])
  const [storedSidebarCollapsed, setStoredSidebarCollapsed] = useState(readWorkbenchSidebarCollapsed)
  const sidebarCollapsed = sidebarCollapsedProp ?? storedSidebarCollapsed
  const narrow = useNarrowWindow()
  const [narrowRevealed, setNarrowRevealed] = useState(false)
  const locale = i18n?.resolvedLanguage || i18n?.language || 'zh-CN'
  const composerRef = useRef<HTMLTextAreaElement>(null)
  const composerHostRef = useRef<HTMLDivElement | null>(null)
  const anchorRef = useRef<HTMLDivElement>(null)
  // React reconciles an empty div and will drop nodes portaled into it. This
  // element is created outside React, then parked in the anchor, so the live
  // composer stays put when the homepage re-renders.
  const portalNodeRef = useRef<HTMLDivElement | null>(null)
  if (portalNodeRef.current === null && typeof document !== 'undefined') {
    const node = document.createElement('div')
    node.dataset.ui = 'workbench.composer'
    node.className = 'relative w-full'
    portalNodeRef.current = node
  }
  const [composerAttached, setComposerAttached] = useState(false)
  const [showComposerFallback, setShowComposerFallback] = useState(false)
  const [fileDragOver, setFileDragOver] = useState(false)
  const fileDragDepth = useRef(0)
  const pendingFocus = useRef(false)
  const renameSettled = useRef(false)
  const seenExplicitProjects = useRef<Set<string>>(new Set())
  const [draft, setDraft] = useState('')
  const memberId = useMemberProfile()?.id ?? null
  const { updateAgent } = useUpdateAgent()
  const permissionRevision = useSyncExternalStore(subscribeTaskPermission, readTaskPermissionRevision, () => 0)
  const permissionSessionId = !homeActive && activeSessionId ? activeSessionId : 'draft'
  const shownPermissionMode = useMemo(
    () => displayedTaskPermissionMode(permissionSessionId, memberId),
    [memberId, permissionRevision, permissionSessionId]
  )
  const changePermissionMode = (mode: string) => {
    const next = storedPermissionMode(mode)
    if (!next) return
    writeTaskPermission(permissionSessionId, { mode: next }, undefined, memberId)
    if (permissionSessionId === 'draft') writePendingPermissionMode(next, undefined, memberId)
  }
  const [historyOpen, setHistoryOpen] = useState(false)
  const [historyQuery, setHistoryQuery] = useState('')
  const [historyCursor, setHistoryCursor] = useState(0)
  const [taskQuery, setTaskQuery] = useState('')
  const [searchOpen, setSearchOpen] = useState(false)
  const [searchCursor, setSearchCursor] = useState(0)
  const [createOpen, setCreateOpen] = useState(false)
  const rowDrag = useRef<RowDrag | null>(null)
  const dragGhost = useRef<HTMLDivElement | null>(null)
  const suppressRowClick = useRef(false)
  const dragActions = useRef({
    toProject: (_sessionId: string, _projectId: string) => {},
    toInbox: (_sessionId: string) => {}
  })
  useEffect(() => {
    const highlight: { node: HTMLElement | null } = { node: null }
    const clearHighlight = () => {
      if (!highlight.node) return
      delete highlight.node.dataset.dropActive
      delete highlight.node.dataset.dropInbox
      highlight.node = null
    }
    const paint = (node: HTMLElement | null, inbox: boolean) => {
      if (highlight.node !== node) {
        clearHighlight()
        if (!node) return
        highlight.node = node
        node.dataset.dropActive = 'true'
      }
      if (!node) return
      if (inbox) node.dataset.dropInbox = 'true'
      else delete node.dataset.dropInbox
    }
    const hideGhost = () => {
      const ghost = dragGhost.current
      if (!ghost) return
      ghost.hidden = true
      ghost.textContent = ''
    }
    const finish = (event: PointerEvent, commit: boolean) => {
      const drag = rowDrag.current
      if (!drag || event.pointerId !== drag.pointerId) return
      rowDrag.current = null
      delete drag.source.dataset.dragging
      const target = commit && drag.active ? sessionDropTarget(event.clientX, event.clientY) : null
      clearHighlight()
      hideGhost()
      if (!drag.active) return
      suppressRowClick.current = true
      window.setTimeout(() => {
        suppressRowClick.current = false
      }, 0)
      if (!commit || !target) return
      if (target.kind === 'project' && target.projectId) dragActions.current.toProject(drag.sessionId, target.projectId)
      else if (target.kind === 'inbox') dragActions.current.toInbox(drag.sessionId)
    }
    const onMove = (event: PointerEvent) => {
      const drag = rowDrag.current
      if (!drag || event.pointerId !== drag.pointerId) return
      if (!drag.active) {
        const dx = event.clientX - drag.startX
        const dy = event.clientY - drag.startY
        if (dx * dx + dy * dy < DRAG_START_DISTANCE * DRAG_START_DISTANCE) return
        drag.active = true
        drag.source.dataset.dragging = 'true'
      }
      const ghost = dragGhost.current
      if (ghost) {
        ghost.hidden = false
        ghost.textContent = drag.name
        ghost.style.transform = `translate(${event.clientX + 12}px, ${event.clientY + 12}px)`
      }
      const target = sessionDropTarget(event.clientX, event.clientY)
      if (target?.kind === 'project') paint(target.node, false)
      else if (target?.kind === 'inbox' && drag.projectId) paint(target.node, true)
      else paint(null, false)
    }
    const onUp = (event: PointerEvent) => finish(event, true)
    const onCancel = (event: PointerEvent) => finish(event, false)
    const onBlur = () => {
      const drag = rowDrag.current
      if (!drag) return
      rowDrag.current = null
      delete drag.source.dataset.dragging
      clearHighlight()
      hideGhost()
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
    window.addEventListener('pointercancel', onCancel)
    window.addEventListener('blur', onBlur)
    return () => {
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
      window.removeEventListener('pointercancel', onCancel)
      window.removeEventListener('blur', onBlur)
      clearHighlight()
      hideGhost()
    }
  }, [])
  const [expandedProjectId, setExpandedProjectId] = useState<string | undefined>()
  const [projectCatalogOpen, setProjectCatalogOpen] = useState(false)
  const searchPresence = useRestingDialog(searchOpen)
  const historyPresence = useRestingDialog(historyOpen)
  const catalogPresence = useRestingDialog(projectCatalogOpen)
  const [projectQuery, setProjectQuery] = useState('')
  const [projectCursor, setProjectCursor] = useState(0)
  const [revealedIds, setRevealedIds] = useState<ReadonlySet<string>>(() => new Set())
  const [scheduledOpen, setScheduledOpen] = useState(false)
  const [paletteOpen, setPaletteOpen] = useState(false)
  const [settledIds, setSettledIds] = useState<string[]>(readSettledIds)
  // shortcut: undo covers only the latest ended session, queue when more than one ending must be reversible.
  const [settleUndo, setSettleUndo] = useState<{ id: string; name: string } | null>(null)
  useEffect(() => {
    writeSettledIds(settledIds)
  }, [settledIds])
  useEffect(() => {
    if (!settleUndo) return
    const timer = window.setTimeout(() => setSettleUndo(null), SETTLE_UNDO_MS)
    return () => window.clearTimeout(timer)
  }, [settleUndo])

  const [renaming, setRenaming] = useState<{ id: string; place: SessionPlace } | null>(null)
  const [renameDraft, setRenameDraft] = useState('')
  const canSend = draft.trim().length > 0
  const settledSet = useMemo(() => new Set(settledIds), [settledIds])
  const activeSessions = useMemo(
    () => sessions.filter((session) => !settledSet.has(session.id)),
    [sessions, settledSet]
  )
  const settledSessions = useMemo(
    () => dedupedSessions(sessions.filter((session) => settledSet.has(session.id))).toSorted(byRecency),
    [sessions, settledSet]
  )
  const projects = useMemo(
    () => withExplicitProjects(projectGroups(activeSessions, t('workbench.projects')), explicitProjects),
    [activeSessions, explicitProjects, t]
  )
  const projectWindow = useMemo(() => sidebarProjectWindow(projects, expandedProjectId), [expandedProjectId, projects])
  const projectNames = useMemo(() => projects.map((project) => project.name), [projects])
  const inbox = useMemo(() => inboxSessions(activeSessions), [activeSessions])
  const searchResults = useMemo(() => {
    const needle = taskQuery.trim().toLowerCase()
    if (!needle) return []
    return activeSessions.filter((session) => session.name.toLowerCase().includes(needle))
  }, [activeSessions, taskQuery])
  const searchActive = searchResults.length === 0 ? -1 : Math.min(searchCursor, searchResults.length - 1)
  const seededProjects = useRef(false)
  const now = Date.now()
  const taskDayGroups = groupByCalendarDay(inbox.recent, now, t, locale)
  const closeSearch = () => {
    setSearchOpen(false)
    setTaskQuery('')
    setSearchCursor(0)
  }
  const openSearchResult = (sessionId: string) => {
    closeSearch()
    setScheduledOpen(false)
    onOpenSession(sessionId)
  }
  const onSearchKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.nativeEvent.isComposing) return
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      closeSearch()
      return
    }
    if (searchResults.length === 0) return
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const direction = event.key === 'ArrowDown' ? 1 : -1
      setSearchCursor((cursor) => {
        const current = Math.min(cursor, searchResults.length - 1)
        return (current + direction + searchResults.length) % searchResults.length
      })
      return
    }
    if (event.key !== 'Enter' || searchActive < 0) return
    event.preventDefault()
    const session = searchResults[searchActive]
    if (session) openSearchResult(session.id)
  }
  const historySessions = [...inbox.pinned, ...inbox.recent]
  const visibleHistory = historySessions.filter((session) => matchesQuery(session.name, historyQuery))
  const historyActiveIndex = visibleHistory.length === 0 ? -1 : Math.min(historyCursor, visibleHistory.length - 1)
  const historyActiveId = historyActiveIndex < 0 ? undefined : visibleHistory[historyActiveIndex]?.id
  const visibleCatalog = projects.filter((project) => matchesQuery(project.name, projectQuery))
  const catalogIndex = visibleCatalog.length === 0 ? -1 : Math.min(projectCursor, visibleCatalog.length - 1)
  const clearProjectQuery = () => {
    setProjectQuery('')
    setProjectCursor(0)
  }
  const clearHistoryQuery = () => {
    setHistoryQuery('')
    setHistoryCursor(0)
  }
  const chooseCatalogProject = (index: number) => {
    const project = visibleCatalog[index]
    if (!project) return
    setExpandedProjectId(project.id)
    setProjectCatalogOpen(false)
  }
  const chooseHistorySession = (sessionId: string) => {
    setHistoryOpen(false)
    setScheduledOpen(false)
    onOpenSession(sessionId)
  }

  const focusComposer = () => {
    composerRef.current?.focus()
  }
  const onFallbackDragEnter = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!dragEventCarriesFiles(event)) return
    event.preventDefault()
    fileDragDepth.current += 1
    setFileDragOver(true)
  }
  const onFallbackDragOver = (event: ReactDragEvent<HTMLDivElement>) => {
    event.preventDefault()
    if (!event.dataTransfer) return
    const carries = dragEventCarriesFiles(event)
    event.dataTransfer.dropEffect = carries ? 'copy' : 'none'
    if (!carries) return
    if (fileDragDepth.current === 0) {
      fileDragDepth.current = 1
      setFileDragOver(true)
    }
  }
  const onFallbackDragLeave = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!dragEventCarriesFiles(event)) return
    fileDragDepth.current = Math.max(0, fileDragDepth.current - 1)
    if (fileDragDepth.current === 0) setFileDragOver(false)
  }
  const onFallbackDrop = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!dragEventCarriesFiles(event)) return
    event.preventDefault()
    event.stopPropagation()
    fileDragDepth.current = 0
    setFileDragOver(false)
    queueWorkbenchDroppedFiles([...event.dataTransfer.files])
    if (useLiveComposer) focusWorkbenchComposer()
    else composerRef.current?.focus()
  }

  const catalogOpen = catalog === 'connector' || catalog === 'skill'
  const catalogOverlay = narrow && catalogOpen
  const layoutCollapsed = catalogOverlay ? !narrowRevealed : sidebarCollapsed
  const toggleSidebar = () => {
    if (catalogOverlay) {
      setNarrowRevealed((open) => !open)
      return
    }
    const next = !sidebarCollapsed
    writeWorkbenchSidebarCollapsed(next)
    if (onSidebarCollapsedChange) onSidebarCollapsedChange(next)
    else setStoredSidebarCollapsed(next)
  }
  useEffect(() => {
    if (!catalogOverlay) setNarrowRevealed(false)
  }, [catalogOverlay])
  const composerHostActive = useLiveComposer && homeActive && !catalogOpen && !scheduledOpen

  useLayoutEffect(() => {
    const anchor = anchorRef.current
    const portal = portalNodeRef.current
    if (!anchor || !portal || !composerHostActive) return
    if (portal.parentNode !== anchor) anchor.appendChild(portal)
  })

  useLayoutEffect(() => {
    const portal = composerHostActive ? portalNodeRef.current : null
    composerHostRef.current = portal
    onComposerHostChange?.(portal)
  }, [composerHostActive, onComposerHostChange])

  useEffect(() => {
    const node = composerHostRef.current
    if (!node || !composerHostActive) {
      setComposerAttached(false)
      return
    }
    const sync = () => setComposerAttached(node.childElementCount > 0)
    sync()
    const observer = new MutationObserver(sync)
    observer.observe(node, { childList: true })
    return () => observer.disconnect()
  }, [composerHostActive])

  useEffect(() => {
    if (!composerAttached) return
    if (draft.trim()) fillWorkbenchComposer(draft)
  }, [composerAttached, draft])

  useEffect(() => {
    if (!composerHostActive || composerAttached) {
      setShowComposerFallback(false)
      return
    }
    let second = 0
    const first = window.requestAnimationFrame(() => {
      second = window.requestAnimationFrame(() => setShowComposerFallback(true))
    })
    return () => {
      window.cancelAnimationFrame(first)
      if (second) window.cancelAnimationFrame(second)
    }
  }, [composerAttached, composerHostActive])

  useEffect(() => {
    if (!homeActive || !pendingFocus.current) return
    pendingFocus.current = false
    if (useLiveComposer) focusWorkbenchComposer()
    else focusComposer()
  }, [homeActive, useLiveComposer])

  // The work tab is already /app/agents, so the same route does not replace this overlay.
  useEffect(() => {
    const onWorkRoute = (event: Event) => {
      if ((event as OpenMainRouteEvent).detail?.path !== '/app/agents') return
      setScheduledOpen(false)
    }
    window.addEventListener(OPEN_MAIN_ROUTE_EVENT, onWorkRoute)
    return () => window.removeEventListener(OPEN_MAIN_ROUTE_EVENT, onWorkRoute)
  }, [])

  useEffect(() => {
    if (seededProjects.current || projects.length === 0) return
    seededProjects.current = true
    const active = projects.find((project) => project.sessions.some((session) => session.id === activeSessionId))
    const target = active ?? projects.find((project) => project.sessions.length > 0)
    if (target) setExpandedProjectId(target.id)
  }, [activeSessionId, projects])

  useEffect(() => {
    const incoming = (explicitProjects ?? []).map((project) => project.id.trim()).filter(Boolean)
    if (!seenExplicitProjects.current.size && incoming.length > 0) {
      for (const id of incoming) seenExplicitProjects.current.add(id)
      return
    }
    const fresh = incoming.filter((id) => !seenExplicitProjects.current.has(id))
    for (const id of incoming) seenExplicitProjects.current.add(id)
    const created = fresh.at(-1)
    if (created) setExpandedProjectId(created)
  }, [explicitProjects])

  const finishFallbackSend = async (outcome: HomeComposerSendOutcome) => {
    if (outcome.deck) {
      if (homeTask) {
        const created = await homeTask.create()
        if (created) {
          armPresentationDeck(created.id, outcome.deck)
          writeAgentDraftCache(getAgentDraftCacheKey(created.id), {
            text: '',
            tokens: [],
            files: [],
            knowledgeBaseIds: [],
            workspaceKey: '',
            agentId: ''
          })
          homeTask.adopt(created.id)
        }
      }
      if (outcome.cleared) setDraft('')
      return
    }
    if (outcome.code || outcome.text !== undefined) {
      const topic = draft.trim()
      if (!homeTask) {
        toast.error(t('workbench.default_agent.preparing'))
        return
      }
      let codeRecord: Awaited<ReturnType<typeof placeCodeSend>> | null = null
      if (outcome.code) {
        try {
          // shortcut: the fallback card has no project folder, so a worktree reports need-project and keeps the draft. Read the pending workspace when this card can.
          codeRecord = await placeCodeSend({
            placement: outcome.code.placement,
            projectPath: null,
            topic,
            suffix: outcome.code.suffix,
            branch: outcome.code.branch,
            base: outcome.code.base,
            existing: outcome.code.existing,
            pending: true
          })
        } catch (error) {
          toast.error(codeWorkspaceErrorText(error))
          return
        }
      }
      const created = await homeTask.create()
      if (!created) {
        if (codeRecord?.placement === 'worktree') writePendingProject(null)
        return
      }
      if (codeRecord) writeCodeSession(created.id, codeRecord)
      writeAgentDraftCache(getAgentDraftCacheKey(created.id), {
        text: topic,
        tokens: [],
        files: [],
        knowledgeBaseIds: [],
        workspaceKey: '',
        agentId: ''
      })
      markPendingHomeSubmit(created.id)
      homeTask.adopt(created.id)
      setDraft('')
      return
    }
    if (outcome.cleared) setDraft('')
  }

  const startNewTask = () => {
    setScheduledOpen(false)
    setRenaming(null)
    holdSessionDraftWrites()
    if (useLiveComposer) {
      clearWorkbenchComposer()
      if (!homeActive) pendingFocus.current = true
    } else {
      setDraft('')
      if (homeActive) focusComposer()
      else pendingFocus.current = true
    }
    onNewTask()
    window.setTimeout(releaseSessionDraftWrites, 0)
  }

  const settleSession = (session: WorkbenchSession) => {
    setSettledIds((ids) => (ids.includes(session.id) ? ids : [...ids, session.id]))
    setSettleUndo({ id: session.id, name: session.name })
  }

  const undoSettle = () => {
    if (!settleUndo) return
    const id = settleUndo.id
    setSettleUndo(null)
    setSettledIds((ids) => ids.filter((item) => item !== id))
  }

  const currentTask =
    !homeActive && activeSessionId ? sessions.find((session) => session.id === activeSessionId) : undefined
  const canSettleCurrent = Boolean(currentTask && !settledSet.has(currentTask.id))
  const focusPaletteComposer = () => {
    if (showComposerFallback || !useLiveComposer) focusComposer()
    else focusWorkbenchComposer()
  }
  useCommandHandler('workbench.palette', () => setPaletteOpen(true))
  // The live composer already owns this shortcut. Handle it only while that composer is absent.
  useCommandHandler('chat.input.focus', focusPaletteComposer, {
    enabled: showComposerFallback || !useLiveComposer
  })

  const beginRename = (session: WorkbenchSession, place: SessionPlace) => {
    renameSettled.current = false
    setRenaming({ id: session.id, place })
    setRenameDraft(session.name)
  }

  const commitRename = (session: WorkbenchSession, raw: string) => {
    if (renameSettled.current) return
    renameSettled.current = true
    const next = raw.trim()
    setRenaming(null)
    if (!next || next === session.name) return
    onRenameSession(session.id, next)
  }

  const cancelRename = () => {
    renameSettled.current = true
    setRenaming(null)
  }

  const moveSessionToProject = (sessionId: string, projectId: string) => {
    const session = sessions.find((item) => item.id === sessionId)
    if (!session || session.projectId === projectId) return
    setExpandedProjectId(projectId)
    onMoveSessionToProject?.(sessionId, projectId)
  }

  const moveSessionToInbox = (sessionId: string) => {
    const session = sessions.find((item) => item.id === sessionId)
    if (!session?.projectId) return
    onMoveSessionToInbox?.(sessionId)
  }

  dragActions.current = {
    toProject: moveSessionToProject,
    toInbox: moveSessionToInbox
  }

  const toggleProject = (projectId: string) => {
    setExpandedProjectId((current) => (current === projectId ? undefined : projectId))
  }

  const renderSession = (session: WorkbenchSession, place: SessionPlace) => {
    const current = !homeActive && activeSessionId === session.id
    const pickerActive = place === 'history' && session.id === historyActiveId
    const avatarActivity = session.avatar ?? 'completed'
    const renamingHere = renaming?.id === session.id && renaming.place === place
    const pinLabel = t(session.pinned ? 'workbench.session.unpin' : 'workbench.session.pin')
    return (
      <div
        key={`${place}-${session.id}`}
        ref={
          pickerActive
            ? (node) => {
                node?.scrollIntoView?.({ block: 'nearest' })
              }
            : undefined
        }
        id={place === 'history' ? `workbench-history-option-${session.id}` : undefined}
        data-ui="workbench.session"
        data-session-id={session.id}
        data-session-place={place}
        data-pinned={session.pinned ? 'true' : undefined}
        data-picker-active={pickerActive ? 'true' : undefined}
        aria-selected={place === 'history' ? pickerActive : undefined}
        onPointerDown={(event) => {
          if (event.button !== 0) return
          if (event.target instanceof Element && event.target.closest('[data-session-actions]')) return
          if (rowDrag.current) delete rowDrag.current.source.dataset.dragging
          rowDrag.current = {
            sessionId: session.id,
            projectId: session.projectId,
            name: session.name,
            pointerId: event.pointerId,
            startX: event.clientX,
            startY: event.clientY,
            active: false,
            source: event.currentTarget
          }
        }}
        className={cn(
          'group relative flex h-9 cursor-grab items-center overflow-hidden rounded-[8px] pr-1 transition-colors outline-none data-[dragging=true]:opacity-60',
          place === 'project' && 'pl-8',
          current
            ? 'bg-[#f2f3f5] font-medium text-[#171717]'
            : pickerActive
              ? 'bg-[#f2f3f5] text-[#171717]'
              : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
        )}
        style={{ WebkitUserDrag: 'none' }}>
        {renamingHere ? (
          <input
            aria-label={t('workbench.session.rename')}
            value={renameDraft}
            autoFocus
            onChange={(event) => setRenameDraft(event.target.value)}
            onBlur={(event) => commitRename(session, event.currentTarget.value)}
            onKeyDown={(event) => {
              // IME candidate confirmation still emits keydown; committing there would save the raw pinyin
              // buffer instead of the composed text. `keyCode === 229` is the legacy fallback.
              // oxlint-disable-next-line no-deprecated
              if (event.nativeEvent.isComposing || event.keyCode === 229) return
              if (event.key === 'Enter') {
                event.preventDefault()
                commitRename(session, event.currentTarget.value)
              }
              if (event.key === 'Escape') {
                event.preventDefault()
                cancelRename()
              }
            }}
            className="mx-1 h-7 min-w-0 flex-1 rounded-[6px] bg-[#f5f5f5] px-2 text-[13px] text-[#171717] outline-none"
          />
        ) : (
          <button
            type="button"
            aria-current={current ? 'page' : undefined}
            onClick={() => {
              if (suppressRowClick.current) return
              if (place === 'history') {
                chooseHistorySession(session.id)
                return
              }
              setScheduledOpen(false)
              onOpenSession(session.id)
            }}
            onDoubleClick={() => beginRename(session, place)}
            className="flex h-full min-w-0 flex-1 items-center gap-2.5 px-2.5 text-left text-[13px] outline-none group-focus-within:pr-28 group-hover:pr-28 focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
            <span aria-hidden data-ui="workbench.session.avatar" className="inline-flex size-5 shrink-0">
              <A3SBotAvatar
                activity={avatarActivity}
                animated={taskListBotAnimated(avatarActivity, current)}
                idleGaze={false}
                size={20}
                variant="default"
              />
            </span>
            {session.pinned && <Pin className="size-3 shrink-0 text-[#8a8a8a]" strokeWidth={1.7} />}
            <span className="min-w-0 flex-1 truncate">
              {place === 'history' ? highlightQuery(session.name, historyQuery) : session.name}
            </span>
          </button>
        )}
        {renamingHere ? null : (
          <time className="shrink-0 pr-1.5 text-[12px] text-[#8a8a8a] group-focus-within:hidden group-hover:hidden">
            {relativeTime(session.updatedAt, now, t, locale)}
          </time>
        )}
        {renamingHere ? null : (
          <span
            data-session-actions
            className={cn(
              'pointer-events-none absolute inset-y-0 right-1 flex items-center rounded-[8px] pl-1 opacity-0 transition-opacity group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100',
              current ? 'bg-[#f2f3f5]' : 'bg-[#f7f7f7]'
            )}>
            <Tooltip content={t('workbench.session.rename')} delay={400} placement="top" sideOffset={6} asChild>
              <button
                type="button"
                data-session-action="rename"
                aria-label={t('workbench.session.rename')}
                onClick={() => beginRename(session, place)}
                className={ROW_ACTION_CLASS}>
                <Pencil size={14} className="text-current" strokeWidth={1.7} />
              </button>
            </Tooltip>
            <Tooltip content={pinLabel} delay={400} placement="top" sideOffset={6} asChild>
              <button
                type="button"
                data-session-action="pin"
                aria-pressed={session.pinned}
                aria-label={pinLabel}
                onClick={() => onTogglePin(session.id)}
                className={ROW_ACTION_CLASS}>
                {session.pinned ? (
                  <PinOff size={14} className="text-current" strokeWidth={1.7} />
                ) : (
                  <Pin size={14} className="text-current" strokeWidth={1.7} />
                )}
              </button>
            </Tooltip>
            {place === 'settled' ? null : (
              <Tooltip content={t('workbench.session.settle')} delay={400} placement="top" sideOffset={6} asChild>
                <button
                  type="button"
                  data-session-action="settle"
                  aria-label={t('workbench.session.settle')}
                  onClick={() => settleSession(session)}
                  className={ROW_ACTION_CLASS}>
                  <Check size={14} className="text-current" strokeWidth={1.7} />
                </button>
              </Tooltip>
            )}
            <PopConfirm
              boundary="#workbench-sidebar"
              tone="default"
              title={t('workbench.session.archive_confirm')}
              description={t('workbench.session.archive_hint')}
              confirmLabel={t('workbench.session.archive_action')}
              cancelLabel={t('common.cancel')}
              onConfirm={() => onDeleteSession(session.id)}>
              <Tooltip content={t('workbench.session.archive')} delay={400} placement="top" sideOffset={6} asChild>
                <button
                  type="button"
                  data-session-action="archive"
                  aria-label={t('workbench.session.archive')}
                  className={ROW_ACTION_CLASS}>
                  <Archive size={14} className="text-current" strokeWidth={1.7} />
                </button>
              </Tooltip>
            </PopConfirm>
          </span>
        )}
      </div>
    )
  }

  const canvasOffset = {
    marginLeft: catalogOverlay || layoutCollapsed ? 0 : WORKBENCH_SIDEBAR_DEFAULT_WIDTH,
    transition: workbenchSidebarSlide()
  }

  return (
    <div
      data-ui="workbench.home"
      className={cn(
        'absolute inset-0 z-20 flex overflow-hidden text-[#171717]',
        homeActive ? '' : 'pointer-events-none'
      )}>
      {typeof document !== 'undefined'
        ? createPortal(
            <div
              ref={dragGhost}
              hidden
              data-ui="workbench.session.drag"
              className="pointer-events-none fixed top-0 left-0 z-[70] max-w-52 truncate rounded-[8px] bg-[#171717] px-2.5 py-1.5 text-[12px] leading-none text-white shadow-[0_8px_24px_rgba(23,23,23,0.18)]"
            />,
            document.body
          )
        : null}
      <aside
        id="workbench-sidebar"
        aria-hidden={layoutCollapsed || undefined}
        inert={layoutCollapsed || undefined}
        className={cn(
          'workbench-sidebar-slide pointer-events-auto absolute inset-y-0 z-20 flex flex-col border-r border-[#efefef] bg-white',
          layoutCollapsed && 'pointer-events-none'
        )}
        style={{
          width: WORKBENCH_SIDEBAR_DEFAULT_WIDTH,
          left: layoutCollapsed ? -WORKBENCH_SIDEBAR_DEFAULT_WIDTH : 0,
          transition: workbenchSidebarSlide()
        }}>
        <div className="flex min-h-0 flex-1 flex-col px-3 pt-2 pb-4">
          <div className="flex items-center gap-2">
            <button
              type="button"
              data-new-task-trigger="true"
              onClick={startNewTask}
              className="flex h-9 min-w-0 flex-1 items-center justify-center gap-1.5 rounded-[8px] bg-[#f5f5f5] text-[14px] font-medium text-[#171717] outline-none hover:bg-[#ececec] focus-visible:ring-2 focus-visible:ring-black/10">
              <MessageSquarePlus className="size-4 text-current" strokeWidth={1.7} />
              <span className="truncate">{t('workbench.nav.new_task')}</span>
            </button>
            <button
              type="button"
              aria-label={t('workbench.search')}
              aria-expanded={searchOpen}
              data-ui="workbench.search.trigger"
              onClick={() => {
                setSearchCursor(0)
                setSearchOpen(true)
              }}
              className="grid size-9 shrink-0 place-items-center rounded-[8px] bg-[#f5f5f5] text-[#171717] outline-none hover:bg-[#ececec] focus-visible:ring-2 focus-visible:ring-black/10">
              <Search className="size-4 text-current" strokeWidth={1.7} />
            </button>
          </div>
          <nav aria-label={t('workbench.nav.menu')} data-ui="workbench.nav" className="mt-3 flex flex-col">
            <button
              type="button"
              data-ui="workbench.nav.connectors"
              aria-current={catalog === 'connector' ? 'page' : undefined}
              onClick={() => {
                setScheduledOpen(false)
                onOpenCatalog?.('connector')
              }}
              className={navClass(catalog === 'connector')}>
              <Plug className="size-4 shrink-0 text-current" strokeWidth={1.7} />
              <span className="min-w-0 flex-1 truncate">{t('workbench.nav.connectors')}</span>
            </button>
            <button
              type="button"
              data-ui="workbench.nav.skills"
              aria-current={catalog === 'skill' ? 'page' : undefined}
              onClick={() => {
                setScheduledOpen(false)
                onOpenCatalog?.('skill')
              }}
              className={navClass(catalog === 'skill')}>
              <Sparkles className="size-4 shrink-0 text-current" strokeWidth={1.7} />
              <span className="min-w-0 flex-1 truncate">{t('workbench.nav.skills')}</span>
            </button>
            <button
              type="button"
              data-ui="workbench.nav.scheduled"
              aria-current={scheduledOpen && !catalogOpen ? 'page' : undefined}
              onClick={() => {
                setScheduledOpen(true)
                onOpenScheduledTasks?.()
              }}
              className={navClass(scheduledOpen && !catalogOpen)}>
              <Clock className="size-4 shrink-0 text-current" strokeWidth={1.7} />
              <span className="min-w-0 flex-1 truncate">{t('workbench.nav.scheduled')}</span>
            </button>
          </nav>

          <div className="mt-3 flex min-h-0 flex-1 flex-col gap-1 overflow-hidden">
            {inbox.pinned.length > 0 && (
              <section role="group" aria-label={t('workbench.tasks.pinned')} data-ui="workbench.tasks.pinned">
                <h3 className="flex h-8 items-center px-2.5 pt-1 text-[12px] leading-none font-normal text-[#8a8a8a]">
                  {t('workbench.tasks.pinned')}
                </h3>
                <div className="flex flex-col gap-0.5">
                  {inbox.pinned.map((session) => renderSession(session, 'recent'))}
                </div>
              </section>
            )}

            <section
              data-ui="workbench.projects"
              aria-label={t('workbench.projects')}
              className="flex max-h-[min(14rem,40%)] min-h-0 shrink-0 flex-col">
              <header className="flex h-8 shrink-0 items-center gap-2 pt-1 pr-1.5 pl-2.5">
                <h3 className="min-w-0 flex-1 truncate text-[12px] leading-none font-normal text-[#8a8a8a]">
                  {t('workbench.projects')}
                </h3>
                {projectWindow.hidden.length > 0 ? (
                  <button
                    type="button"
                    data-ui="workbench.projects.more"
                    aria-label={t('workbench.projects.more_label')}
                    onClick={() => {
                      clearProjectQuery()
                      setProjectCatalogOpen(true)
                    }}
                    className="shrink-0 text-[13px] font-normal text-[#8a8a8a] outline-none hover:text-[#171717]">
                    {t('workbench.projects.more')}
                  </button>
                ) : null}
                <Tooltip content={t('workbench.projects.create')} delay={400} placement="top" sideOffset={6} asChild>
                  <button
                    type="button"
                    data-project-action="create"
                    aria-label={t('workbench.projects.create')}
                    onClick={() => setCreateOpen(true)}
                    className={SECTION_ACTION_CLASS}>
                    <Plus className="size-3.5 text-current" strokeWidth={1.7} />
                  </button>
                </Tooltip>
              </header>
              {projects.length === 0 ? (
                <p data-ui="workbench.sessions.empty" className="px-2.5 py-1.5 text-[13px] leading-5 text-[#8a8a8a]">
                  {t('workbench.projects.empty')}
                </p>
              ) : (
                <div className="flex min-h-0 flex-col gap-0.5 overflow-y-auto">
                  {projectWindow.visible.map((project) => {
                    const expanded = expandedProjectId === project.id
                    const revealed = revealedIds.has(project.id)
                    const containsCurrent =
                      !homeActive && project.sessions.some((session) => session.id === activeSessionId)
                    const visibleTasks = revealed ? project.sessions : project.sessions.slice(0, PROJECT_TASK_PREVIEW)
                    const hiddenCount = project.sessions.length - visibleTasks.length
                    return (
                      <div
                        key={project.id}
                        data-ui="workbench.project"
                        data-project-id={project.id}
                        className="data-[drop-active=true]:[&>div:first-child]:bg-[#f2f3f5] data-[drop-active=true]:[&>div:first-child]:font-medium data-[drop-active=true]:[&>div:first-child]:text-[#171717]">
                        <div className={cn(navClass(Boolean(containsCurrent && !expanded)), 'group/project pr-1.5')}>
                          <button
                            type="button"
                            data-project-action="toggle"
                            aria-expanded={expanded}
                            onClick={() => toggleProject(project.id)}
                            className="flex h-full min-w-0 flex-1 items-center gap-2.5 text-left outline-none">
                            <span className="relative grid size-4 shrink-0 place-items-center">
                              {expanded ? (
                                <FolderOpen
                                  className="size-4 text-current transition-opacity group-hover/project:opacity-0"
                                  strokeWidth={1.7}
                                />
                              ) : (
                                <Folder
                                  className="size-4 text-current transition-opacity group-hover/project:opacity-0"
                                  strokeWidth={1.7}
                                />
                              )}
                              <ChevronRight
                                aria-hidden
                                className={cn(
                                  'absolute size-4 text-[#8a8a8a] opacity-0 transition group-hover/project:opacity-100',
                                  expanded && 'rotate-90'
                                )}
                                strokeWidth={1.7}
                              />
                            </span>
                            <span className="min-w-0 flex-1 truncate">{project.name}</span>
                          </button>
                          <button
                            type="button"
                            data-project-action="new-task"
                            aria-label={t('workbench.projects.new_task')}
                            title={t('workbench.projects.new_task')}
                            onClick={() => {
                              setExpandedProjectId(project.id)
                              onCreateTaskInProject?.(project.id)
                            }}
                            className={cn(
                              SECTION_ACTION_CLASS,
                              'opacity-0 group-focus-within/project:opacity-100 group-hover/project:opacity-100'
                            )}>
                            <Plus className="size-3.5 text-current" strokeWidth={1.7} />
                          </button>
                        </div>
                        {expanded && visibleTasks.length > 0 ? (
                          <div className="mt-0.5 flex flex-col gap-0.5">
                            {visibleTasks.map((session) => renderSession(session, 'project'))}
                            {hiddenCount > 0 && (
                              <button
                                type="button"
                                data-ui="workbench.projects.expand"
                                onClick={() => setRevealedIds((current) => new Set(current).add(project.id))}
                                className={cn(navClass(false, true), 'text-[#8a8a8a]')}>
                                {t('workbench.projects.expand')}
                              </button>
                            )}
                          </div>
                        ) : null}
                      </div>
                    )
                  })}
                </div>
              )}
            </section>

            <section
              data-ui="workbench.tasks"
              aria-label={t('workbench.tasks')}
              className="flex min-h-[9rem] min-w-0 flex-1 flex-col data-[drop-active=true]:rounded-[8px] data-[drop-active=true]:bg-[#f7f7f7]">
              <header className="mt-2 flex h-8 shrink-0 items-center gap-2 pt-1 pr-1.5 pl-2.5">
                <h3 className="min-w-0 flex-1 truncate text-[12px] leading-none font-normal text-[#8a8a8a]">
                  {t('workbench.tasks')}
                </h3>
                <button
                  type="button"
                  data-ui="workbench.tasks.more"
                  aria-label={t('workbench.tasks.more_label')}
                  onClick={() => {
                    clearHistoryQuery()
                    setHistoryOpen(true)
                  }}
                  className="shrink-0 text-[13px] font-normal text-[#8a8a8a] outline-none hover:text-[#171717]">
                  {t('workbench.tasks.more')}
                </button>
              </header>
              {inbox.recent.length === 0 ? (
                inbox.pinned.length === 0 ? (
                  <p data-ui="workbench.tasks.empty" className="px-2.5 py-1.5 text-[13px] leading-5 text-[#8a8a8a]">
                    {t('workbench.tasks.empty')}
                  </p>
                ) : null
              ) : (
                <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
                  {taskDayGroups.map((group) => (
                    <section key={group.id} data-task-date-group={group.id} aria-label={group.label}>
                      <h3 className="px-2.5 pt-2 pb-0.5 text-[12px] leading-none font-normal text-[#8a8a8a]">
                        {group.label}
                      </h3>
                      {group.sessions.map((session) => renderSession(session, 'recent'))}
                    </section>
                  ))}
                </div>
              )}
            </section>
            {settledSessions.length > 0 ? (
              <section
                role="group"
                aria-label={t('workbench.tasks.settled')}
                data-ui="workbench.tasks.settled"
                className="flex max-h-[40%] min-h-0 shrink-0 flex-col">
                <h3 className="px-2.5 pt-2 pb-0.5 text-[12px] leading-none font-normal text-[#8a8a8a]">
                  {t('workbench.tasks.settled')}
                </h3>
                <div className="flex min-h-0 flex-col gap-0.5 overflow-y-auto">
                  {settledSessions.map((session) => renderSession(session, 'settled'))}
                </div>
              </section>
            ) : null}
          </div>
          {settleUndo ? (
            <div
              data-ui="workbench.session.settle.undo"
              role="status"
              className="mt-2 flex h-9 shrink-0 items-center gap-2 rounded-[8px] bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717]">
              <span className="min-w-0 flex-1 truncate">{t('workbench.session.settled_notice')}</span>
              <button
                type="button"
                onClick={undoSettle}
                className="shrink-0 text-[13px] font-normal text-[#8a8a8a] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
                {t('workbench.session.undo')}
              </button>
            </div>
          ) : null}
        </div>
      </aside>
      <SidebarSeamToggle collapsed={layoutCollapsed} onToggle={toggleSidebar} />

      <WorkbenchProjectCreateDialog
        open={createOpen}
        existingNames={projectNames}
        onOpenChange={setCreateOpen}
        onCreate={onCreateProject ? (name) => onCreateProject({ name }) : undefined}
      />

      {searchPresence.present && typeof document !== 'undefined'
        ? createPortal(
            <div
              data-state={searchPresence.state}
              className={cn(
                'a3s-dialog-scrim fixed inset-0 z-[220] flex items-start justify-center bg-black/20 p-6 pt-[14vh]',
                searchPresence.state === 'closed' ? 'pointer-events-none' : 'pointer-events-auto'
              )}
              onMouseDown={closeSearch}>
              <section
                role="dialog"
                aria-modal="true"
                aria-label={t('workbench.search')}
                data-ui="workbench.search"
                data-state={searchPresence.state}
                onMouseDown={(event) => event.stopPropagation()}
                onKeyDown={onSearchKeyDown}
                className="a3s-dialog-surface flex max-h-[70vh] w-[420px] max-w-[calc(100vw-3rem)] flex-col overflow-hidden rounded-[12px] bg-white shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
                <div className="border-b border-[#efefef] px-1">
                  <SearchInput
                    autoFocus
                    value={taskQuery}
                    placeholder={t('workbench.search')}
                    aria-label={t('workbench.search')}
                    aria-autocomplete="list"
                    aria-controls="workbench-search-results"
                    aria-activedescendant={searchActive >= 0 ? `workbench-search-option-${searchActive}` : undefined}
                    aria-expanded={searchOpen}
                    data-ui="workbench.search.input"
                    containerClassName="h-11 rounded-none border-transparent bg-transparent shadow-none has-[[data-slot=input-group-control]:focus-visible]:ring-0"
                    className="text-[14px] placeholder:text-[#8a8a8a]"
                    onChange={(event) => {
                      setTaskQuery(event.target.value)
                      setSearchCursor(0)
                    }}
                    onClear={() => {
                      setTaskQuery('')
                      setSearchCursor(0)
                    }}
                    clearLabel={t('common.clear')}
                  />
                </div>
                <div
                  id="workbench-search-results"
                  role="listbox"
                  aria-label={t('workbench.search')}
                  className="min-h-0 flex-1 overflow-y-auto p-1.5">
                  {taskQuery.trim().length === 0 ? (
                    <p data-ui="workbench.search.hint" className="px-2.5 py-2 text-[13px] text-[#8a8a8a]">
                      {t('workbench.search.hint')}
                    </p>
                  ) : searchResults.length === 0 ? (
                    <p data-ui="workbench.search.empty" className="px-2.5 py-2 text-[13px] text-[#8a8a8a]">
                      {t('workbench.search.empty')}
                    </p>
                  ) : (
                    searchResults.map((session, index) => {
                      const active = index === searchActive
                      return (
                        <button
                          key={session.id}
                          type="button"
                          role="option"
                          id={`workbench-search-option-${index}`}
                          aria-selected={active}
                          ref={
                            active
                              ? (node) => {
                                  node?.scrollIntoView?.({ block: 'nearest' })
                                }
                              : undefined
                          }
                          data-ui="workbench.search.result"
                          data-session-id={session.id}
                          onMouseEnter={() => setSearchCursor(index)}
                          onClick={() => openSearchResult(session.id)}
                          className={cn(
                            'flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
                            active
                              ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                              : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
                          )}>
                          <span className="min-w-0 flex-1 truncate">{highlightQuery(session.name, taskQuery)}</span>
                          <time className="shrink-0 text-[12px] font-normal text-[#8a8a8a]">
                            {relativeTime(session.updatedAt, now, t, locale)}
                          </time>
                        </button>
                      )
                    })
                  )}
                </div>
              </section>
            </div>,
            document.body
          )
        : null}

      {catalogPresence.present && typeof document !== 'undefined'
        ? createPortal(
            <div
              data-state={catalogPresence.state}
              className={cn(
                'a3s-dialog-scrim fixed inset-0 z-[220] flex items-center justify-center bg-black/20 p-6',
                catalogPresence.state === 'closed' ? 'pointer-events-none' : 'pointer-events-auto'
              )}
              onMouseDown={() => setProjectCatalogOpen(false)}>
              <section
                role="dialog"
                aria-modal="true"
                aria-label={t('workbench.projects.catalog')}
                data-ui="workbench.projects.catalog"
                data-state={catalogPresence.state}
                onMouseDown={(event) => event.stopPropagation()}
                className="a3s-dialog-surface flex max-h-[70vh] w-[360px] flex-col overflow-hidden rounded-[12px] bg-white shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
                <header className="flex h-11 shrink-0 items-center gap-2 px-3">
                  <h2 className="min-w-0 flex-1 truncate text-[14px] font-medium text-[#171717]">
                    {t('workbench.projects.catalog')}
                  </h2>
                  <button
                    type="button"
                    aria-label={t('common.close')}
                    onClick={() => setProjectCatalogOpen(false)}
                    className="grid size-7 place-items-center rounded-[6px] text-[#3f3f3f] hover:bg-[#f5f5f5]">
                    <X size={14} strokeWidth={1.7} />
                  </button>
                </header>
                <div className="shrink-0 px-3 pb-2">
                  <SearchInput
                    autoFocus
                    size="sm"
                    value={projectQuery}
                    placeholder={t('workbench.projects.catalog_search')}
                    aria-label={t('workbench.projects.catalog_search')}
                    aria-autocomplete="list"
                    aria-controls="workbench-project-catalog"
                    aria-activedescendant={catalogIndex >= 0 ? `workbench-project-option-${catalogIndex}` : undefined}
                    aria-expanded={projectCatalogOpen}
                    data-ui="workbench.projects.catalog.search"
                    containerClassName={PICKER_FIELD_CLASS}
                    className="text-[13px] placeholder:text-[#8a8a8a]"
                    onChange={(event) => {
                      setProjectQuery(event.target.value)
                      setProjectCursor(0)
                    }}
                    onClear={clearProjectQuery}
                    clearLabel={t('common.clear')}
                    onKeyDown={(event) =>
                      pickerKeyDown(event, {
                        count: visibleCatalog.length,
                        query: projectQuery,
                        onMove: (direction) =>
                          setProjectCursor((cursor) => {
                            const count = visibleCatalog.length
                            const current = Math.min(cursor, count - 1)
                            return (current + direction + count) % count
                          }),
                        onChoose: () => chooseCatalogProject(Math.min(projectCursor, visibleCatalog.length - 1)),
                        onClear: clearProjectQuery,
                        onClose: () => setProjectCatalogOpen(false)
                      })
                    }
                  />
                </div>
                <div
                  id="workbench-project-catalog"
                  role="listbox"
                  aria-label={t('workbench.projects.catalog')}
                  className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
                  {projects.length === 0 ? (
                    <p className="px-2.5 py-2 text-[13px] text-[#8a8a8a]">{t('workbench.projects.empty')}</p>
                  ) : visibleCatalog.length === 0 ? (
                    <p data-ui="workbench.projects.catalog.empty" className="px-2.5 py-2 text-[13px] text-[#8a8a8a]">
                      {t('workbench.projects.catalog_empty')}
                    </p>
                  ) : (
                    visibleCatalog.map((project, index) => (
                      <button
                        key={project.id}
                        type="button"
                        role="option"
                        id={`workbench-project-option-${index}`}
                        aria-selected={index === catalogIndex}
                        data-project-catalog-id={project.id}
                        ref={
                          index === catalogIndex
                            ? (node) => {
                                node?.scrollIntoView?.({ block: 'nearest' })
                              }
                            : undefined
                        }
                        onMouseEnter={() => setProjectCursor(index)}
                        onClick={() => chooseCatalogProject(index)}
                        className={cn(
                          'flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
                          index === catalogIndex
                            ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                            : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
                        )}>
                        <Folder className="size-4 shrink-0 text-current" strokeWidth={1.7} />
                        <span className="min-w-0 flex-1 truncate">{highlightQuery(project.name, projectQuery)}</span>
                      </button>
                    ))
                  )}
                </div>
              </section>
            </div>,
            document.body
          )
        : null}

      {historyPresence.present && typeof document !== 'undefined'
        ? createPortal(
            <div
              data-state={historyPresence.state}
              className={cn(
                'a3s-dialog-scrim fixed inset-0 z-[220] flex items-center justify-center bg-black/20 p-6',
                historyPresence.state === 'closed' ? 'pointer-events-none' : 'pointer-events-auto'
              )}
              onMouseDown={() => setHistoryOpen(false)}>
              <section
                role="dialog"
                aria-modal="true"
                aria-label={t('workbench.tasks.history')}
                data-ui="workbench.tasks.history"
                data-state={historyPresence.state}
                onMouseDown={(event) => event.stopPropagation()}
                className="a3s-dialog-surface flex max-h-[70vh] w-[360px] flex-col overflow-hidden rounded-[12px] bg-white shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
                <header className="flex h-11 shrink-0 items-center gap-2 px-3">
                  <h2 className="min-w-0 flex-1 truncate text-[14px] font-medium text-[#171717]">
                    {t('workbench.tasks.history')}
                  </h2>
                  <button
                    type="button"
                    aria-label={t('common.close')}
                    onClick={() => setHistoryOpen(false)}
                    className="grid size-7 place-items-center rounded-[6px] text-[#3f3f3f] hover:bg-[#f5f5f5]">
                    <X size={14} strokeWidth={1.7} />
                  </button>
                </header>
                <div className="shrink-0 px-3 pb-2">
                  <SearchInput
                    autoFocus
                    size="sm"
                    value={historyQuery}
                    placeholder={t('workbench.tasks.history_search')}
                    aria-label={t('workbench.tasks.history_search')}
                    aria-autocomplete="list"
                    aria-controls="workbench-task-history"
                    aria-activedescendant={historyActiveId ? `workbench-history-option-${historyActiveId}` : undefined}
                    aria-expanded={historyOpen}
                    data-ui="workbench.tasks.history.search"
                    containerClassName={PICKER_FIELD_CLASS}
                    className="text-[13px] placeholder:text-[#8a8a8a]"
                    onChange={(event) => {
                      setHistoryQuery(event.target.value)
                      setHistoryCursor(0)
                    }}
                    onClear={clearHistoryQuery}
                    clearLabel={t('common.clear')}
                    onKeyDown={(event) =>
                      pickerKeyDown(event, {
                        count: visibleHistory.length,
                        query: historyQuery,
                        onMove: (direction) =>
                          setHistoryCursor((cursor) => {
                            const count = visibleHistory.length
                            const current = Math.min(cursor, count - 1)
                            return (current + direction + count) % count
                          }),
                        onChoose: () => {
                          const session = visibleHistory[Math.min(historyCursor, visibleHistory.length - 1)]
                          if (session) chooseHistorySession(session.id)
                        },
                        onClear: clearHistoryQuery,
                        onClose: () => setHistoryOpen(false)
                      })
                    }
                  />
                </div>
                <div
                  id="workbench-task-history"
                  role="listbox"
                  aria-label={t('workbench.tasks.history')}
                  className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
                  {historySessions.length === 0 ? (
                    <p className="px-2.5 py-2 text-[13px] text-[#8a8a8a]">{t('workbench.tasks.empty')}</p>
                  ) : visibleHistory.length === 0 ? (
                    <p data-ui="workbench.tasks.history.empty" className="px-2.5 py-2 text-[13px] text-[#8a8a8a]">
                      {t('workbench.tasks.history_empty')}
                    </p>
                  ) : (
                    visibleHistory.map((session) => renderSession(session, 'history'))
                  )}
                </div>
              </section>
            </div>,
            document.body
          )
        : null}

      {homeActive && paperloomRequest !== null && (
        <main className={cn(WORKBENCH_CANVAS_CLASS, 'overflow-hidden')} style={canvasOffset}>
          <PaperloomPanel
            key={paperloomRequest.requestId}
            {...paperloomRequest}
            onClose={() => setPaperloomRequest(null)}
          />
        </main>
      )}

      {homeActive && paperloomRequest === null && catalogOpen && catalog && (
        <main className={cn(WORKBENCH_CANVAS_CLASS, 'overflow-hidden')} style={canvasOffset}>
          <CapabilitiesPage key={catalog} kind={catalog} onUseSkill={onUseSkill} />
        </main>
      )}

      {homeActive && paperloomRequest === null && scheduledOpen && !catalogOpen && (
        <main className={cn(WORKBENCH_CANVAS_CLASS, 'overflow-hidden')} style={canvasOffset}>
          <TasksSettings
            surface="workbench"
            onCreateWithAgent={() => {
              setScheduledOpen(false)
              onCreateScheduledWithAgent?.()
            }}
          />
        </main>
      )}

      {homeActive && !catalogOpen && !scheduledOpen && (
        <main
          data-ui="workbench.canvas"
          data-composer-dock-layer=""
          className={cn(WORKBENCH_CANVAS_CLASS, 'overflow-y-auto px-8')}
          style={paperloomRequest === null ? canvasOffset : { ...canvasOffset, display: 'none' }}>
          {/* shrink-0 keeps a tall home from being compressed to the viewport, so the canvas can scroll without a bottom cap. A short home still centers. */}
          <div
            className="flex min-h-full w-full shrink-0 flex-col items-center py-8"
            style={{ justifyContent: 'safe center' }}>
            <div className="flex w-full max-w-[920px] flex-col">
              <div className="relative mx-auto w-full max-w-[760px]">
                <h1
                  data-ui="workbench.wordmark"
                  aria-label={studioProductName()}
                  className="workbench-wordmark mb-5 flex justify-center text-center text-[40px] leading-none text-[#1a1a1a]">
                  <StudioProductMark />
                </h1>
                <HomeComposerModeTrack active />
                <HomeComposerTaskBar
                  onFillPrompt={(text) => {
                    setDraft(text)
                    fillWorkbenchComposer(text)
                  }}
                  onCommitDraft={(handoff) => {
                    setDraft(handoff.text)
                    fillWorkbenchComposer(handoff.text)
                    window.dispatchEvent(new CustomEvent(OFFICIAL_WRITING_EVENT, { detail: handoff }))
                  }}
                />
                <HomeComposerModeChips active />
                {useLiveComposer ? (
                  <>
                    <div ref={anchorRef} data-ui="workbench.composer.anchor" className="relative w-full" />
                    <div data-ui="workbench.composer.scope" />
                  </>
                ) : null}
                {!useLiveComposer || (showComposerFallback && !composerAttached) ? (
                  <>
                    <div
                      data-ui="workbench.composer.fallback"
                      data-drag-over={fileDragOver ? 'true' : undefined}
                      onDragEnter={onFallbackDragEnter}
                      onDragOver={onFallbackDragOver}
                      onDragLeave={onFallbackDragLeave}
                      onDrop={onFallbackDrop}
                      className="relative w-full rounded-[14px] border border-[#ececec] bg-white shadow-[0_12px_28px_rgba(32,42,68,0.08)]">
                      <ComposerFileDropOverlay active={fileDragOver} roundedClassName="rounded-[14px]" />
                      <textarea
                        ref={composerRef}
                        value={draft}
                        rows={2}
                        placeholder={composerMode.prompt ?? t('workbench.composer.placeholder')}
                        onChange={(event) => setDraft(event.target.value)}
                        className="w-full resize-none bg-transparent px-[15px] pt-4 pb-2 text-[15px] text-[#171717] outline-none placeholder:text-[#c5c5c5]"
                      />
                      <div className="flex flex-wrap items-center gap-1 px-3 pb-2">
                        <ComposerScreenshotControl onCapture={(file) => queueWorkbenchScreenshot(file)} />
                        <HomeFallbackComposerOptions />
                        <button
                          type="button"
                          aria-label={t('workbench.nav.new_task')}
                          onClick={focusComposer}
                          className="grid size-8 place-items-center rounded-lg text-[#171717] hover:bg-[#f5f5f5]">
                          <Plus size={18} strokeWidth={1.7} />
                        </button>
                        <WorkbenchPermissionPicker
                          permissionMode={shownPermissionMode}
                          onPermissionModeChange={changePermissionMode}
                        />
                        <CodeWorkspacePlacement />
                        <div className="ml-auto flex items-center gap-1">
                          <span className="flex h-8 items-center px-2 text-[13px] text-[#171717]">
                            {t('workbench.composer.model')}
                          </span>
                          <button
                            type="button"
                            aria-label={t('workbench.composer.mic')}
                            className="grid size-8 place-items-center rounded-lg text-[#171717] hover:bg-[#f5f5f5]">
                            <Mic size={16} strokeWidth={1.7} />
                          </button>
                          <button
                            type="button"
                            aria-label={t('workbench.composer.send')}
                            disabled={!canSend}
                            onClick={() => {
                              const mode = displayedTaskPermissionMode(permissionSessionId, memberId)
                              void (async () => {
                                if (permissionAgentId) {
                                  await updateAgent(
                                    {
                                      id: permissionAgentId,
                                      configuration: { permission_mode: mode as PermissionMode }
                                    },
                                    { showSuccessToast: false }
                                  )
                                }
                                const outcome = await requestHomeComposerSend({
                                  surface: 'fallback',
                                  text: draft,
                                  permissionMode: mode
                                })
                                finishFallbackSend(outcome)
                              })()
                            }}
                            className={cn(
                              'grid size-8 place-items-center rounded-[8px] text-white',
                              canSend ? 'bg-[#3a3a3a]' : 'bg-[#d0d0d0]'
                            )}>
                            <ArrowUp size={16} strokeWidth={2} />
                          </button>
                        </div>
                      </div>
                    </div>
                  </>
                ) : null}
              </div>

              <WorkbenchFeatureStrip
                onCommitDraft={(handoff) => {
                  setDraft(handoff.text)
                  fillWorkbenchComposer(handoff.text)
                  window.dispatchEvent(new CustomEvent(OFFICIAL_WRITING_EVENT, { detail: handoff }))
                }}
              />
            </div>
          </div>
        </main>
      )}
      <WorkbenchCommandPalette
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        canSettle={canSettleCurrent}
        onNewTask={startNewTask}
        onSettle={() => {
          if (currentTask) settleSession(currentTask)
        }}
        onFocus={focusPaletteComposer}
        onCycleMode={() => {
          cycleTaskPermissionMode(permissionSessionId, memberId)
        }}
      />
    </div>
  )
}
