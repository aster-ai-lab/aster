import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuItemContent,
  ContextMenuTrigger,
  Popover,
  PopoverContent,
  PopoverTrigger
} from '@a3s-lab/ui'
import { ChevronRight, PinOff } from 'lucide-react'
import type { PointerEvent as ReactPointerEvent, ReactNode } from 'react'
import { useEffect, useId, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { useMemberProfile } from '@renderer/components/account/memberProfile'
import { DocumentFileIcon } from '@renderer/components/icons/DocumentFileIcon'
import { readAixCatalogue, subscribeAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import { packageHasHomeCards, reconcileHomePluginPins, unpinHomePlugin, useHomePluginPins } from './homePins'
import {
  HOME_SHELF_COLUMNS,
  HOME_SHELF_ROW_PX,
  defaultHomeShelfLayout,
  hasSavedHomeShelfLayout,
  readHomeShelfLayout,
  settleHomeShelfLayout,
  stackedHomeShelfLayout,
  writeHomeShelfLayout,
  type HomeShelfId,
  type HomeShelfPlacement
} from './homeShelfLayout'
import { toastNotOpen } from './notOpenToast'
import {
  OFFICIAL_WRITING_TYPES,
  writingChoiceOn,
  type OfficialWritingHandoff,
  type OfficialWritingTypeId
} from './officialWriting'
import { OfficialWritingDialog } from './OfficialWritingDialog'
import { openWorkbenchAppletWindow } from './openWorkbenchAppletWindow'
import { dropPdfToolHandoff, stagePdfToolHandoff } from './pdfToolHandoff'
import { intakeAccept, stagePdfToolKind, type PdfToolKind } from './pdfToolsJob'
import { PDF_TOOLS_APPLET_ID, PLAN_APPLET_ID } from './WorkbenchApplets'
import type { WritingShelf } from './writingCenterData'

const WRITE_MARK: Record<
  WritingShelf,
  { icon: 'medal' | 'grid' | 'square'; color: string; typeId: OfficialWritingTypeId; tagKey: string }
> = {
  notice: { icon: 'medal', color: '#69b9c9', typeId: 'notice', tagKey: 'workbench.feature.write.blessing_tag' },
  summary: { icon: 'grid', color: '#4c6fff', typeId: 'report', tagKey: 'workbench.feature.write.career_tag' },
  request: { icon: 'square', color: '#69b9c9', typeId: 'request', tagKey: 'workbench.feature.write.plan_tag' },
  minutes: { icon: 'square', color: '#e07848', typeId: 'minutes', tagKey: 'workbench.feature.write.review_tag' }
}

type HomeCard = NonNullable<AixWorkbenchCatalogue['packages'][number]['home']>[number]

function writingShelf(typeId: 'notice' | 'report' | 'request' | 'minutes'): WritingShelf {
  if (typeId === 'report') return 'summary'
  return typeId
}

function useHomeCatalogue(): AixWorkbenchCatalogue {
  return useSyncExternalStore(subscribeAixCatalogue, readAixCatalogue, readAixCatalogue)
}

function homeFace(cards: readonly HomeCard[]): 'write' | 'create' | 'process' | 'cards' | 'launch' {
  if (cards.length === 0) return 'launch'
  const shelf = cards[0].shelf
  if (cards.every((card) => card.shelf === shelf)) return shelf
  return 'cards'
}

function WriteMark() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden>
      <path
        d="M3.2 12.6 6.4 11.4 12.2 5.6 10.4 3.8 4.6 9.6 3.2 12.6Z"
        fill="none"
        stroke="#69b9c9"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
      <path d="M9.6 4.6 11.4 6.4" stroke="#69b9c9" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  )
}

function CreateMark() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden>
      <rect x="2.2" y="2.2" width="11.6" height="11.6" rx="3" fill="none" stroke="#e07848" strokeWidth="1.4" />
      <path d="M8 5.2v5.6M5.2 8h5.6" stroke="#e07848" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  )
}

function CreateCover({ kind }: { kind: 'ppt' | 'site' | 'video' | 'image' }) {
  const raw = useId().replace(/:/g, '')
  if (kind === 'ppt') {
    return (
      <svg
        viewBox="0 0 160 120"
        className="absolute inset-0 h-full w-full"
        preserveAspectRatio="xMidYMid slice"
        aria-hidden>
        <defs>
          <linearGradient id={`${raw}-ppt`} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#3a2a24" />
            <stop offset="1" stopColor="#e07848" />
          </linearGradient>
        </defs>
        <rect width="160" height="120" fill={`url(#${raw}-ppt)`} />
        <rect x="14" y="34" width="78" height="48" rx="4" fill="#fff" opacity="0.55" transform="rotate(-8 53 58)" />
        <rect x="58" y="22" width="86" height="54" rx="5" fill="#fff" />
        <rect x="68" y="32" width="34" height="5" rx="2" fill="#171717" />
        <rect x="68" y="42" width="52" height="3" rx="1.5" fill="#eadfd8" />
        <rect x="68" y="50" width="40" height="3" rx="1.5" fill="#eadfd8" />
        <rect x="108" y="58" width="8" height="12" rx="1" fill="#e07848" />
        <rect x="118" y="50" width="8" height="20" rx="1" fill="#f0b08a" />
        <rect x="128" y="44" width="8" height="26" rx="1" fill="#171717" />
      </svg>
    )
  }
  if (kind === 'site') {
    return (
      <svg
        viewBox="0 0 160 120"
        data-art="app"
        className="absolute inset-0 h-full w-full"
        preserveAspectRatio="xMidYMid slice"
        aria-hidden>
        <rect width="160" height="120" fill="#f6f1ea" />
        <rect x="6" y="18" width="72" height="100" rx="18" fill="#fff" opacity="0.55" transform="rotate(-7 42 68)" />
        <rect x="22" y="-8" width="156" height="124" rx="22" fill="#fff" stroke="#171717" strokeWidth="1.6" />
        <rect x="36" y="10" width="22" height="3.2" rx="1.6" fill="#eadfd8" />
        <rect x="36" y="44" width="108" height="22" rx="5" fill="#4c6fff" />
        <rect x="36" y="72" width="32" height="26" rx="6" fill="#e07848" />
        <rect x="72" y="72" width="32" height="26" rx="6" fill="#69b9c9" />
        <rect x="108" y="72" width="32" height="26" rx="6" fill="#e7ecff" />
        <rect x="74" y="106" width="36" height="4" rx="2" fill="#171717" />
      </svg>
    )
  }
  if (kind === 'video') {
    return (
      <svg
        viewBox="0 0 160 120"
        data-art="interface"
        className="absolute inset-0 h-full w-full"
        preserveAspectRatio="xMidYMid slice"
        aria-hidden>
        <defs>
          <linearGradient id={`${raw}-ui`} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="#1e2430" />
            <stop offset="1" stopColor="#3d4d68" />
          </linearGradient>
        </defs>
        <rect width="160" height="120" fill={`url(#${raw}-ui)`} />
        <rect x="26" y="14" width="108" height="80" rx="8" fill="#f4f5f7" />
        <rect x="32" y="22" width="16" height="64" rx="3" fill="#fff" />
        <rect x="36" y="28" width="8" height="8" rx="2" fill="#4c6fff" />
        <rect x="36" y="40" width="8" height="8" rx="2" fill="#e7ecff" />
        <rect x="36" y="52" width="8" height="8" rx="2" fill="#e7ecff" />
        <rect x="54" y="22" width="72" height="14" rx="3" fill="#fff" />
        <rect x="58" y="26" width="26" height="6" rx="2" fill="#171717" />
        <rect x="54" y="42" width="34" height="24" rx="3" fill="#fff" />
        <rect x="94" y="42" width="32" height="24" rx="4" fill="#4c6fff" />
        <rect x="92" y="40" width="36" height="28" rx="3" fill="none" stroke="#171717" strokeWidth="1.2" />
        <rect x="90.5" y="38.5" width="3.5" height="3.5" fill="#fff" stroke="#171717" strokeWidth="0.8" />
        <rect x="126" y="38.5" width="3.5" height="3.5" fill="#fff" stroke="#171717" strokeWidth="0.8" />
        <rect x="90.5" y="65.5" width="3.5" height="3.5" fill="#fff" stroke="#171717" strokeWidth="0.8" />
        <rect x="126" y="65.5" width="3.5" height="3.5" fill="#fff" stroke="#171717" strokeWidth="0.8" />
        <rect x="54" y="72" width="72" height="10" rx="3" fill="#e07848" />
      </svg>
    )
  }
  return (
    <svg
      viewBox="0 0 160 120"
      data-art="plan"
      className="absolute inset-0 h-full w-full"
      preserveAspectRatio="xMidYMid slice"
      aria-hidden>
      <rect width="160" height="120" fill="#f4efe6" />
      <rect x="16" y="18" width="86" height="92" rx="6" fill="#fff" opacity="0.72" transform="rotate(-7 59 64)" />
      <rect x="42" y="16" width="100" height="88" rx="6" fill="#fff" />
      <rect x="54" y="28" width="36" height="6" rx="2" fill="#171717" />
      <rect x="54" y="42" width="10" height="10" rx="2" fill="#e07848" />
      <rect x="70" y="45" width="56" height="4" rx="2" fill="#eadfd8" />
      <rect x="54" y="58" width="10" height="10" rx="2" fill="#4c6fff" />
      <rect x="70" y="61" width="44" height="4" rx="2" fill="#eadfd8" />
      <rect x="54" y="74" width="10" height="10" rx="2" fill="#69b9c9" />
      <rect x="70" y="77" width="50" height="4" rx="2" fill="#eadfd8" />
      <rect x="54" y="90" width="72" height="6" rx="3" fill="#f4efe6" />
      <rect x="54" y="90" width="34" height="6" rx="3" fill="#e07848" />
    </svg>
  )
}

const CREATE_TILES = [
  {
    id: 'ppt',
    label: 'workbench.feature.create.ppt',
    tone: 'dark',
    place: 'top-0 left-0 z-[1] h-[58%] w-[62%] -rotate-2',
    labelPlace: 'top-2 left-2.5'
  },
  {
    id: 'site',
    label: 'workbench.feature.create.site',
    tone: 'light',
    place: 'top-0 right-0 z-[2] h-[48%] w-[44%] rotate-1',
    labelPlace: 'top-2 right-2.5 text-right'
  },
  {
    id: 'video',
    label: 'workbench.feature.create.video',
    tone: 'dark',
    place: 'bottom-0 left-0 z-[2] h-[48%] w-[48%] rotate-1',
    labelPlace: 'bottom-2 left-2.5'
  },
  {
    id: 'image',
    label: 'workbench.feature.create.image',
    tone: 'light',
    place: 'right-0 bottom-0 z-[3] h-[58%] w-[56%] -rotate-1',
    labelPlace: 'right-2.5 bottom-2 text-right'
  }
] as const

function PdfMark() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden>
      <path
        d="M4 1.8h5.2L12.2 5v8.2a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V2.8a1 1 0 0 1 1-1Z"
        fill="none"
        stroke="#5b35d4"
        strokeWidth="1.4"
      />
      <path d="M9.1 1.9v3.2h3.1" fill="none" stroke="#5b35d4" strokeWidth="1.4" />
    </svg>
  )
}

function TagIcon({ name, color }: { name: 'medal' | 'grid' | 'square'; color: string }) {
  if (name === 'grid') {
    return (
      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden>
        <rect x="1.4" y="1.4" width="3.6" height="3.6" rx="0.6" fill={color} />
        <rect x="7" y="1.4" width="3.6" height="3.6" rx="0.6" fill={color} />
        <rect x="1.4" y="7" width="3.6" height="3.6" rx="0.6" fill={color} />
        <rect x="7" y="7" width="3.6" height="3.6" rx="0.6" fill={color} />
      </svg>
    )
  }
  if (name === 'medal') {
    return (
      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden>
        <circle cx="6" cy="5" r="2.4" fill="none" stroke={color} strokeWidth="1.2" />
        <path
          d="M4.4 7.1 3.6 10.4 6 9.1 8.4 10.4 7.6 7.1"
          fill="none"
          stroke={color}
          strokeWidth="1.2"
          strokeLinejoin="round"
        />
      </svg>
    )
  }
  return (
    <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden>
      <rect x="2" y="2" width="8" height="8" rx="1.4" fill="none" stroke={color} strokeWidth="1.2" />
    </svg>
  )
}

function OfficialDocPreview({ title, banner }: { title: string; banner: boolean }) {
  const size = title.length > 2 ? 5 : 7
  return (
    <svg viewBox="0 0 32 40" className="h-10 w-8 shrink-0" data-ui="workbench.writing.preview" aria-hidden="true">
      <path
        d="M8 3.2h12.2L25.2 8.6v25a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3V6.2a3 3 0 0 1 3-3Z"
        fill={banner ? '#FFF7F7' : '#F7F7F6'}
        stroke={banner ? '#F6DDDD' : '#E6E6E4'}
        strokeWidth="1"
        strokeLinejoin="round"
      />
      <path d="M20.1 3.5 25 8.5h-3.3a1.7 1.7 0 0 1-1.6-1.7V3.5Z" fill={banner ? '#8f1d1d' : '#C8C8C4'} />
      {banner ? <rect x="8.4" y="11.4" width="15.2" height="2" rx="0.3" fill="#8f1d1d" /> : null}
      <text
        x="16"
        y={banner ? 20.6 : 18.2}
        textAnchor="middle"
        fontSize={size}
        fontWeight="700"
        fill={banner ? '#8f1d1d' : '#171717'}
        fontFamily="Inter, -apple-system, BlinkMacSystemFont, sans-serif">
        {title}
      </text>
      <path
        d={banner ? 'M10.2 25.2h11.6M10.2 28.4h8' : 'M10.2 23.2h11.6M10.2 26.4h8M10.2 29.6h6'}
        stroke="#E6E7EE"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  )
}

function OfficialWritingPicker({ onChoose }: { onChoose: (typeId: OfficialWritingTypeId) => void }) {
  const { t } = useTranslation()
  const headingId = useId()
  const [open, setOpen] = useState(false)

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="ml-auto flex items-center gap-0.5 text-[12px] text-[#8a8a8a] hover:text-[#171717]">
          {t('workbench.feature.write.go')}
          <ChevronRight size={12} strokeWidth={1.8} aria-hidden />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        side="bottom"
        sideOffset={8}
        data-ui="workbench.writing.picker"
        className="w-[312px] rounded-[12px] border-[1px] border-[#e5e5e5] bg-white p-3 text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
        <div role="dialog" aria-labelledby={headingId}>
          <p id={headingId} className="m-0 px-1 pb-2 text-[12px] leading-4 text-[#8a8a8a]">
            {t('workbench.writing.hub.title')}
          </p>
          <div className="grid grid-cols-4">
            {OFFICIAL_WRITING_TYPES.map((type) => {
              const banner = writingChoiceOn(type.id, 'banner')
              return (
                <button
                  key={type.id}
                  type="button"
                  data-official-type={type.id}
                  data-writing-banner={banner ? 'true' : 'false'}
                  className="flex flex-col items-center gap-1.5 rounded-[8px] px-1 py-2 text-[12px] leading-4 text-[#171717] outline-none hover:bg-[#f7f7f7] focus-visible:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10"
                  onClick={() => {
                    setOpen(false)
                    onChoose(type.id)
                  }}>
                  <OfficialDocPreview title={type.title} banner={banner} />
                  <span>{type.title}</span>
                </button>
              )
            })}
          </div>
        </div>
      </PopoverContent>
    </Popover>
  )
}

function FeatureHeader({
  icon,
  title,
  action,
  onAction
}: {
  icon: ReactNode
  title: string
  action: string
  onAction: () => void
}) {
  return (
    <div className="mb-2.5 flex items-center gap-1.5 pr-0.5">
      {icon}
      <span className="text-[14px] font-medium text-[#171717]">{title}</span>
      <button
        type="button"
        onClick={(event) => {
          event.stopPropagation()
          onAction()
        }}
        className="ml-auto flex items-center gap-0.5 text-[12px] text-[#8a8a8a] hover:text-[#171717]">
        {action}
        <ChevronRight size={12} strokeWidth={1.8} />
      </button>
    </div>
  )
}

function PdfPreview({ line1, line2 }: { line1: string; line2: string }) {
  return (
    <div
      data-ui="workbench.feature-thumb"
      className="w-[86px] shrink-0 overflow-hidden rounded-[6px] border border-[#ececec] bg-white shadow-[0_2px_8px_rgba(0,0,0,0.08)]">
      <div className="h-1 bg-[#8f1d1d]" />
      <div className="px-1.5 pt-1.5 text-[8px] leading-[11px] font-semibold text-[#123a78]">
        <div>{line1}</div>
        <div>{line2}</div>
      </div>
      <div className="space-y-[3px] px-1.5 pt-1 pb-1.5">
        <div className="h-[3px] rounded bg-[#e6e7ee]" />
        <div className="h-[3px] w-4/5 rounded bg-[#e6e7ee]" />
        <div className="h-[3px] w-3/5 rounded bg-[#e6e7ee]" />
      </div>
    </div>
  )
}

type HomePdfKind = Extract<PdfToolKind, 'pdf-to-word' | 'word-to-pdf'>

export function WorkbenchFeatureStrip({ onCommitDraft }: { onCommitDraft: (handoff: OfficialWritingHandoff) => void }) {
  const { t } = useTranslation()
  const memberId = useMemberProfile()?.id ?? null
  const pins = useHomePluginPins(memberId)
  const catalogue = useHomeCatalogue()
  useEffect(() => {
    reconcileHomePluginPins(memberId, catalogue.packages)
  }, [memberId, catalogue])
  const pinned = pins.flatMap((slug) => {
    const pack = catalogue.packages.find((item) => item.slug === slug)
    return pack && packageHasHomeCards(pack) ? [pack] : []
  })
  const [writing, setWriting] = useState<{ typeId: OfficialWritingTypeId | null; seed: string } | null>(null)
  const pdfInputRef = useRef<HTMLInputElement>(null)
  const pendingKind = useRef<HomePdfKind | null>(null)
  const notOpen = () => toastNotOpen(t)
  const openPdfTools = () => {
    stagePdfToolKind(null)
    const title = t('workbench.feature.pdf.tools.title')
    void openWorkbenchAppletWindow(PDF_TOOLS_APPLET_ID, title).then((opened) => {
      if (!opened) notOpen()
    })
  }
  const openHomePdfWindow = (kind: HomePdfKind, files: File[]) => {
    const title = t('workbench.feature.pdf.tools.title')
    void stagePdfToolHandoff(kind, files).then(async (staged) => {
      if (!staged) {
        notOpen()
        return
      }
      const opened = await openWorkbenchAppletWindow(PDF_TOOLS_APPLET_ID, title, undefined, kind)
      if (!opened) {
        await dropPdfToolHandoff()
        notOpen()
      }
    })
  }
  const askPdfFile = (kind: HomePdfKind) => {
    const input = pdfInputRef.current
    if (!input) return
    const spec = intakeAccept(kind)
    pendingKind.current = kind
    input.accept = spec.accept
    input.multiple = spec.multiple
    input.value = ''
    input.click()
  }
  const shelfIds: HomeShelfId[] = pinned.map((pack) => pack.slug)
  const shelfKey = shelfIds.join('|')
  const [layout, setLayout] = useState<HomeShelfPlacement[]>(() => readHomeShelfLayout(shelfIds))
  const boardRef = useRef<HTMLDivElement>(null)
  const layoutRef = useRef(layout)
  const draggingRef = useRef(false)
  layoutRef.current = layout
  useEffect(() => {
    const board = boardRef.current
    const ids = shelfKey.length === 0 ? [] : (shelfKey.split('|') as HomeShelfId[])
    const apply = () => {
      if (draggingRef.current) return
      const width = board?.clientWidth ?? 0
      if (!hasSavedHomeShelfLayout() && width > 0 && width < 720) {
        setLayout(stackedHomeShelfLayout(ids))
        return
      }
      setLayout(hasSavedHomeShelfLayout() ? readHomeShelfLayout(ids) : defaultHomeShelfLayout(ids))
    }
    apply()
    if (!board || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(apply)
    observer.observe(board)
    return () => observer.disconnect()
  }, [shelfKey])
  const track = (id: HomeShelfId, event: ReactPointerEvent, mode: 'move' | 'resize') => {
    event.preventDefault()
    event.stopPropagation()
    const board = boardRef.current
    const origin = layout.find((item) => item.i === id)
    const startX = Number.isFinite(event.clientX) ? event.clientX : event.nativeEvent.clientX
    const startY = Number.isFinite(event.clientY) ? event.clientY : event.nativeEvent.clientY
    if (!board || !origin || !Number.isFinite(startX) || !Number.isFinite(startY)) return
    const column = board.clientWidth / HOME_SHELF_COLUMNS || 1
    if (typeof event.currentTarget.setPointerCapture === 'function' && Number.isFinite(event.pointerId)) {
      try {
        event.currentTarget.setPointerCapture(event.pointerId)
      } catch {
        // jsdom has no pointer capture. The window listeners still follow the pointer.
      }
    }
    draggingRef.current = true
    let extraRows = 0
    let lastX = startX
    let lastY = startY
    let frame = 0
    let edgeAt = 0
    let pending: HomeShelfPlacement[] | null = null
    if (board) {
      board.style.cursor = mode === 'resize' ? 'nwse-resize' : 'grabbing'
      board.dataset.dragging = 'true'
    }
    const paint = (next: readonly HomeShelfPlacement[]) => {
      const surface = boardRef.current
      if (!surface) return
      const rows = Math.max(...next.map((item) => item.y + item.h), 1)
      surface.style.height = `${rows * HOME_SHELF_ROW_PX}px`
      for (const item of next) {
        const node = surface.querySelector(`[data-shelf="${item.i}"]`)
        if (!(node instanceof HTMLElement)) continue
        node.style.transition = 'none'
        node.style.zIndex = item.i === id ? '30' : ''
        node.style.left = `${(item.x / HOME_SHELF_COLUMNS) * 100}%`
        node.style.top = `${item.y * HOME_SHELF_ROW_PX}px`
        node.style.width = `${(item.w / HOME_SHELF_COLUMNS) * 100}%`
        node.style.height = `${item.h * HOME_SHELF_ROW_PX}px`
      }
    }
    const place = (clientX: number, clientY: number) => {
      const dx = Math.round((clientX - startX) / column)
      const dy = Math.round((clientY - startY) / HOME_SHELF_ROW_PX) + extraRows
      const next =
        mode === 'move'
          ? {
              x: Math.min(HOME_SHELF_COLUMNS - origin.w, Math.max(0, origin.x + dx)),
              y: Math.max(0, origin.y + dy),
              w: origin.w,
              h: origin.h
            }
          : {
              x: origin.x,
              y: origin.y,
              w: Math.min(HOME_SHELF_COLUMNS, Math.max(3, origin.w + dx)),
              h: Math.max(6, origin.h + dy)
            }
      const placed = mode === 'resize' ? { ...next, x: Math.min(origin.x, HOME_SHELF_COLUMNS - next.w) } : next
      const current = pending ?? layoutRef.current
      const moved = current.map((item) => (item.i === id ? { ...item, ...placed } : item))
      pending = settleHomeShelfLayout(moved, id, mode === 'move' ? origin : undefined)
      paint(pending)
    }
    const flush = (now: number) => {
      frame = 0
      const scroller = boardRef.current?.closest('[data-ui="workbench.canvas"]')
      const box = scroller instanceof HTMLElement ? scroller.getBoundingClientRect() : null
      const atBottom = scroller instanceof HTMLElement && box != null && box.height >= 1 && lastY >= box.bottom - 28
      if (atBottom && now - edgeAt >= 80) {
        edgeAt = now
        scroller.scrollTop += HOME_SHELF_ROW_PX
        extraRows += 1
      }
      place(lastX, lastY)
      if (atBottom && frame === 0) frame = window.requestAnimationFrame(flush)
    }
    const move = (pointer: PointerEvent) => {
      if (!Number.isFinite(pointer.clientX) || !Number.isFinite(pointer.clientY)) return
      lastX = pointer.clientX
      lastY = pointer.clientY
      if (frame !== 0) return
      frame = window.requestAnimationFrame(flush)
    }
    const stop = () => {
      window.cancelAnimationFrame(frame)
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', stop)
      window.removeEventListener('pointercancel', stop)
      if (pending == null) place(lastX, lastY)
      const surface = boardRef.current
      if (surface) {
        surface.style.cursor = ''
        delete surface.dataset.dragging
        for (const node of surface.querySelectorAll('[data-shelf]')) {
          if (!(node instanceof HTMLElement)) continue
          node.style.transition = ''
          node.style.zIndex = ''
        }
      }
      const settled = pending ?? layoutRef.current
      pending = null
      draggingRef.current = false
      writeHomeShelfLayout(settled)
      setLayout(settled)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', stop)
    window.addEventListener('pointercancel', stop)
  }
  const openHomeCard = (card: HomeCard) => {
    if (card.action === 'writing' && card.writingType) {
      setWriting({ typeId: card.writingType, seed: card.seed ?? card.description })
      return
    }
    if (card.action === 'file' && (card.fileKind === 'pdf-to-word' || card.fileKind === 'word-to-pdf')) {
      askPdfFile(card.fileKind)
      return
    }
    if (card.action === 'applet') {
      void openWorkbenchAppletWindow(PLAN_APPLET_ID, card.title).then((opened) => {
        if (!opened) notOpen()
      })
      return
    }
    notOpen()
  }
  const unpin = (slug: string) => unpinHomePlugin(memberId, slug)
  if (pinned.length === 0) return null
  const rows = Math.max(...layout.map((item) => item.y + item.h), 1)

  return (
    <section
      ref={boardRef}
      className="relative mt-20 w-full data-[dragging]:select-none data-[dragging]:[&_*]:transition-none!"
      style={{ height: rows * HOME_SHELF_ROW_PX }}
      data-ui="workbench.features">
      {pinned.map((pack) => {
        const cards = [...(pack.home ?? [])].toSorted((left, right) => left.order - right.order)
        const face = homeFace(cards)
        return face === 'write' ? (
          <ShelfCell
            key={pack.slug}
            item={layout.find((item) => item.i === pack.slug)}
            onTrack={track}
            onRemove={() => unpin(pack.slug)}>
            <article
              data-ui="workbench.feature"
              data-feature="write"
              className="flex h-full flex-col overflow-auto rounded-[16px] border border-[#efefef] bg-white p-3 shadow-[0_4px_16px_rgba(0,0,0,0.04)]">
              <div className="mb-2.5 flex items-center gap-1.5 pr-0.5">
                <WriteMark />
                <span className="text-[14px] font-medium text-[#171717]">{t('workbench.feature.write')}</span>
                <OfficialWritingPicker onChoose={(typeId) => setWriting({ typeId, seed: '' })} />
              </div>
              <div data-ui="workbench.feature.write.grid" className="grid flex-1 grid-cols-2 gap-2">
                {cards.map((card) => {
                  const mark = card.writingType ? WRITE_MARK[writingShelf(card.writingType)] : undefined
                  return (
                    <button
                      key={card.id}
                      type="button"
                      data-home={card.id}
                      data-writing-type={card.writingType}
                      onClick={() => {
                        if (card.action !== 'writing' || !card.writingType) return
                        setWriting({ typeId: card.writingType, seed: card.seed ?? card.description })
                      }}
                      className="flex min-h-[92px] flex-col items-start justify-between rounded-xl bg-[#f4f5f7] px-2.5 py-2.5 text-left hover:bg-[#eef0f3]">
                      <span className="line-clamp-2 text-[13px] leading-5 font-medium text-[#222]">{card.title}</span>
                      <span className="mt-1 line-clamp-2 text-[12px] leading-[18px] text-[#8a8a8a]">
                        {card.description}
                      </span>
                      <span className="mt-2 flex items-center gap-1 text-[11px] text-[#8a8a8a]">
                        {mark ? <TagIcon name={mark.icon} color={mark.color} /> : null}
                        {card.tag}
                      </span>
                    </button>
                  )
                })}
              </div>
              <OfficialWritingDialog
                open={writing != null}
                typeId={writing?.typeId ?? null}
                seed={writing?.seed ?? ''}
                onOpenChange={(open) => {
                  if (!open) setWriting(null)
                }}
                onCommit={(handoff) => {
                  onCommitDraft(handoff)
                  setWriting(null)
                }}
              />
            </article>
          </ShelfCell>
        ) : face === 'create' ? (
          <ShelfCell
            key={pack.slug}
            item={layout.find((item) => item.i === pack.slug)}
            onTrack={track}
            onRemove={() => unpin(pack.slug)}>
            <article
              data-ui="workbench.feature"
              data-feature="create"
              onClick={notOpen}
              className="flex h-full cursor-pointer flex-col overflow-hidden rounded-[16px] border border-[#efefef] bg-white p-3 shadow-[0_4px_16px_rgba(0,0,0,0.04)]">
              <FeatureHeader
                icon={<CreateMark />}
                title={t('workbench.feature.create')}
                action={t('workbench.feature.create.center')}
                onAction={notOpen}
              />
              <div
                data-ui="workbench.feature.create.grid"
                data-layout="collage"
                className="relative min-h-[196px] flex-1 overflow-hidden">
                {cards.map((item) => {
                  const cover = 'cover' in item ? item.cover : item.id
                  const place = CREATE_TILES.find((tile) => tile.id === cover) ?? CREATE_TILES[0]
                  const title = 'title' in item && item.title ? item.title : t(place.label)
                  return (
                    <button
                      key={item.id}
                      type="button"
                      data-create={cover ?? item.id}
                      data-cover={cover ?? item.id}
                      onClick={(event) => {
                        event.stopPropagation()
                        const action = 'action' in item ? item.action : item.id === 'image' ? 'applet' : 'unavailable'
                        if (action !== 'applet') {
                          notOpen()
                          return
                        }
                        void openWorkbenchAppletWindow(PLAN_APPLET_ID, title).then((opened) => {
                          if (!opened) notOpen()
                        })
                      }}
                      className={`absolute overflow-hidden rounded-[14px] shadow-[0_8px_18px_rgba(23,23,23,0.12)] ring-2 ring-white transition duration-200 ease-out hover:z-10 hover:-translate-y-0.5 hover:shadow-[0_12px_24px_rgba(23,23,23,0.18)] motion-reduce:transition-none motion-reduce:hover:transform-none ${place.place}`}>
                      <CreateCover kind={place.id} />
                      <span
                        className={`absolute text-[13px] leading-5 font-medium ${place.labelPlace} ${place.tone === 'dark' ? 'text-white drop-shadow-[0_1px_2px_rgba(0,0,0,0.45)]' : 'text-[#171717]'}`}>
                        {title}
                      </span>
                    </button>
                  )
                })}
              </div>
            </article>
          </ShelfCell>
        ) : face === 'process' ? (
          <ShelfCell
            key={pack.slug}
            item={layout.find((item) => item.i === pack.slug)}
            onTrack={track}
            onRemove={() => unpin(pack.slug)}>
            <article
              data-ui="workbench.feature"
              data-feature="pdf"
              className="flex h-full flex-col overflow-visible rounded-[16px] border border-[#efefef] bg-white p-3 shadow-[0_4px_16px_rgba(0,0,0,0.04)]">
              <FeatureHeader
                icon={<PdfMark />}
                title={t('workbench.feature.pdf')}
                action={t('workbench.feature.pdf.open')}
                onAction={() => openPdfTools()}
              />
              {cards.map((card) =>
                card.fileKind && card.format ? (
                  <PdfRow
                    key={card.id}
                    format={card.format}
                    title={card.title}
                    body={card.description}
                    sample={t('workbench.feature.pdf.sample')}
                    line1={t(
                      card.fileKind === 'pdf-to-word'
                        ? 'workbench.feature.pdf.edit.preview_line1'
                        : 'workbench.feature.pdf.translate.preview_line1'
                    )}
                    line2={t(
                      card.fileKind === 'pdf-to-word'
                        ? 'workbench.feature.pdf.edit.preview_line2'
                        : 'workbench.feature.pdf.translate.preview_line2'
                    )}
                    onClick={() => askPdfFile(card.fileKind!)}
                  />
                ) : null
              )}
              <input
                ref={pdfInputRef}
                type="file"
                className="sr-only"
                data-ui="workbench.feature.pdf.input"
                onChange={(event) => {
                  const list = [...(event.target.files ?? [])]
                  event.target.value = ''
                  const kind = pendingKind.current
                  if (!kind || list.length === 0) return
                  pendingKind.current = null
                  openHomePdfWindow(kind, list)
                }}
              />
            </article>
          </ShelfCell>
        ) : (
          <ShelfCell
            key={pack.slug}
            item={layout.find((item) => item.i === pack.slug)}
            onTrack={track}
            onRemove={() => unpin(pack.slug)}>
            <article
              data-ui="workbench.feature"
              data-feature={face === 'cards' ? 'cards' : 'launch'}
              className="flex h-full flex-col overflow-auto rounded-[16px] border border-[#efefef] bg-white p-3 shadow-[0_4px_16px_rgba(0,0,0,0.04)]">
              <div className="mb-2.5 text-[14px] font-medium text-[#171717]">{pack.views[0]?.title ?? pack.slug}</div>
              {face === 'cards' ? (
                <div className="grid flex-1 grid-cols-2 gap-2">
                  {cards.map((card) => (
                    <button
                      key={card.id}
                      type="button"
                      data-home={card.id}
                      onClick={() => openHomeCard(card)}
                      className="flex min-h-[72px] flex-col items-start rounded-xl bg-[#f4f5f7] px-2.5 py-2.5 text-left hover:bg-[#eef0f3]">
                      <span className="line-clamp-2 text-[13px] leading-5 font-medium text-[#222]">{card.title}</span>
                      <span className="mt-1 line-clamp-2 text-[12px] leading-[18px] text-[#8a8a8a]">
                        {card.description}
                      </span>
                    </button>
                  ))}
                </div>
              ) : (
                <button
                  type="button"
                  onClick={notOpen}
                  className="mx-auto mt-8 flex h-[30px] items-center rounded-[8px] border border-[#e5e5e5] px-3 text-[13px] text-[#171717]">
                  {t('common.open')}
                </button>
              )}
            </article>
          </ShelfCell>
        )
      })}
    </section>
  )
}

function ShelfCell({
  item,
  onTrack,
  onRemove,
  children
}: {
  item: HomeShelfPlacement | undefined
  onTrack: (id: HomeShelfId, event: ReactPointerEvent, mode: 'move' | 'resize') => void
  onRemove: () => void
  children: ReactNode
}) {
  const { t } = useTranslation()
  if (!item) return null
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div
          className="absolute z-0 px-1.5 transition-[top,left,width,height] duration-150 ease-out motion-reduce:transition-none"
          data-shelf={item.i}
          style={{
            left: `${(item.x / HOME_SHELF_COLUMNS) * 100}%`,
            top: item.y * HOME_SHELF_ROW_PX,
            width: `${(item.w / HOME_SHELF_COLUMNS) * 100}%`,
            height: item.h * HOME_SHELF_ROW_PX
          }}>
          <div
            role="button"
            tabIndex={0}
            aria-label={t('workbench.home.layout.resize')}
            data-ui="workbench.home.layout.resize"
            className="absolute right-3 bottom-3 z-20 flex h-6 w-6 cursor-nwse-resize touch-none items-end justify-end"
            onPointerDown={(event) => onTrack(item.i, event, 'resize')}
            onClick={(event) => event.stopPropagation()}>
            <span className="mr-0.5 mb-0.5 block h-2.5 w-2.5 border-r-2 border-b-2 border-[#c4c4c4]" />
          </div>
          {children}
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent aria-label={t('workbench.home.pin.unpin')}>
        <ContextMenuItem data-ui="workbench.home.pin.remove" onSelect={() => onRemove()}>
          <ContextMenuItemContent icon={<PinOff size={16} strokeWidth={1.7} />}>
            {t('workbench.home.pin.unpin')}
          </ContextMenuItemContent>
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
}

function PdfRow({
  format,
  title,
  body,
  sample,
  line1,
  line2,
  onClick
}: {
  format: 'pdf' | 'word'
  title: string
  body: string
  sample: string
  line1: string
  line2: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="group relative z-[1] mt-2 flex flex-1 items-start gap-2 overflow-visible rounded-xl bg-[#f4f5f7] px-2.5 py-2.5 pr-[96px] text-left shadow-[0_1px_2px_rgba(0,0,0,0.04)] transition-[background-color,box-shadow] duration-300 ease-out hover:z-10 hover:bg-[#eef0f3] hover:shadow-[0_10px_24px_rgba(15,23,42,0.12)] motion-reduce:transition-none">
      <span className="absolute top-2 right-2 rounded-md bg-[#f3f1ff] px-1.5 py-0.5 text-[11px] text-[#6d5ef5]">
        {sample}
      </span>
      <DocumentFileIcon type={format} className="mt-0.5 h-9 w-8" />
      <span className="min-w-0">
        <span className="block text-[13px] leading-5 font-medium text-[#222]">{title}</span>
        <span className="mt-1 block text-[11px] leading-4 text-[#8a8a8a]">{body}</span>
      </span>
      <span className="pointer-events-none absolute inset-0 overflow-hidden rounded-xl">
        <span className="absolute right-2.5 bottom-0 translate-y-1/2 transition-transform duration-300 ease-out group-hover:-translate-y-1 motion-reduce:transition-none">
          <PdfPreview line1={line1} line2={line2} />
        </span>
      </span>
    </button>
  )
}
