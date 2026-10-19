import { Plus } from 'lucide-react'
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import type { MemberOrg } from '@renderer/data/memberOrgs'
import { appletDragShift, appletDragTarget } from '@renderer/pages/agents/appletDrag'
import { appletGlyph, INBOX_APPLET_ID } from '@renderer/pages/agents/WorkbenchApplets'

import { closeAppletLaunchpad, OrgAppletLaunchpad } from './OrgAppletLaunchpad'
import {
  DEFAULT_ORG_FAVORITES,
  openOrgHomeApp,
  readOrgFavorites,
  subscribeOrgFavorites,
  writeOrgFavorites,
  type OrgApplication,
  type OrgHomeApp
} from './orgWorkbenchCatalog'

type Application = OrgApplication
type HomeApp = OrgHomeApp

const ZONES: { id: 'people' | 'finance' | 'rd' | 'admin'; apps: Application[] }[] = [
  { id: 'people', apps: ['leave', 'probation', 'clock'] },
  { id: 'finance', apps: ['expense', 'travel'] },
  { id: 'rd', apps: ['ai-resource'] },
  { id: 'admin', apps: ['approval', 'meeting', 'seal'] }
]

const DRAG_THRESHOLD_PX = 8
const DRAG_CURSOR = 'html[data-favorite-drag], html[data-favorite-drag] * { cursor: grabbing !important; }'

type FavoriteDrag = {
  originX: number
  originY: number
  x: number
  y: number
  moved: boolean
  from: number
  to: number
  grabX: number
  grabY: number
  width: number
  rects: { left: number; top: number; width: number; height: number }[]
  slots: HTMLElement[]
}

function placeFavoriteGhost(node: HTMLElement | null, drag: FavoriteDrag) {
  if (!node) return
  node.style.width = `${drag.width}px`
  node.style.transform = `translate3d(${drag.x - drag.grabX}px, ${drag.y - drag.grabY}px, 0) scale(1.03)`
}

function clearFavoriteShift(slots: readonly HTMLElement[]) {
  for (const slot of slots) {
    slot.style.transition = 'none'
    slot.style.transform = ''
  }
}

function moveFavorite(ids: HomeApp[], from: number, to: number): HomeApp[] {
  if (from === to || from < 0 || to < 0 || from >= ids.length || to >= ids.length) return ids
  const next = ids.slice()
  const [item] = next.splice(from, 1)
  next.splice(to, 0, item)
  return next
}

function FavoriteCard({
  id,
  label,
  icon,
  onOpen,
  onMove
}: {
  id: HomeApp
  label: string
  icon: ReactNode
  onOpen: () => void
  onMove: (from: number, to: number) => void
}) {
  const drag = useRef<FavoriteDrag | null>(null)
  const floatRef = useRef<HTMLDivElement>(null)
  const suppressClick = useRef(false)
  const onOpenRef = useRef(onOpen)
  const onMoveRef = useRef(onMove)
  onOpenRef.current = onOpen
  onMoveRef.current = onMove
  const [lifted, setLifted] = useState(false)

  useLayoutEffect(() => {
    const current = drag.current
    if (!lifted || !current) return
    placeFavoriteGhost(floatRef.current, current)
  }, [lifted])

  useEffect(() => {
    if (!lifted) return
    const previousCursor = document.body.style.cursor
    document.body.style.cursor = 'grabbing'
    const style = document.createElement('style')
    style.dataset.favoriteDrag = 'true'
    style.textContent = DRAG_CURSOR
    document.head.appendChild(style)
    document.documentElement.dataset.favoriteDrag = 'true'
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      const current = drag.current
      if (!current) return
      drag.current = null
      clearFavoriteShift(current.slots)
      suppressClick.current = true
      setLifted(false)
    }
    window.addEventListener('keydown', onKey)
    return () => {
      style.remove()
      delete document.documentElement.dataset.favoriteDrag
      document.body.style.cursor = previousCursor
      window.removeEventListener('keydown', onKey)
    }
  }, [lifted])

  const finish = (event: { clientX: number; clientY: number }) => {
    const current = drag.current
    if (!current) return
    drag.current = null
    clearFavoriteShift(current.slots)
    setLifted(false)
    if (!current.moved) return
    suppressClick.current = true
    const centers = current.rects.map((rect) => ({ x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }))
    const measurable = current.rects.length > 0 && current.rects.every((rect) => rect.width > 0 && rect.height > 0)
    const target = measurable ? appletDragTarget({ x: event.clientX, y: event.clientY }, centers) : current.to
    onMoveRef.current(current.from, target)
  }

  return (
    <li
      data-applet={id}
      data-dragging={lifted ? 'true' : undefined}
      className="relative min-w-0 touch-none"
      onPointerDown={(event) => {
        if (event.button > 0) return
        const list = event.currentTarget.parentElement
        const slots = list
          ? [...list.children].filter(
              (node): node is HTMLElement => node instanceof HTMLElement && node.hasAttribute('data-applet')
            )
          : []
        const rects = slots.map((slot) => {
          const rect = slot.getBoundingClientRect()
          return { left: rect.left, top: rect.top, width: rect.width, height: rect.height }
        })
        const from = slots.indexOf(event.currentTarget)
        const own = rects[Math.max(0, from)]
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
          width: own?.width ?? 0,
          rects,
          slots
        }
      }}
      onPointerMove={(event) => {
        const current = drag.current
        if (!current) return
        current.x = event.clientX
        current.y = event.clientY
        if (!current.moved) {
          if (Math.hypot(event.clientX - current.originX, event.clientY - current.originY) <= DRAG_THRESHOLD_PX) return
          current.moved = true
          setLifted(true)
          // Capturing on press steals the click from the button, so a tap never opens.
          try {
            event.currentTarget.setPointerCapture(event.pointerId)
          } catch {
            // Pointer capture is missing in the test DOM.
          }
        }
        const measurable = current.rects.every((rect) => rect.width > 0 && rect.height > 0)
        if (measurable) {
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
        placeFavoriteGhost(floatRef.current, current)
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
        clearFavoriteShift(current.slots)
        suppressClick.current = true
        setLifted(false)
      }}>
      {lifted ? (
        <div aria-hidden className="h-[72px] rounded-[12px] border border-dashed border-[#d6d6d6] bg-white/70" />
      ) : null}
      <button
        type="button"
        className={`flex h-[72px] w-full items-center gap-3 rounded-[12px] border border-[#ececec] bg-white px-3 text-left text-[14px] text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.04)] hover:bg-[#fafafa] ${
          lifted ? 'invisible absolute inset-0 cursor-grabbing' : 'cursor-pointer'
        }`}
        onClick={() => {
          if (suppressClick.current) {
            suppressClick.current = false
            return
          }
          onOpenRef.current()
        }}>
        <span className="size-11 shrink-0">{icon}</span>
        <span className="min-w-0 truncate">{label}</span>
      </button>
      {lifted
        ? createPortal(
            <div
              ref={floatRef}
              data-ui="org.workbench.favorite.drag"
              aria-hidden
              className="pointer-events-none fixed top-0 left-0 z-[260]">
              <div className="flex h-[72px] w-full items-center gap-3 rounded-[12px] border border-[#e6e6e6] bg-white px-3 text-[14px] text-[#171717] shadow-[0_16px_36px_rgba(23,23,23,0.18)]">
                <span className="size-11 shrink-0">{icon}</span>
                <span className="min-w-0 truncate">{label}</span>
              </div>
            </div>,
            document.body
          )
        : null}
    </li>
  )
}

export function OrgWorkbench({ orgId }: { orgId: string; org?: MemberOrg }) {
  const { t } = useTranslation()
  const favorites = useSyncExternalStore(
    subscribeOrgFavorites,
    () => readOrgFavorites(orgId),
    () => DEFAULT_ORG_FAVORITES
  )
  const [pending, setPending] = useState<HomeApp | null>(null)
  const [launcherPhase, setLauncherPhase] = useState<'closed' | 'open' | 'closing'>('closed')
  const [launchOrigin, setLaunchOrigin] = useState('50% 40%')
  const addRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    setPending(null)
    setLauncherPhase('closed')
  }, [orgId])

  const saveFavorites = (next: HomeApp[]) => {
    writeOrgFavorites(orgId, next)
  }

  const openApplet = (id: HomeApp) => {
    const title = id === INBOX_APPLET_ID ? t('work.inbox.box.todo') : t(`workbench.applet.${id}`)
    void openOrgHomeApp(id, title, t, setPending)
  }

  const openLauncher = () => {
    const rect = addRef.current?.getBoundingClientRect()
    if (rect) setLaunchOrigin(`${rect.left + rect.width / 2}px ${rect.top + rect.height / 2}px`)
    setLauncherPhase('open')
  }

  const closeLauncher = () => {
    setLauncherPhase((current) => closeAppletLaunchpad(current))
  }

  return (
    <section data-ui="org.workbench" className="flex h-full min-h-0 min-w-0 flex-1 flex-col bg-[#f5f6f7]">
      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
        <h2 className="m-0 text-[24px] leading-tight font-bold text-foreground">{t('org.nav.workbench')}</h2>
        <p className="mt-2 mb-5 text-[13px] leading-5 text-[#8a8a8a]">{t('org.workbench.description')}</p>
        <section aria-label={t('org.workbench.favorites')}>
          <header className="mb-3 flex items-center justify-between gap-3">
            <h3 className="m-0 text-[16px] font-medium text-foreground">{t('org.workbench.favorites')}</h3>
            <button
              ref={addRef}
              type="button"
              data-ui="org.workbench.add"
              aria-expanded={launcherPhase !== 'closed'}
              aria-label={t('org.workbench.add')}
              onClick={openLauncher}
              className="inline-flex h-8 items-center gap-1.5 rounded-full border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.04)] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
              <Plus size={15} strokeWidth={1.8} aria-hidden />
              {t('org.workbench.add')}
            </button>
          </header>
          <ul className="m-0 grid list-none grid-cols-2 gap-3 p-0 sm:grid-cols-3 xl:grid-cols-4">
            {favorites.map((id) => {
              const Icon = appletGlyph(id)
              const label = id === INBOX_APPLET_ID ? t('work.inbox.box.todo') : t(`workbench.applet.${id}`)
              return (
                <FavoriteCard
                  key={id}
                  id={id}
                  label={pending === id ? t('workbench.applet.installing') : label}
                  icon={<Icon />}
                  onOpen={() => openApplet(id)}
                  onMove={(from, to) => saveFavorites(moveFavorite(favorites, from, to))}
                />
              )
            })}
          </ul>
        </section>
        <div className="mt-6 flex flex-col gap-6">
          {ZONES.map((zone) => (
            <section key={zone.id} aria-label={t(`org.workbench.zone.${zone.id}`)}>
              <h3 className="m-0 mb-3 text-[16px] font-medium text-foreground">{t(`org.workbench.zone.${zone.id}`)}</h3>
              <ul className="m-0 grid list-none grid-cols-4 gap-1 rounded-[16px] bg-white px-3 py-4 sm:grid-cols-6 lg:grid-cols-8">
                {zone.apps.map((id) => {
                  const Icon = appletGlyph(id)
                  return (
                    <li key={id}>
                      <button
                        type="button"
                        className="flex w-full flex-col items-center gap-2 rounded-[12px] px-1 py-2 text-[13px] text-[#171717] hover:bg-[#f7f7f7]"
                        onClick={() => openApplet(id)}>
                        <span className="size-14">
                          <Icon />
                        </span>
                        <span className="max-w-full truncate">{t(`workbench.applet.${id}`)}</span>
                      </button>
                    </li>
                  )
                })}
              </ul>
            </section>
          ))}
        </div>
      </div>
      <OrgAppletLaunchpad
        orgId={orgId}
        phase={launcherPhase}
        origin={launchOrigin}
        onClose={closeLauncher}
        onExited={() => setLauncherPhase((current) => (current === 'closing' ? 'closed' : current))}
      />
    </section>
  )
}
