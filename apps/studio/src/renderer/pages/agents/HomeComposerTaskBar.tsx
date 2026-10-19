import {
  AppWindow,
  ChevronLeft,
  ChevronRight,
  ClipboardList,
  Database,
  FileText,
  FileType,
  MonitorPlay,
  Presentation,
  Sparkles,
  type LucideIcon
} from 'lucide-react'
import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { useMemberProfile } from '@renderer/components/account/memberProfile'
import { readAixCatalogue, subscribeAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import { HOST_CHAT_MODE_ID, useHomeComposerModeState } from './homeComposerMode'
import { packageHasHomeCards, useHomePluginPins } from './homePins'
import { toastNotOpen } from './notOpenToast'
import { OFFICIAL_WRITING_EVENT, type OfficialWritingHandoff, type OfficialWritingTypeId } from './officialWriting'
import { OfficialWritingDialog } from './OfficialWritingDialog'
import { openWorkbenchAppletWindow } from './openWorkbenchAppletWindow'
import { dropPdfToolHandoff, stagePdfToolHandoff } from './pdfToolHandoff'
import { intakeAccept, type PdfToolKind } from './pdfToolsJob'
import { PDF_TOOLS_APPLET_ID, PLAN_APPLET_ID } from './WorkbenchApplets'
import { fillWorkbenchComposer } from './workbenchComposerBridge'

type HomeCard = NonNullable<AixWorkbenchCatalogue['packages'][number]['home']>[number]
type HomePdfKind = Extract<PdfToolKind, 'pdf-to-word' | 'word-to-pdf'>

function taskIcon(card: HomeCard): LucideIcon {
  if (card.shelf === 'write') return FileText
  if (card.shelf === 'process') return FileType
  if (card.cover === 'ppt') return Presentation
  if (card.cover === 'site') return AppWindow
  if (card.cover === 'video') return MonitorPlay
  if (card.cover === 'image') return ClipboardList
  return FileText
}

function useCatalogue(): AixWorkbenchCatalogue {
  return useSyncExternalStore(subscribeAixCatalogue, readAixCatalogue, readAixCatalogue)
}

const PROMPT_ICONS: Record<string, LucideIcon> = {
  database: Database
}

const TASK_CHIP =
  'inline-flex h-8 max-w-[220px] shrink-0 items-center gap-1.5 rounded-full border border-[#ececec] bg-white px-3 text-[13px] text-[#171717] transition-colors duration-150 outline-none hover:border-[#e0e0e0] hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10'

/** Which ends of the chip row hide chips; the row scrolls instead of wrapping. */
function useRowEdges(content: string) {
  const [node, setNode] = useState<HTMLDivElement | null>(null)
  const [edges, setEdges] = useState({ start: false, end: false })
  useEffect(() => {
    if (!node) return
    const update = () => {
      const start = node.scrollLeft > 1
      const end = node.scrollLeft + node.clientWidth < node.scrollWidth - 1
      setEdges((current) => (current.start === start && current.end === end ? current : { start, end }))
    }
    update()
    node.addEventListener('scroll', update, { passive: true })
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(update)
    observer?.observe(node)
    for (const child of node.children) observer?.observe(child)
    return () => {
      node.removeEventListener('scroll', update)
      observer?.disconnect()
    }
  }, [node, content])
  return { node, edges, ref: setNode }
}

type TaskEntry = { key: string; id: string; title: string; Glyph: LucideIcon; open: () => void }

export function HomeComposerTaskBar({
  onCommitDraft,
  onFillPrompt
}: {
  onCommitDraft?: (handoff: OfficialWritingHandoff) => void
  onFillPrompt?: (text: string) => void
}) {
  const { t } = useTranslation()
  const memberId = useMemberProfile()?.id ?? null
  const pins = useHomePluginPins(memberId)
  const catalogue = useCatalogue()
  const { selected, selectedId } = useHomeComposerModeState()
  const office = selectedId === HOST_CHAT_MODE_ID
  const tasks = office
    ? pins.flatMap((slug) => {
        const pack = catalogue.packages.find((item) => item.slug === slug)
        if (!packageHasHomeCards(pack)) return []
        return [...(pack?.home ?? [])].sort((left, right) => left.order - right.order).map((card) => ({ slug, card }))
      })
    : []
  const prompts = office ? [] : [...(selected?.tasks ?? [])]
  const row = useRowEdges(`${selectedId}:${tasks.length}:${prompts.length}`)
  const [writing, setWriting] = useState<{ typeId: OfficialWritingTypeId; seed: string } | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)
  const pendingKind = useRef<HomePdfKind | null>(null)
  if (tasks.length === 0 && prompts.length === 0) return null
  const fillPrompt = (prompt: string) => {
    if (onFillPrompt) {
      onFillPrompt(prompt)
      return
    }
    fillWorkbenchComposer(prompt)
  }
  const notOpen = () => toastNotOpen(t)
  const commit = (handoff: OfficialWritingHandoff) => {
    if (onCommitDraft) {
      onCommitDraft(handoff)
      return
    }
    fillWorkbenchComposer(handoff.text)
    window.dispatchEvent(new CustomEvent(OFFICIAL_WRITING_EVENT, { detail: handoff }))
  }
  const askPdfFile = (kind: HomePdfKind) => {
    const input = fileRef.current
    if (!input) return
    const spec = intakeAccept(kind)
    pendingKind.current = kind
    input.accept = spec.accept
    input.multiple = spec.multiple
    input.value = ''
    input.click()
  }
  const openPdf = (kind: HomePdfKind, files: File[]) => {
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
  const openTask = (card: HomeCard) => {
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
  const PromptGlyph = PROMPT_ICONS[selected?.icon ?? ''] ?? Sparkles
  const entries: TaskEntry[] = [
    ...tasks.map(({ slug, card }) => ({
      key: `${slug}:${card.id}`,
      id: card.id,
      title: card.title,
      Glyph: taskIcon(card),
      open: () => openTask(card)
    })),
    ...prompts.map((task) => ({
      key: `${selectedId}:${task.id}`,
      id: task.id,
      title: task.title,
      Glyph: PromptGlyph,
      open: () => fillPrompt(task.prompt)
    }))
  ]
  const scrollRow = () => {
    const node = row.node
    if (!node) return
    const step = Math.max(160, Math.round(node.clientWidth * 0.6))
    node.scrollBy({ left: row.edges.end ? step : -node.scrollLeft, behavior: 'smooth' })
  }

  return (
    <div
      data-ui="workbench.composer.tasks"
      role="group"
      aria-label={t('workbench.composer.tasks.label')}
      className="mb-3 flex items-center gap-2">
      <div className="relative min-w-0 flex-1">
        <div
          ref={row.ref}
          data-ui="workbench.composer.tasks.row"
          className="flex [scrollbar-width:none] flex-nowrap items-center gap-2 overflow-x-auto [&::-webkit-scrollbar]:hidden">
          {entries.map(({ key, id, title, Glyph, open }) => (
            <button
              key={key}
              type="button"
              data-ui="workbench.composer.task"
              data-task={id}
              title={title}
              onClick={open}
              className={TASK_CHIP}>
              <Glyph aria-hidden size={16} strokeWidth={1.7} className="shrink-0 text-[#3f3f3f]" />
              <span className="truncate">{title}</span>
            </button>
          ))}
        </div>
        {row.edges.start ? (
          <span
            aria-hidden
            className="pointer-events-none absolute inset-y-0 left-0 w-8 bg-gradient-to-r from-white to-transparent"
          />
        ) : null}
        {row.edges.end ? (
          <span
            aria-hidden
            className="pointer-events-none absolute inset-y-0 right-0 w-12 bg-gradient-to-l from-white to-transparent"
          />
        ) : null}
      </div>
      {row.edges.start || row.edges.end ? (
        <button
          type="button"
          data-ui="workbench.composer.tasks.more"
          aria-label={t(row.edges.end ? 'workbench.composer.tasks.next' : 'workbench.composer.tasks.first')}
          onClick={scrollRow}
          className="grid size-6 shrink-0 place-items-center rounded-full bg-[#f0f0f0] text-[#6f6f6f] transition-colors duration-150 outline-none hover:bg-[#e5e5e5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10">
          {row.edges.end ? (
            <ChevronRight aria-hidden size={14} strokeWidth={2} />
          ) : (
            <ChevronLeft aria-hidden size={14} strokeWidth={2} />
          )}
        </button>
      ) : null}
      <input
        ref={fileRef}
        type="file"
        className="sr-only"
        data-ui="workbench.composer.task.file"
        onChange={(event) => {
          const list = [...(event.target.files ?? [])]
          event.target.value = ''
          const kind = pendingKind.current
          if (!kind || list.length === 0) return
          pendingKind.current = null
          openPdf(kind, list)
        }}
      />
      {writing ? (
        <OfficialWritingDialog
          open
          typeId={writing.typeId}
          seed={writing.seed}
          onOpenChange={(open) => {
            if (!open) setWriting(null)
          }}
          onCommit={(handoff) => {
            commit(handoff)
            setWriting(null)
          }}
        />
      ) : null}
    </div>
  )
}
