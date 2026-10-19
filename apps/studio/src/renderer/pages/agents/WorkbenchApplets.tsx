import { Pin, PinOff, Plus, Search } from 'lucide-react'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { toast } from '@renderer/services/toast'

import { installCapability, listCapabilities, type CapabilityEntry } from '../capabilities/capabilitiesData'
import { pluginInstallOverlay, PLUGIN_INSTALL_EVENT, rememberPluginInstall } from '../capabilities/pluginInstallSync'
import { appletDragShift, appletDragTarget } from './appletDrag'
import {
  HOME_APPLET_PIN_LIMIT,
  HOME_APPLET_SLUG,
  isWorkbenchAppletId,
  readHomeAppletPins,
  reorderHomeAppletPins,
  toggleHomeAppletPin,
  WORKBENCH_APPLET_IDS,
  writeHomeAppletPins,
  type WorkbenchAppletId
} from './homeApplets'
import { toastNotOpen } from './notOpenToast'
import { openWorkbenchAppletWindow } from './openWorkbenchAppletWindow'

import './workbenchComposer.css'

export { WORKBENCH_APPLET_IDS, isWorkbenchAppletId, type WorkbenchAppletId } from './homeApplets'

/** Tools window opened from the handling card. It is not a home-strip applet. */
export const PDF_TOOLS_APPLET_ID = 'pdf' as const

/** Message center. It opens from the rail, not the home strip. */
export const INBOX_APPLET_ID = 'inbox' as const

/** Team notice. It opens from the plugin, not the home strip. */
export const NOTICE_APPLET_ID = 'notice' as const

/** Plan editor. It opens from the create collage, not the home strip. */
export const PLAN_APPLET_ID = 'plan' as const

export type WorkbenchWindowAppletId =
  | WorkbenchAppletId
  | typeof PDF_TOOLS_APPLET_ID
  | typeof INBOX_APPLET_ID
  | typeof NOTICE_APPLET_ID
  | typeof PLAN_APPLET_ID
  | 'probation'
  | 'clock'
  | 'correction'
  | 'ai-resource'

type Applet = {
  id: WorkbenchAppletId
  promptKey: string
  Icon: () => ReactNode
}

const APPLETS: Applet[] = [
  { id: 'approval', promptKey: 'workbench.applet.approval.prompt', Icon: ApprovalIcon },
  { id: 'leave', promptKey: 'workbench.applet.leave.prompt', Icon: LeaveIcon },
  { id: 'travel', promptKey: 'workbench.applet.travel.prompt', Icon: TravelIcon },
  { id: 'expense', promptKey: 'workbench.applet.expense.prompt', Icon: ExpenseIcon },
  { id: 'meeting', promptKey: 'workbench.applet.meeting.prompt', Icon: MeetingIcon },
  { id: 'seal', promptKey: 'workbench.applet.seal.prompt', Icon: SealIcon }
]

export function appletGlyph(id: string) {
  if (id === INBOX_APPLET_ID) return InboxIcon
  if (id === 'probation') return ProbationIcon
  if (id === 'clock' || id === 'correction') return ClockIcon
  if (id === 'ai-resource') return AiResourceIcon
  if (!isWorkbenchAppletId(id)) return ApprovalIcon
  return workbenchApplet(id).Icon
}

export function workbenchApplet(id: WorkbenchAppletId): Applet {
  const applet = APPLETS.find((item) => item.id === id)
  if (!applet) throw new Error(`unknown applet ${id}`)
  return applet
}

function appletTitle(t: (key: string) => string, id: WorkbenchAppletId) {
  return t(`workbench.applet.${id}`)
}

export function WorkbenchApplets() {
  const { t } = useTranslation()
  const notOpen = () => toastNotOpen(t)
  const [pins, setPins] = useState<WorkbenchAppletId[]>(() => readHomeAppletPins(localStorage))
  const [catalog, setCatalog] = useState<Readonly<Record<string, boolean>>>({})
  const [catalogEntries, setCatalogEntries] = useState<readonly CapabilityEntry[]>([])
  const [catalogFailed, setCatalogFailed] = useState(false)
  const [catalogReady, setCatalogReady] = useState(false)
  const [launcherPhase, setLauncherPhase] = useState<'closed' | 'open' | 'closing'>('closed')
  const [launchOrigin, setLaunchOrigin] = useState('50% 72%')
  const [query, setQuery] = useState('')
  const addButtonRef = useRef<HTMLButtonElement>(null)
  const launcherPhaseRef = useRef(launcherPhase)
  const [pending, setPending] = useState<{ id: WorkbenchAppletId; percent: number | null } | null>(null)
  const [installTick, setInstallTick] = useState(0)
  const catalogLoad = useRef(Promise.resolve())
  const catalogRef = useRef(catalog)
  const entriesRef = useRef<CapabilityEntry[]>([])
  catalogRef.current = catalog

  useEffect(() => {
    let live = true
    const task = listCapabilities({ kind: 'plugin', pageSize: 100 })
      .then((page) => {
        const next: Record<string, boolean> = {}
        for (const item of page.items) next[item.slug] = item.installed
        entriesRef.current = page.items
        catalogRef.current = next
        if (!live) return
        setCatalogEntries(page.items)
        setCatalogFailed(false)
        setCatalog(next)
      })
      .catch(() => {
        if (live) setCatalogFailed(true)
      })
      .finally(() => {
        if (live) setCatalogReady(true)
      })
    catalogLoad.current = task.then(
      () => undefined,
      () => undefined
    )
    return () => {
      live = false
    }
  }, [installTick])

  useEffect(() => {
    const onInstall = () => setInstallTick((value) => value + 1)
    window.addEventListener(PLUGIN_INSTALL_EVENT, onInstall)
    return () => window.removeEventListener(PLUGIN_INSTALL_EVENT, onInstall)
  }, [])

  const installed = (id: WorkbenchAppletId) => {
    const slug = HOME_APPLET_SLUG[id]
    const noted = pluginInstallOverlay(slug)
    if (noted !== undefined) return noted
    return catalog[slug] === true
  }

  const installing = useRef(new Set<WorkbenchAppletId>())

  const savePins = (next: WorkbenchAppletId[]) => {
    setPins(next)
    writeHomeAppletPins(localStorage, next)
  }

  const openLauncher = () => {
    const rect = addButtonRef.current?.getBoundingClientRect()
    if (rect) setLaunchOrigin(`${rect.left + rect.width / 2}px ${rect.top + rect.height / 2}px`)
    setLauncherPhase('open')
  }

  const closeLauncher = useCallback(() => {
    setLauncherPhase((current) => {
      if (current !== 'open') return current
      return launchpadMotionReduced() ? 'closed' : 'closing'
    })
  }, [])

  useEffect(() => {
    const previous = launcherPhaseRef.current
    launcherPhaseRef.current = launcherPhase
    if (launcherPhase !== 'closed' || previous === 'closed') return
    setQuery('')
    addButtonRef.current?.focus()
  }, [launcherPhase])

  useEffect(() => {
    if (launcherPhase !== 'open') return
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeLauncher()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [closeLauncher, launcherPhase])

  const installIfNeeded = async (id: WorkbenchAppletId) => {
    await catalogLoad.current
    const slug = HOME_APPLET_SLUG[id]
    if (pluginInstallOverlay(slug) === true || catalogRef.current[slug] === true) return
    if (installing.current.has(id)) return
    installing.current.add(id)
    setPending({ id, percent: null })
    try {
      await installCapability(slug, globalThis.fetch, (progress) => {
        if (progress.total <= 0) return
        const percent = Math.max(0, Math.min(100, Math.round((progress.received / progress.total) * 100)))
        setPending((current) => (current?.id === id ? { id, percent } : current))
      })
      rememberPluginInstall(slug, true)
    } catch {
      toast.error(t('workbench.applet.install_failed'))
    } finally {
      installing.current.delete(id)
      setPending((current) => (current?.id === id ? null : current))
    }
  }

  const openApplet = (id: WorkbenchAppletId) => {
    void (async () => {
      await catalogLoad.current
      const contract = entriesRef.current.find((item) => item.slug === HOME_APPLET_SLUG[id])?.applet
      const opened = contract
        ? await openWorkbenchAppletWindow(id, appletTitle(t, id), contract)
        : await openWorkbenchAppletWindow(id, appletTitle(t, id))
      if (!opened) notOpen()
    })()
    void installIfNeeded(id)
  }

  const pinned = pins.flatMap((id) => {
    const applet = APPLETS.find((item) => item.id === id)
    return applet ? [applet] : []
  })
  const launcherRows = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase()
    const ordered = [...pins, ...WORKBENCH_APPLET_IDS.filter((id) => !pins.includes(id))]
    return ordered.flatMap((id) => {
      if (!launcherShowsApplet(id, catalogEntries, catalogFailed)) return []
      const applet = APPLETS.find((item) => item.id === id)
      if (!applet) return []
      const label = t(`workbench.applet.${id}`)
      if (needle && !`${label} ${id}`.toLocaleLowerCase().includes(needle)) return []
      return [applet]
    })
  }, [catalogEntries, catalogFailed, pins, query, t])

  return (
    <>
      <nav
        aria-label={t('workbench.applet.menu')}
        data-ui="workbench.applets"
        className="relative z-10 mt-8 flex items-start justify-center gap-8">
        {pinned.map((applet) => (
          <HomeStripItem
            key={applet.id}
            applet={applet}
            label={pending?.id === applet.id ? t('workbench.applet.installing') : t(`workbench.applet.${applet.id}`)}
            busy={pending?.id === applet.id}
            percent={pending?.id === applet.id ? pending.percent : null}
            installed={installed(applet.id)}
            installingLabel={t('workbench.applet.installing')}
            onOpen={() => openApplet(applet.id)}
            onDrop={(target) => {
              const next = reorderHomeAppletPins(pins, applet.id, target)
              if (next !== pins) savePins(next)
            }}
          />
        ))}
        <button
          ref={addButtonRef}
          type="button"
          data-ui="workbench.applet.add"
          aria-expanded={launcherPhase !== 'closed'}
          onClick={openLauncher}
          className="flex w-14 flex-col items-center gap-2 text-[12px] leading-none text-[#666] hover:text-[#171717]">
          <span className="grid size-14 place-items-center rounded-full bg-[#f3f3f3]">
            <Plus size={22} strokeWidth={1.6} className="text-[#c4c4c4]" aria-hidden />
          </span>
          <span>{t('workbench.applet.add')}</span>
        </button>
      </nav>
      {launcherPhase !== 'closed' ? (
        <AppletLaunchpad
          phase={launcherPhase}
          origin={launchOrigin}
          rows={launcherRows}
          pins={pins}
          query={query}
          catalogReady={catalogReady}
          onQuery={setQuery}
          onClose={closeLauncher}
          onExited={() => setLauncherPhase((current) => (current === 'closing' ? 'closed' : current))}
          onOpen={(id) => {
            if (!isWorkbenchAppletId(id)) return
            closeLauncher()
            openApplet(id)
          }}
          onToggle={(id) => {
            if (!isWorkbenchAppletId(id)) return
            const next = toggleHomeAppletPin(pins, id)
            if (next === pins) {
              toast.info({ title: t('workbench.applet.pin.full'), appearance: 'pill' })
              return
            }
            savePins(next)
          }}
          onReorder={(next) => {
            const ids = next.filter(isWorkbenchAppletId)
            if (ids.length === next.length) savePins(ids)
          }}
        />
      ) : null}
    </>
  )
}

export type LaunchpadApplet = {
  id: string
  Icon: () => ReactNode
  label?: string
}

/** macOS-style fullscreen applet board. The home strip and the team workbench share it. */
export function AppletLaunchpad({
  phase,
  origin,
  rows,
  pins,
  query,
  catalogReady,
  pinLimit = HOME_APPLET_PIN_LIMIT,
  onQuery,
  onClose,
  onExited,
  onOpen,
  onToggle,
  onReorder
}: {
  phase: 'open' | 'closing'
  origin: string
  rows: readonly LaunchpadApplet[]
  pins: readonly string[]
  query: string
  catalogReady: boolean
  pinLimit?: number
  onQuery: (value: string) => void
  onClose: () => void
  onExited: () => void
  onOpen: (id: string) => void
  onToggle: (id: string) => void
  onReorder: (next: string[]) => void
}) {
  const { t } = useTranslation()
  const searchRef = useRef<HTMLInputElement>(null)
  const [grid, setGrid] = useState(readLaunchpadGrid)
  const [page, setPage] = useState(0)
  const pageSize = grid.cols * grid.rows
  const pageCount = Math.max(1, Math.ceil(rows.length / pageSize))
  const safePage = Math.min(page, pageCount - 1)
  const visible = rows.slice(safePage * pageSize, safePage * pageSize + pageSize)
  const host = typeof document === 'undefined' ? null : document.body

  useEffect(() => {
    const apply = () => setGrid(readLaunchpadGrid())
    window.addEventListener('resize', apply)
    return () => window.removeEventListener('resize', apply)
  }, [])

  useEffect(() => {
    setPage(0)
  }, [query])

  useEffect(() => {
    if (phase !== 'open') return
    searchRef.current?.focus()
  }, [phase])

  useEffect(() => {
    if (phase === 'closing') return
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose()
        return
      }
      if (event.target instanceof HTMLInputElement) return
      if (event.key === 'ArrowRight') setPage((current) => Math.min(pageCount - 1, current + 1))
      if (event.key === 'ArrowLeft') setPage((current) => Math.max(0, current - 1))
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose, pageCount, phase])

  const dropPin = (id: string, point: { x: number; y: number }) => {
    if (query.trim()) return
    const target = launchpadIdAtPoint(point, id)
    if (!target || !pins.includes(id) || !pins.includes(target) || id === target) return
    const next = [...pins]
    const from = next.indexOf(id)
    const to = next.indexOf(target)
    const [item] = next.splice(from, 1)
    next.splice(to, 0, item)
    onReorder(next)
  }

  const sheet = (
    <div
      role="dialog"
      aria-label={t('workbench.applet.launcher')}
      data-ui="workbench.applet.launcher"
      data-state={phase}
      data-ready={catalogReady ? 'true' : 'false'}
      className="workbench-launchpad fixed inset-0 z-[200] backdrop-blur-[72px]"
      style={{ background: LAUNCHPAD_FIELD }}
      onMouseDown={onClose}>
      <div
        data-ui="workbench.applet.launcher.stage"
        className="workbench-launchpad-stage pointer-events-none flex h-full flex-col"
        style={{ transformOrigin: origin }}
        onAnimationEnd={(event) => {
          if (event.target !== event.currentTarget) return
          if (event.animationName !== 'workbench-launchpad-out') return
          onExited()
        }}>
        <label
          className="pointer-events-auto mx-auto mt-8 flex h-7 w-[200px] items-center gap-1.5 rounded-full bg-black/40 px-3 text-white/70"
          onMouseDown={(event) => event.stopPropagation()}>
          <Search size={13} strokeWidth={1.8} aria-hidden />
          <input
            ref={searchRef}
            value={query}
            onChange={(event) => onQuery(event.target.value)}
            placeholder={t('workbench.applet.search')}
            aria-label={t('workbench.applet.search')}
            className="h-full min-w-0 flex-1 bg-transparent text-[13px] text-white outline-none placeholder:text-white/60"
          />
        </label>
        <div
          data-ui="workbench.applet.launcher.grid"
          className={
            rows.length === 0
              ? 'flex min-h-0 flex-1 items-center justify-center px-10'
              : 'flex min-h-0 flex-1 items-start justify-center px-10 pt-8'
          }>
          {rows.length === 0 ? (
            <p className="text-center text-[13px] text-white/80">{t('workbench.applet.search.empty')}</p>
          ) : (
            <ul
              className="m-0 grid list-none content-start gap-x-6 gap-y-8 p-0"
              style={{ gridTemplateColumns: `repeat(${grid.cols}, 128px)` }}>
              {visible.map((applet) => (
                <LaunchpadTile
                  key={applet.id}
                  applet={applet}
                  label={applet.label ?? t(`workbench.applet.${applet.id}`)}
                  pinned={pins.includes(applet.id)}
                  pinLabel={
                    pins.includes(applet.id)
                      ? t('workbench.applet.unpin')
                      : pins.length >= pinLimit
                        ? t('workbench.applet.pin.full')
                        : t('workbench.applet.pin')
                  }
                  onOpen={() => onOpen(applet.id)}
                  onToggle={() => onToggle(applet.id)}
                  onDrop={(point) => dropPin(applet.id, point)}
                />
              ))}
            </ul>
          )}
        </div>
        <div
          data-ui="workbench.applet.launcher.pages"
          className="pointer-events-auto mb-5 flex h-4 items-center justify-center gap-1.5"
          onMouseDown={(event) => event.stopPropagation()}>
          {Array.from({ length: pageCount }, (_, index) => (
            <button
              key={index}
              type="button"
              aria-current={index === safePage ? 'page' : undefined}
              aria-label={String(index + 1)}
              onClick={() => setPage(index)}
              className={
                index === safePage
                  ? 'size-[7px] rounded-full bg-white'
                  : 'size-1.5 rounded-full bg-white/45 hover:bg-white/70'
              }
            />
          ))}
        </div>
      </div>
    </div>
  )
  return host ? createPortal(sheet, host) : sheet
}

function LaunchpadTile({
  applet,
  label,
  pinned,
  pinLabel,
  onOpen,
  onToggle,
  onDrop
}: {
  applet: LaunchpadApplet
  label: string
  pinned: boolean
  pinLabel: string
  onOpen: () => void
  onToggle: () => void
  onDrop: (point: { x: number; y: number }) => void
}) {
  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null)
  const suppressClick = useRef(false)
  return (
    <li
      data-applet={applet.id}
      className="group pointer-events-auto relative flex w-[128px] flex-col items-center"
      onMouseDown={(event) => event.stopPropagation()}
      onPointerDown={(event) => {
        if ((event.target as HTMLElement).closest('[data-pin]')) return
        drag.current = { x: event.clientX, y: event.clientY, moved: false }
      }}
      onPointerMove={(event) => {
        const current = drag.current
        if (!current) return
        if (Math.hypot(event.clientX - current.x, event.clientY - current.y) > 8) current.moved = true
      }}
      onPointerUp={(event) => {
        const current = drag.current
        drag.current = null
        if (!current?.moved) return
        suppressClick.current = true
        onDrop({ x: event.clientX, y: event.clientY })
      }}>
      <button
        type="button"
        onClick={() => {
          if (suppressClick.current) {
            suppressClick.current = false
            return
          }
          onOpen()
        }}
        className="flex w-full flex-col items-center gap-2 rounded-2xl px-1 py-1 transition-transform duration-200 outline-none hover:scale-110 focus-visible:scale-110 active:scale-95">
        <span className="size-20 drop-shadow-[0_12px_18px_rgba(0,0,0,0.32)]">
          <applet.Icon />
        </span>
        <span className="w-full truncate text-center text-[12px] leading-none text-white [text-shadow:0_1px_2px_rgba(0,0,0,0.85)]">
          {label}
        </span>
      </button>
      <button
        type="button"
        data-pin=""
        aria-pressed={pinned}
        aria-label={pinLabel}
        onClick={onToggle}
        className="absolute top-0 right-2 grid size-6 place-items-center rounded-full border border-white bg-[#171717] text-white opacity-0 shadow-[0_1px_3px_rgba(23,23,23,0.45)] outline-none group-hover:opacity-100 hover:bg-black focus-visible:opacity-100">
        {pinned ? (
          <PinOff size={14} strokeWidth={2} className="text-white" aria-hidden />
        ) : (
          <Pin size={14} strokeWidth={2} className="text-white" aria-hidden />
        )}
      </button>
    </li>
  )
}

const LAUNCHPAD_FIELD = [
  'radial-gradient(80% 60% at 8% 12%, rgba(232, 214, 176, 0.72), transparent 58%)',
  'radial-gradient(55% 45% at 42% 8%, rgba(168, 186, 168, 0.35), transparent 60%)',
  'radial-gradient(70% 55% at 86% 18%, rgba(126, 176, 220, 0.62), transparent 56%)',
  'radial-gradient(80% 70% at 62% 88%, rgba(28, 86, 176, 0.88), transparent 62%)',
  'radial-gradient(50% 40% at 22% 72%, rgba(72, 124, 186, 0.55), transparent 64%)',
  'linear-gradient(160deg, rgba(88, 104, 112, 0.55), rgba(24, 58, 120, 0.72))'
].join(',')

export function launchpadMotionReduced(): boolean {
  if (typeof window.matchMedia !== 'function') return false
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

function readLaunchpadGrid() {
  if (typeof window === 'undefined') return { cols: 7, rows: 5 }
  return { cols: launchpadColumns(window.innerWidth), rows: launchpadRows(window.innerHeight) }
}

function launchpadColumns(width: number) {
  const available = Math.max(128, width - 80)
  const count = Math.floor((available + 24) / (128 + 24))
  return Math.max(1, Math.min(7, count))
}

function launchpadRows(height: number) {
  if (height >= 760) return 5
  if (height >= 560) return 3
  return 2
}

/** A catalog row without `ui` is not a mini app. A missing row keeps the built-in mini app. */
function launcherShowsApplet(
  id: WorkbenchAppletId,
  entries: readonly CapabilityEntry[],
  catalogFailed: boolean
): boolean {
  if (catalogFailed || entries.length === 0) return true
  const entry = entries.find((item) => item.slug === HOME_APPLET_SLUG[id])
  if (!entry || !Array.isArray(entry.surfaces)) return true
  return entry.surfaces.includes('ui')
}

function launchpadIdAtPoint(point: { x: number; y: number }, ignore: string): string | null {
  const seen = typeof document.elementsFromPoint === 'function' ? document.elementsFromPoint(point.x, point.y) : []
  const fallback = typeof document.elementFromPoint === 'function' ? document.elementFromPoint(point.x, point.y) : null
  const nodes = seen.length > 0 ? seen : [fallback]
  for (const node of nodes) {
    if (!node) continue
    const id = node.closest('[data-applet]')?.getAttribute('data-applet')
    if (id && id !== ignore) return id
  }
  return null
}

function appletIdAtPoint(point: { x: number; y: number }, ignore: string): WorkbenchAppletId | null {
  const seen = typeof document.elementsFromPoint === 'function' ? document.elementsFromPoint(point.x, point.y) : []
  const fallback = typeof document.elementFromPoint === 'function' ? document.elementFromPoint(point.x, point.y) : null
  const nodes = seen.length > 0 ? seen : [fallback]
  for (const node of nodes) {
    if (!node) continue
    const id = node.closest('[data-applet]')?.getAttribute('data-applet')
    if (id && id !== ignore && isWorkbenchAppletId(id)) return id
  }
  return null
}

const APPLET_DRAG_THRESHOLD_PX = 8
const APPLET_DRAG_LIFT = 'scale(1.06)'
const APPLET_DRAG_CURSOR = 'html[data-applet-drag], html[data-applet-drag] * { cursor: grabbing !important; }'

type StripDrag = {
  originX: number
  originY: number
  x: number
  y: number
  moved: boolean
  from: number
  to: number
  grabX: number
  grabY: number
  rects: { left: number; top: number; width: number; height: number }[]
  measurable: boolean
  slots: HTMLElement[]
}

function appletSlots(nav: Element): HTMLElement[] {
  const slots: HTMLElement[] = []
  for (const node of nav.children) {
    if (node instanceof HTMLElement && node.hasAttribute('data-applet')) slots.push(node)
  }
  return slots
}

function clearAppletShift(slots: readonly HTMLElement[]) {
  for (const slot of slots) {
    slot.style.transition = 'none'
    slot.style.transform = ''
  }
}

function placeAppletGhost(node: HTMLElement | null, x: number, y: number, grabX: number, grabY: number) {
  if (!node) return
  node.style.transform = `translate3d(${x - grabX}px, ${y - grabY}px, 0) ${APPLET_DRAG_LIFT}`
}

function HomeStripItem({
  applet,
  label,
  busy,
  percent,
  installed,
  installingLabel,
  onOpen,
  onDrop
}: {
  applet: Applet
  label: string
  busy: boolean
  percent: number | null
  installed: boolean
  installingLabel: string
  onOpen: () => void
  onDrop: (target: WorkbenchAppletId) => void
}) {
  const drag = useRef<StripDrag | null>(null)
  const floatRef = useRef<HTMLDivElement>(null)
  const suppressClick = useRef(false)
  const onOpenRef = useRef(onOpen)
  const onDropRef = useRef(onDrop)
  onOpenRef.current = onOpen
  onDropRef.current = onDrop
  const [lifted, setLifted] = useState(false)

  useLayoutEffect(() => {
    const current = drag.current
    if (!lifted || !current) return
    placeAppletGhost(floatRef.current, current.x, current.y, current.grabX, current.grabY)
  }, [lifted])

  useEffect(() => {
    if (!lifted) return
    const previousCursor = document.body.style.cursor
    document.body.style.cursor = 'grabbing'
    const style = document.createElement('style')
    style.dataset.appletDrag = 'true'
    style.textContent = APPLET_DRAG_CURSOR
    document.head.appendChild(style)
    document.documentElement.dataset.appletDrag = 'true'
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      const current = drag.current
      if (!current) return
      drag.current = null
      clearAppletShift(current.slots)
      suppressClick.current = true
      setLifted(false)
    }
    window.addEventListener('keydown', onKey)
    return () => {
      style.remove()
      delete document.documentElement.dataset.appletDrag
      document.body.style.cursor = previousCursor
      window.removeEventListener('keydown', onKey)
    }
  }, [lifted])

  const finish = (event: { clientX: number; clientY: number }) => {
    const current = drag.current
    if (!current) return
    drag.current = null
    clearAppletShift(current.slots)
    setLifted(false)
    if (!current.moved) {
      suppressClick.current = true
      onOpenRef.current()
      return
    }
    suppressClick.current = true
    const point = { x: event.clientX, y: event.clientY }
    const target = current.measurable
      ? current.to === current.from
        ? null
        : current.slots[current.to]?.getAttribute('data-applet')
      : appletIdAtPoint(point, applet.id)
    if (!isWorkbenchAppletId(target)) return
    onDropRef.current(target)
  }

  return (
    <div
      data-applet={applet.id}
      data-dragging={lifted ? 'true' : undefined}
      className={lifted ? 'invisible touch-none' : 'touch-none'}
      onPointerDown={(event) => {
        if (event.button > 0) return
        const nav = event.currentTarget.parentElement
        const slots = nav ? appletSlots(nav) : []
        const rects = slots.map((slot) => {
          const rect = slot.getBoundingClientRect()
          return { left: rect.left, top: rect.top, width: rect.width, height: rect.height }
        })
        const from = slots.findIndex((slot) => slot.getAttribute('data-applet') === applet.id)
        const own = rects[from]
        drag.current = {
          originX: event.clientX,
          originY: event.clientY,
          x: event.clientX,
          y: event.clientY,
          moved: false,
          from: Math.max(0, from),
          to: Math.max(0, from),
          grabX: own ? event.clientX - own.left : 0,
          grabY: own ? event.clientY - own.top : 0,
          rects,
          measurable: rects.length > 0 && rects.every((rect) => rect.width > 0 && rect.height > 0),
          slots
        }
        try {
          event.currentTarget.setPointerCapture(event.pointerId)
        } catch {
          // Pointer capture is missing in the test DOM.
        }
      }}
      onPointerMove={(event) => {
        const current = drag.current
        if (!current) return
        current.x = event.clientX
        current.y = event.clientY
        if (!current.moved) {
          const distance = Math.hypot(event.clientX - current.originX, event.clientY - current.originY)
          if (distance <= APPLET_DRAG_THRESHOLD_PX) return
          current.moved = true
          setLifted(true)
        }
        if (current.measurable) {
          const centers = current.rects.map((rect) => ({
            x: rect.left + rect.width / 2,
            y: rect.top + rect.height / 2
          }))
          const next = appletDragTarget({ x: event.clientX, y: event.clientY }, centers)
          if (next !== current.to) {
            current.to = next
            const reduce =
              typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
            const transition = reduce ? 'none' : 'transform 180ms cubic-bezier(0.2, 0, 0, 1)'
            current.slots.forEach((slot, index) => {
              const shift = appletDragShift(index, current.from, current.to, current.rects)
              slot.style.transition = transition
              slot.style.transform = shift.x === 0 && shift.y === 0 ? '' : `translate3d(${shift.x}px, ${shift.y}px, 0)`
            })
          }
        }
        placeAppletGhost(floatRef.current, current.x, current.y, current.grabX, current.grabY)
      }}
      onPointerUp={(event) => {
        try {
          event.currentTarget.releasePointerCapture(event.pointerId)
        } catch {
          // Pointer capture is missing in the test DOM.
        }
        finish(event)
      }}
      onPointerCancel={() => {
        const current = drag.current
        if (!current) return
        drag.current = null
        clearAppletShift(current.slots)
        suppressClick.current = true
        setLifted(false)
      }}>
      <button
        type="button"
        data-ui={`workbench.applet.${applet.id}`}
        data-installed={installed ? 'true' : 'false'}
        aria-busy={busy || undefined}
        onClick={() => {
          if (suppressClick.current) {
            suppressClick.current = false
            return
          }
          onOpen()
        }}
        className={`flex w-14 touch-none flex-col items-center gap-2 text-[12px] leading-none text-[#666] select-none hover:text-[#171717] ${
          lifted ? 'cursor-grabbing' : 'cursor-pointer'
        }`}>
        <span className="relative size-14">
          <applet.Icon />
          {busy ? (
            <span
              data-ui="workbench.applet.installing"
              role="progressbar"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={percent ?? undefined}
              aria-label={installingLabel}
              className="absolute inset-0 grid place-items-center rounded-full bg-white/80 text-[11px] font-medium text-[#171717]">
              {percent == null ? '…' : `${percent}%`}
            </span>
          ) : null}
        </span>
        <span>{label}</span>
      </button>
      {lifted
        ? createPortal(
            <div
              ref={floatRef}
              data-ui="workbench.applet.drag"
              data-applet={applet.id}
              aria-hidden
              className="pointer-events-none fixed top-0 left-0 z-[260] w-14 drop-shadow-[0_14px_22px_rgba(23,23,23,0.2)]">
              <span className="flex flex-col items-center gap-2 text-[12px] leading-none text-[#171717]">
                <span className="relative size-14">
                  <applet.Icon />
                </span>
                <span>{label}</span>
              </span>
            </div>,
            document.body
          )
        : null}
    </div>
  )
}

function InboxIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#EEF3FF" />
      <rect x="15" y="20" width="26" height="18" rx="4" fill="#3B6FE0" />
      <path d="M15 24.5 28 33l13-8.5" fill="none" stroke="#fff" strokeWidth="1.8" strokeLinejoin="round" />
    </svg>
  )
}

function ProbationIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#E8F7EF" />
      <circle cx="28" cy="22" r="6" fill="#1F8A4C" />
      <path d="M16 40c1.6-6 6-9 12-9s10.4 3 12 9" fill="#1F8A4C" />
    </svg>
  )
}

function ClockIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#FFF4E8" />
      <circle cx="28" cy="28" r="11" fill="none" stroke="#E07A1F" strokeWidth="2.2" />
      <path d="M28 21.5V28l4.5 3" fill="none" stroke="#E07A1F" strokeWidth="2" strokeLinecap="round" />
    </svg>
  )
}

function AiResourceIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#F3EEFF" />
      <rect x="16" y="18" width="24" height="20" rx="4" fill="#6E5CE6" />
      <path d="M22 28h12M28 22v12" stroke="#fff" strokeWidth="1.8" strokeLinecap="round" />
    </svg>
  )
}

function ApprovalIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#F4F1FF" />
      <rect x="16" y="15" width="20" height="24" rx="4" fill="#6E5CE6" />
      <rect x="20" y="21" width="11" height="2" rx="1" fill="#fff" />
      <rect x="20" y="26" width="8" height="2" rx="1" fill="#fff" opacity="0.75" />
      <circle cx="35" cy="35" r="8" fill="#F04B55" />
      <path
        d="M31.6 35.1 33.8 37.3 38.4 32.6"
        fill="none"
        stroke="#fff"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

function LeaveIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#EEF3FF" />
      <rect x="15" y="18" width="26" height="22" rx="4" fill="#3B78F0" />
      <rect x="15" y="18" width="26" height="7" rx="4" fill="#2C62D6" />
      <rect x="21" y="14.5" width="2.4" height="7" rx="1.2" fill="#2C62D6" />
      <rect x="32.6" y="14.5" width="2.4" height="7" rx="1.2" fill="#2C62D6" />
      <rect x="20" y="29" width="6" height="2" rx="1" fill="#fff" />
      <rect x="20" y="33.5" width="10" height="2" rx="1" fill="#fff" opacity="0.8" />
    </svg>
  )
}

function ExpenseIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#FFF3EA" />
      <path
        d="M18 16h20a2 2 0 0 1 2 2v22l-3.2-2.1-3.1 2.1-3.2-2.1-3.1 2.1-3.2-2.1L21 40l-3-2V18a2 2 0 0 1 2-2Z"
        fill="#F08A3A"
      />
      <rect x="23" y="22" width="12" height="2" rx="1" fill="#fff" />
      <rect x="23" y="27" width="8" height="2" rx="1" fill="#fff" opacity="0.85" />
      <rect x="23" y="32" width="10" height="2" rx="1" fill="#fff" opacity="0.7" />
    </svg>
  )
}

function TravelIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#E7F6FB" />
      <rect x="18" y="22" width="20" height="14" rx="3" fill="#2A9BC4" />
      <rect x="22" y="18" width="12" height="6" rx="2" fill="#1E86AE" />
      <rect x="20" y="36" width="5" height="3" rx="1" fill="#1E86AE" />
      <rect x="31" y="36" width="5" height="3" rx="1" fill="#1E86AE" />
      <rect x="24" y="26" width="8" height="2" rx="1" fill="#fff" />
    </svg>
  )
}

function MeetingIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#E8F7F1" />
      <circle cx="21" cy="22" r="4" fill="#2F9B73" />
      <circle cx="35" cy="22" r="4" fill="#247A5A" />
      <rect x="15" y="28" width="26" height="11" rx="4" fill="#2F9B73" />
      <rect x="22" y="32" width="12" height="2" rx="1" fill="#fff" />
    </svg>
  )
}

function SealIcon() {
  return (
    <svg viewBox="0 0 56 56" className="size-full" aria-hidden>
      <circle cx="28" cy="28" r="28" fill="#FDECEC" />
      <circle cx="28" cy="27" r="12" fill="#E15B5B" />
      <circle cx="28" cy="27" r="7" fill="none" stroke="#fff" strokeWidth="1.6" />
      <rect x="24" y="37" width="8" height="4" rx="1" fill="#C94545" />
    </svg>
  )
}
