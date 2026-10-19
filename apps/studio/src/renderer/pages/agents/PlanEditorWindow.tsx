import {
  ConfirmDialog,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  SearchSelect,
  Tooltip
} from '@a3s-lab/ui'
import {
  AlertTriangle,
  BarChart3,
  BookOpen,
  Briefcase,
  CalendarDays,
  Check,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  ClipboardCheck,
  Code,
  Columns2,
  Copy,
  Cpu,
  Download,
  Expand,
  File,
  FileCode,
  FilePlus,
  FileText,
  FileType,
  FileUp,
  Flag,
  FolderOpen,
  GraduationCap,
  Image as ImageIcon,
  ImagePlus,
  LayoutGrid,
  Library,
  Link,
  List,
  ListChecks,
  ListOrdered,
  ListTree,
  Minus,
  NotebookPen,
  Package,
  Palette,
  PanelTop,
  Pencil,
  PenTool,
  Pilcrow,
  Printer,
  Quote,
  Route,
  Save,
  Search,
  SeparatorHorizontal,
  Settings2,
  ShieldCheck,
  Sparkles,
  SwatchBook,
  Table,
  Trash2,
  Type as TypeIcon,
  X,
  type LucideIcon
} from 'lucide-react'
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useCallback,
  type ChangeEvent,
  type ReactNode,
  type RefObject
} from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { toast } from '@renderer/services/toast'

import { AppletWindowBar } from './AppletWindowBar'
import { planLineDiff, requestPlanRewrite } from './planAssist'
import { planDocumentDocx } from './planDocx'
import { planExportName, printPlanPages, savePlanFile } from './planExport'
import {
  acceptPlanImage,
  acceptPlanLogo,
  clipPlanMarkdown,
  createPlanDraft,
  insertPlanImage,
  insertPlanMarkup,
  loadPlanDrafts,
  planAccent,
  planAssetUrl,
  planBumpHeading,
  planCaptionLabel,
  planCover,
  planDocumentHtml,
  planEditedPhrase,
  planFlowPages,
  planHeadingNumber,
  planHeadings,
  planImageSlot,
  planListMark,
  planTocChunks,
  planTocEntries,
  removePlanImage,
  planStats,
  planTemplate,
  planTemplatesIn,
  resolvePlanImage,
  PLAN_CATEGORIES,
  PLAN_COVERS,
  savePlanDrafts,
  type PlanBlock,
  type PlanBodySize,
  type PlanCategory,
  type PlanCoverId,
  type PlanDraft,
  type PlanHeadingCounter,
  type PlanLayout,
  type PlanMarkup,
  type PlanMeta,
  type PlanPanel,
  type PlanStyle,
  type PlanTemplate,
  type PlanView
} from './planModel'
import { PlanSource, type PlanSourceHandle } from './planSource'

const FOCUS = 'focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-black/10'
const FIELD = `h-9 w-full min-w-0 rounded-lg border-0 bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717] placeholder:text-[#8a8a8a] ${FOCUS}`
const COMPACT = `h-9 w-full min-w-0 rounded-lg border-0 bg-[#f5f5f5] px-1.5 text-[13px] text-[#171717] ${FOCUS}`
const QUIET = `inline-flex h-7 items-center gap-1.5 rounded-lg border border-[#e5e5e5] bg-white px-2.5 text-[13px] text-[#171717] hover:bg-[#fafafa] ${FOCUS}`
const GHOST = `inline-flex h-7 items-center gap-1.5 rounded-lg px-2.5 text-[13px] text-[#3f3f3f] hover:bg-[#f7f7f7] ${FOCUS}`
const ICON_BUTTON = `inline-flex size-6 shrink-0 items-center justify-center rounded-md text-[#3f3f3f] hover:bg-[#f7f7f7] ${FOCUS}`
const FILL = `inline-flex h-9 items-center justify-center rounded-lg bg-[#171717] px-3 text-[13px] font-medium text-white disabled:opacity-40 ${FOCUS}`
const FILL_SM = `inline-flex h-7 items-center justify-center gap-1.5 rounded-lg bg-[#171717] px-2.5 text-[13px] font-medium text-white ${FOCUS}`
const MARK_BUTTON = `inline-flex h-6 min-w-6 shrink-0 items-center justify-center rounded-md px-1 text-[12px] font-medium text-[#3f3f3f] hover:bg-[#f7f7f7] ${FOCUS}`
const SHEET_WIDTH = 794
const SHEET_HEIGHT = 1123
const PAPER =
  'relative flex h-full w-full flex-col overflow-hidden border border-[#e5e5e5] bg-white text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]'

const VIEW_ICON: Record<PlanView, LucideIcon> = {
  split: Columns2,
  edit: Pencil,
  preview: Expand
}

/** A cover choice in the theme library also sets the body accent, type, and rules. */
const THEME_LOOK: Record<PlanCoverId, Partial<PlanStyle>> = {
  classic: { ink: 'pine', font: 'serif', h1Form: 'underline', h2Form: 'line', headerRule: 'accent' },
  tech: { ink: 'ink', font: 'sans', h1Form: 'accent', h2Form: 'bar', headerRule: 'solid' },
  formal: { ink: 'wine', font: 'serif', h1Form: 'underline', h2Form: 'plain', headerRule: 'double' },
  mono: { ink: 'ink', font: 'sans', h1Form: 'minimal', h2Form: 'plain', headerRule: 'none' },
  gradient: { ink: 'violet', font: 'sans', h1Form: 'badge', h2Form: 'bar', headerRule: 'accent' },
  academic: { ink: 'ink', font: 'serif', h1Form: 'underline', h2Form: 'line', headerRule: 'solid' },
  approval: { ink: 'ink', font: 'hei', h1Form: 'underline', h2Form: 'plain', headerRule: 'double' },
  brief: { ink: 'pine', font: 'sans', h1Form: 'accent', h2Form: 'bar', headerRule: 'accent' }
}

const PANEL_ICON: Record<PlanPanel, LucideIcon> = {
  cover: PanelTop,
  header: Pilcrow,
  toc: ListTree,
  color: Palette,
  style: TypeIcon,
  other: Settings2
}

const TEMPLATE_ICON: Record<string, LucideIcon> = {
  blank: File,
  guide: BookOpen,
  delivery: Package,
  whitepaper: PenTool,
  architecture: Route,
  approval: ClipboardCheck,
  research: Library,
  tokens: SwatchBook,
  proposal: Flag,
  thesis: GraduationCap,
  minutes: NotebookPen,
  prd: ListChecks,
  incident: AlertTriangle,
  sop: ShieldCheck,
  weekly: CalendarDays,
  metrics: BarChart3
}

const CATEGORY_ICON: Record<PlanCategory, LucideIcon> = {
  all: LayoutGrid,
  blank: File,
  guide: BookOpen,
  development: Cpu,
  design: PenTool,
  delivery: Package,
  business: Briefcase,
  meeting: CalendarDays,
  academic: GraduationCap
}

const CATEGORY_MARK: Record<PlanCategory, { fill: string; glyph: string }> = {
  all: { fill: '#171717', glyph: '#ffffff' },
  blank: { fill: '#d4d4d4', glyph: '#171717' },
  guide: { fill: '#2f6b4f', glyph: '#ffffff' },
  development: { fill: '#3b6ea5', glyph: '#ffffff' },
  design: { fill: '#6b4c9a', glyph: '#ffffff' },
  delivery: { fill: '#1f6f78', glyph: '#ffffff' },
  business: { fill: '#b26a2a', glyph: '#ffffff' },
  meeting: { fill: '#c4554a', glyph: '#ffffff' },
  academic: { fill: '#a6843d', glyph: '#ffffff' }
}

type PlanPatch = Omit<Partial<PlanDraft>, 'meta' | 'style'> & {
  meta?: Partial<PlanMeta>
  style?: Partial<PlanStyle>
}

const PANELS: readonly PlanPanel[] = ['cover', 'header', 'toc', 'color', 'style', 'other']
const VIEWS: readonly PlanView[] = ['split', 'edit', 'preview']
const SIZES: readonly PlanBodySize[] = ['sm', 'md', 'lg']

type Screen = { kind: 'gallery' } | { kind: 'editor'; id: string }
type Confirm = { kind: 'clear' } | { kind: 'delete'; id: string } | null

/**
 * Plan editor inside a native applet window. The gallery starts a draft.
 * The editor keeps Markdown as the source and projects the cover from the
 * same fields.
 */
export function PlanEditorWindow() {
  const { t } = useTranslation()
  const [drafts, setDrafts] = useState<PlanDraft[]>(() => loadPlanDrafts(localStorage))
  const [screen, setScreen] = useState<Screen>({ kind: 'gallery' })
  const [past, setPast] = useState<Screen[]>([])
  const [future, setFuture] = useState<Screen[]>([])
  const [category, setCategory] = useState<PlanCategory>('all')
  const [view, setView] = useState<PlanView>('split')
  const [panel, setPanel] = useState<PlanPanel>('cover')
  const [fileOpen, setFileOpen] = useState(false)
  const [confirm, setConfirm] = useState<Confirm>(null)
  const [saveFailed, setSaveFailed] = useState(false)
  const screenRef = useRef(screen)
  const pastRef = useRef<Screen[]>([])
  const futureRef = useRef<Screen[]>([])
  const draftsRef = useRef(drafts)
  screenRef.current = screen
  draftsRef.current = drafts
  const draft = screen.kind === 'editor' ? drafts.find((item) => item.id === screen.id) : undefined
  const title = draft?.meta.title.trim() || t('workbench.feature.create.image')

  const remember = (next: PlanDraft[]) => {
    draftsRef.current = next
    setDrafts(next)
    try {
      savePlanDrafts(localStorage, next)
      setSaveFailed(false)
      return true
    } catch {
      setSaveFailed(true)
      return false
    }
  }

  const show = (next: Screen) => {
    pastRef.current = [...pastRef.current, screenRef.current]
    futureRef.current = []
    setPast(pastRef.current)
    setFuture([])
    screenRef.current = next
    setScreen(next)
    setFileOpen(false)
  }

  const openTemplate = (template: PlanTemplate) => {
    const created = createPlanDraft({
      template,
      title: template.id === 'blank' ? t('workbench.plan.untitled') : t(template.titleKey),
      subtitle: t(template.subtitleKey),
      organization: t('workbench.plan.defaults.organization'),
      department: t('workbench.plan.defaults.department'),
      now: Date.now()
    })
    remember([created, ...draftsRef.current.filter((item) => item.id !== created.id)])
    show({ kind: 'editor', id: created.id })
  }

  const patch = (id: string, change: PlanPatch) => {
    remember(
      draftsRef.current.map((item) => {
        if (item.id !== id) return item
        return {
          ...item,
          ...change,
          meta: { ...item.meta, ...change.meta },
          style: { ...item.style, ...change.style },
          updatedAt: Date.now()
        }
      })
    )
  }

  const back = () => {
    const previous = pastRef.current[pastRef.current.length - 1]
    if (!previous) return
    futureRef.current = [screenRef.current, ...futureRef.current]
    pastRef.current = pastRef.current.slice(0, -1)
    setFuture(futureRef.current)
    setPast(pastRef.current)
    screenRef.current = previous
    setScreen(previous)
  }

  const forward = () => {
    const next = futureRef.current[0]
    if (!next) return
    pastRef.current = [...pastRef.current, screenRef.current]
    futureRef.current = futureRef.current.slice(1)
    setPast(pastRef.current)
    setFuture(futureRef.current)
    screenRef.current = next
    setScreen(next)
  }

  const home = () => {
    if (screenRef.current.kind !== 'gallery') show({ kind: 'gallery' })
  }

  const refresh = () => {
    const next = loadPlanDrafts(localStorage)
    remember(next)
    const current = screenRef.current
    if (current.kind === 'editor' && !next.some((item) => item.id === current.id)) show({ kind: 'gallery' })
  }

  return (
    <section
      aria-label={title}
      data-ui="workbench.applet.window"
      data-applet="plan"
      data-presentation="window"
      className="fixed inset-0 flex flex-col overflow-hidden bg-white text-[#171717]">
      <AppletWindowBar
        title={title}
        icon={<FileText className="size-4 text-[#3f3f3f]" strokeWidth={1.7} aria-hidden />}
        canBack={past.length > 0}
        canForward={future.length > 0}
        onBack={back}
        onForward={forward}
        onHome={home}
        onRefresh={refresh}
      />
      {draft ? (
        <PlanEditor
          draft={draft}
          view={view}
          panel={panel}
          fileOpen={fileOpen}
          saveFailed={saveFailed}
          onView={setView}
          onPanel={setPanel}
          onFileOpen={setFileOpen}
          onPatch={(change) => patch(draft.id, change)}
          onNew={() => show({ kind: 'gallery' })}
          onSave={() => remember(draftsRef.current)}
        />
      ) : (
        <PlanGallery
          drafts={drafts}
          category={category}
          onCategory={setCategory}
          onTemplate={openTemplate}
          onContinue={(id) => show({ kind: 'editor', id })}
          onAskClear={() => setConfirm({ kind: 'clear' })}
          onAskDelete={(id) => setConfirm({ kind: 'delete', id })}
        />
      )}
      <ConfirmDialog
        open={confirm != null}
        onOpenChange={(open) => {
          if (!open) setConfirm(null)
        }}
        destructive
        title={t(
          confirm?.kind === 'delete' ? 'workbench.plan.drafts.delete_title' : 'workbench.plan.drafts.clear_title'
        )}
        description={t(
          confirm?.kind === 'delete' ? 'workbench.plan.drafts.delete_body' : 'workbench.plan.drafts.clear_body'
        )}
        confirmText={t(confirm?.kind === 'delete' ? 'workbench.plan.drafts.delete' : 'workbench.plan.drafts.clear')}
        cancelText={t('common.cancel')}
        onConfirm={() => {
          if (confirm?.kind === 'delete') {
            remember(draftsRef.current.filter((item) => item.id !== confirm.id))
            if (screenRef.current.kind === 'editor' && screenRef.current.id === confirm.id) show({ kind: 'gallery' })
          } else if (confirm?.kind === 'clear') {
            remember([])
            if (screenRef.current.kind === 'editor') show({ kind: 'gallery' })
          }
        }}
      />
    </section>
  )
}

function PlanGallery({
  drafts,
  category,
  onCategory,
  onTemplate,
  onContinue,
  onAskClear,
  onAskDelete
}: {
  drafts: readonly PlanDraft[]
  category: PlanCategory
  onCategory: (category: PlanCategory) => void
  onTemplate: (template: PlanTemplate) => void
  onContinue: (id: string) => void
  onAskClear: () => void
  onAskDelete: (id: string) => void
}) {
  const { t } = useTranslation()
  const [query, setQuery] = useState('')
  const [plansOpen, setPlansOpen] = useState(false)
  const now = Date.now()
  const needle = query.trim().toLowerCase()
  const matches = (template: PlanTemplate) => {
    if (!needle) return true
    return [template.titleKey, template.subtitleKey, template.badgeKey, template.englishKey]
      .map((key) => t(key).toLowerCase())
      .join('\n')
      .includes(needle)
  }
  const templates = planTemplatesIn(category).filter(matches)
  const pinned = category === 'all' && !needle ? templates.find((item) => item.id === 'guide') : undefined
  const cards = pinned ? [pinned, ...templates.filter((item) => item.id !== pinned.id)] : templates
  const heading = t(category === 'all' ? 'workbench.plan.category.all' : `workbench.plan.category.${category}`)
  return (
    <div data-ui="workbench.plan" className="flex min-h-0 flex-1 bg-[#f7f7f7]">
      <aside className="flex w-[220px] shrink-0 flex-col border-r border-[#efefef] bg-white">
        <div className="px-3 pt-4 pb-3">
          <h2 className="m-0 text-[14px] font-medium text-[#171717]">{t('workbench.plan.gallery.kicker')}</h2>
          <p className="m-0 mt-1 text-[12px] leading-4 text-[#8a8a8a]">{t('workbench.plan.gallery.hint')}</p>
          <label className="relative mt-3 block">
            <span className="sr-only">{t('workbench.plan.gallery.search')}</span>
            <Search
              className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-[#8a8a8a]"
              strokeWidth={1.7}
              aria-hidden
            />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t('workbench.plan.gallery.search')}
              className={`h-[30px] w-full rounded-lg border-0 bg-[#f5f5f5] pr-2.5 pl-8 text-[13px] text-[#171717] placeholder:text-[#8a8a8a] ${FOCUS}`}
            />
          </label>
        </div>
        <div
          role="radiogroup"
          aria-label={t('workbench.plan.gallery.kicker')}
          className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-2 pb-3">
          {PLAN_CATEGORIES.map((item) => {
            const CategoryIcon = CATEGORY_ICON[item]
            const mark = CATEGORY_MARK[item]
            const count = planTemplatesIn(item).filter(matches).length
            const selected = category === item
            return (
              <button
                key={item}
                type="button"
                role="radio"
                aria-checked={selected}
                data-ui="workbench.plan.category"
                data-category={item}
                onClick={() => onCategory(item)}
                className={`flex h-9 shrink-0 items-center gap-2 rounded-lg px-2 text-left text-[13px] ${FOCUS} ${selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7]'}`}>
                <span
                  className="inline-flex size-[22px] shrink-0 items-center justify-center rounded-md"
                  style={{ background: mark.fill, color: mark.glyph }}>
                  <CategoryIcon className="size-3.5" strokeWidth={1.7} aria-hidden />
                </span>
                <span className="min-w-0 flex-1 truncate">{t(`workbench.plan.category.${item}`)}</span>
                <span aria-hidden className="text-[12px] font-normal text-[#8a8a8a]">
                  {count}
                </span>
              </button>
            )
          })}
        </div>
      </aside>
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <div className="flex items-center justify-between gap-3 px-6 pt-4 pb-3">
          <h3 className="m-0 text-[14px] font-medium text-[#171717]">
            {heading}
            <span aria-hidden className="ml-2 text-[12px] font-normal text-[#8a8a8a]">
              {cards.length}
            </span>
          </h3>
          <button type="button" className={QUIET} onClick={() => setPlansOpen(true)}>
            <FolderOpen className="size-3.5" strokeWidth={1.7} aria-hidden />
            {t('workbench.plan.gallery.mine')}
            {drafts.length > 0 ? (
              <span aria-hidden className="text-[12px] text-[#8a8a8a]">
                {drafts.length}
              </span>
            ) : null}
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
          <div data-ui="workbench.plan.gallery">
            {cards.length === 0 ? (
              <p data-ui="workbench.plan.gallery.empty" className="m-0 py-6 text-[13px] text-[#8a8a8a]">
                {t('workbench.plan.gallery.empty')}
              </p>
            ) : (
              <ul className="m-0 grid list-none grid-cols-[repeat(auto-fill,minmax(196px,1fr))] gap-3 p-0">
                {cards.map((template) => (
                  <TemplateCoverCard
                    key={template.id}
                    template={template}
                    recommended={pinned?.id === template.id}
                    onTemplate={onTemplate}
                  />
                ))}
              </ul>
            )}
          </div>
        </div>
      </div>
      <Dialog open={plansOpen} onOpenChange={setPlansOpen}>
        <DialogContent
          size="xl"
          closeLabel={t('common.close')}
          className="flex max-h-[min(720px,calc(100%-4rem))] flex-col gap-0 overflow-hidden p-0 sm:max-w-[760px]">
          <DialogHeader className="gap-1 border-b border-[#efefef] px-5 py-4 pr-12 text-left">
            <DialogTitle className="text-[16px] font-medium text-[#171717]">
              {t('workbench.plan.gallery.mine')}
            </DialogTitle>
            <DialogDescription className="text-[12px] text-[#8a8a8a]">
              {t('workbench.plan.drafts.count', { count: drafts.length })}
            </DialogDescription>
          </DialogHeader>
          <div className="flex items-center justify-end px-5 py-2">
            <button type="button" className={QUIET} disabled={drafts.length === 0} onClick={onAskClear}>
              {t('workbench.plan.drafts.clear')}
            </button>
          </div>
          {drafts.length === 0 ? (
            <p data-ui="workbench.plan.drafts.empty" className="m-0 px-5 py-10 text-[13px] text-[#8a8a8a]">
              {t('workbench.plan.drafts.empty')}
            </p>
          ) : (
            <ul className="m-0 min-h-0 flex-1 list-none overflow-y-auto p-0">
              {drafts.map((item) => {
                const template = planTemplate(item.templateId)
                const stats = planStats(item.markdown)
                const phrase = planEditedPhrase(item.updatedAt, now)
                const DraftIcon = TEMPLATE_ICON[item.templateId] ?? FileText
                const cover = planCover(template?.coverId ?? 'mono')
                return (
                  <li
                    key={item.id}
                    data-ui="workbench.plan.draft"
                    data-draft={item.id}
                    className="flex items-center gap-3 border-t border-[#efefef] px-5 py-2.5 first:border-t-0 hover:bg-[#f7f7f7]">
                    <span
                      className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg border border-[#efefef]"
                      style={{ background: cover.paper, color: cover.ink }}>
                      <DraftIcon className="size-3.5" strokeWidth={1.7} aria-hidden />
                    </span>
                    <div className="min-w-0 flex-1">
                      <p className="m-0 truncate text-[13px] font-medium text-[#171717]">
                        {item.meta.title || t('workbench.plan.untitled')}
                      </p>
                      <p className="m-0 truncate text-[12px] text-[#8a8a8a]">
                        {t(phrase.key, { count: phrase.count })}
                        {' · '}
                        {template ? t(template.badgeKey) : t(`workbench.plan.category.${item.category}`)}
                        {' · '}
                        {t('workbench.plan.draft.stats', stats)}
                      </p>
                    </div>
                    <button
                      type="button"
                      className={GHOST}
                      onClick={() => {
                        setPlansOpen(false)
                        onContinue(item.id)
                      }}>
                      {t('workbench.plan.drafts.continue')}
                    </button>
                    <button
                      type="button"
                      aria-label={t('workbench.plan.drafts.delete')}
                      onClick={() => onAskDelete(item.id)}
                      className={`inline-flex size-6 shrink-0 items-center justify-center rounded-md text-[#3f3f3f] hover:bg-[#f2f3f5] ${FOCUS}`}>
                      <Trash2 className="size-3.5" strokeWidth={1.7} aria-hidden />
                    </button>
                  </li>
                )
              })}
            </ul>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}

function TemplateCoverCard({
  template,
  recommended,
  onTemplate
}: {
  template: PlanTemplate
  recommended: boolean
  onTemplate: (template: PlanTemplate) => void
}) {
  const { t } = useTranslation()
  const cover = planCover(template.coverId)
  const formal = template.coverId === 'formal'
  const title = t(template.titleKey)
  return (
    <li>
      <button
        type="button"
        data-ui="workbench.plan.template"
        data-template={template.id}
        aria-label={title}
        onClick={() => onTemplate(template)}
        className={`group flex h-full w-full flex-col overflow-hidden rounded-xl border border-[#efefef] bg-white text-left hover:border-[#e5e5e5] ${FOCUS}`}>
        <span
          className="relative block h-[168px] overflow-hidden"
          style={{ background: cover.paper, color: cover.ink }}>
          {formal ? <span className="absolute inset-x-0 top-0 h-1.5" style={{ background: cover.accent }} /> : null}
          {template.coverId === 'brief' ? (
            <span className="absolute inset-y-0 left-0 w-1.5" style={{ background: cover.accent }} />
          ) : null}
          <span className={`flex h-full flex-col px-4 pt-5 pb-4 ${template.coverId === 'brief' ? 'pl-5' : ''}`}>
            <span className="text-[11px] tracking-wide" style={{ color: cover.mute }}>
              {[template.number, template.version].filter(Boolean).join('  ·  ') || t(template.badgeKey)}
            </span>
            <span className="mt-4 line-clamp-3 text-[16px] leading-6 font-medium">{title}</span>
            <span className="mt-auto h-0.5 w-8" style={{ background: cover.accent }} />
          </span>
          {recommended ? (
            <span className="absolute top-3 right-3 rounded-md bg-white/80 px-1.5 py-0.5 text-[11px] text-[#171717]">
              {t('workbench.plan.gallery.recommended')}
            </span>
          ) : null}
        </span>
        <span className="flex flex-col gap-1 px-3 py-2.5">
          <span className="truncate text-[13px] font-medium text-[#171717]">{title}</span>
          <span className="line-clamp-2 text-[12px] leading-4 text-[#8a8a8a]">{t(template.subtitleKey)}</span>
        </span>
      </button>
    </li>
  )
}

function PlanEditor({
  draft,
  view,
  panel,
  fileOpen,
  saveFailed,
  onView,
  onPanel,
  onFileOpen,
  onPatch,
  onNew,
  onSave
}: {
  draft: PlanDraft
  view: PlanView
  panel: PlanPanel
  fileOpen: boolean
  saveFailed: boolean
  onView: (view: PlanView) => void
  onPanel: (panel: PlanPanel) => void
  onFileOpen: (open: boolean) => void
  onPatch: (change: PlanPatch) => void
  onNew: () => void
  onSave: () => boolean
}) {
  const { t } = useTranslation()
  const sourceRef = useRef<PlanSourceHandle>(null)
  const previewRef = useRef<HTMLDivElement>(null)
  const scrollEcho = useRef<null | 'source' | 'preview'>(null)
  const overflowMap = useRef(new Map<number, boolean>())
  const overflowStampRef = useRef('')
  const logoRef = useRef<HTMLInputElement>(null)
  const imageRef = useRef<HTMLInputElement>(null)
  const markdownRef = useRef<HTMLInputElement>(null)
  const fileRef = useRef<HTMLDivElement>(null)
  const fileMenuRef = useRef<HTMLDivElement>(null)
  const exportRef = useRef<HTMLDivElement>(null)
  const exportMenuRef = useRef<HTMLDivElement>(null)
  const savedTimer = useRef<number | null>(null)
  const [savedFlash, setSavedFlash] = useState(false)
  const abortRef = useRef<AbortController | null>(null)
  const [tool, setTool] = useState<null | 'ai' | 'themes' | 'images'>(null)
  const [exportOpen, setExportOpen] = useState(false)
  const [syncOn, setSyncOn] = useState(true)
  const [selectedCount, setSelectedCount] = useState(0)
  const [fontSize, setFontSize] = useState<12 | 13 | 15>(13)
  const [outlineOpen, setOutlineOpen] = useState(false)
  const [zoom, setZoom] = useState(0.62)
  const [pageCursor, setPageCursor] = useState(1)
  const [overflowPages, setOverflowPages] = useState<number[]>([])
  const stageRef = useRef<HTMLDivElement>(null)
  const [aiInstruction, setAiInstruction] = useState('')
  const [aiResult, setAiResult] = useState('')
  const [aiRunning, setAiRunning] = useState(false)
  const [aiError, setAiError] = useState('')
  const overflowStamp = `${draft.bodySize}:${draft.style.pagination}:${draft.style.h1Break}:${draft.style.tocOn}:${draft.markdown}`
  if (overflowStampRef.current !== overflowStamp) {
    overflowStampRef.current = overflowStamp
    overflowMap.current = new Map()
    if (overflowPages.length > 0) setOverflowPages([])
  }
  const stats = planStats(draft.markdown)
  const bodyPages = planFlowPages(draft.markdown, draft.style.pagination, draft.style.tableCaption, draft.style.h1Break)
  const headingStarts: number[] = []
  let headingCount = 0
  for (const blocks of bodyPages) {
    headingStarts.push(headingCount)
    headingCount += blocks.filter((block) => block.kind === 'h').length
  }
  const showSource = view !== 'preview'
  const showPreview = view !== 'edit'
  const showInspector = view !== 'preview'
  const coverOn = draft.showCover
  const bodySheets: PlanBlock[][] = bodyPages.length > 0 ? bodyPages : coverOn ? [] : [[]]
  let imageCursor = 0
  let tableCursor = 0
  const sheetStarts = bodySheets.map((blocks) => {
    const start = { image: imageCursor, table: tableCursor }
    for (const block of blocks) {
      if (block.kind === 'image') imageCursor += 1
      if (block.kind === 'table') tableCursor += 1
    }
    return start
  })
  let headingCursor: PlanHeadingCounter = [0, 0, 0, 0]
  const headingCursors = bodySheets.map((blocks) => {
    const start: PlanHeadingCounter = [headingCursor[0], headingCursor[1], headingCursor[2], headingCursor[3]]
    for (const block of blocks) {
      if (block.kind === 'h') headingCursor = planBumpHeading(headingCursor, block.level)
    }
    return start
  })
  const tocEntries = draft.style.tocOn ? planTocEntries(bodySheets, draft.style.tocDepth) : []
  const tocSheets = draft.style.tocOn ? planTocChunks(tocEntries) : []
  const coverCount = coverOn ? 1 : 0
  const sheetCount = coverCount + tocSheets.length + bodySheets.length
  const pageOffset = coverCount + tocSheets.length
  const shownPage = Math.min(pageCursor, Math.max(1, sheetCount))
  const outlineCount = planHeadings(draft.markdown).length

  useEffect(() => {
    if (!fileOpen) return
    const close = (event: MouseEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      if (fileRef.current?.contains(target) || fileMenuRef.current?.contains(target)) return
      onFileOpen(false)
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onFileOpen(false)
    }
    document.addEventListener('mousedown', close)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', close)
      document.removeEventListener('keydown', onKey)
    }
  }, [fileOpen, onFileOpen])

  useEffect(() => {
    if (!exportOpen) return
    const close = (event: MouseEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      if (exportRef.current?.contains(target) || exportMenuRef.current?.contains(target)) return
      setExportOpen(false)
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setExportOpen(false)
    }
    document.addEventListener('mousedown', close)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', close)
      document.removeEventListener('keydown', onKey)
    }
  }, [exportOpen])

  useEffect(() => () => abortRef.current?.abort(), [])

  useEffect(() => {
    const node = stageRef.current
    if (!node || !showPreview) return
    let fitted = false
    const fit = () => {
      if (fitted || node.clientWidth < 80) return
      fitted = true
      setZoom(Math.min(1.4, Math.max(0.4, (node.clientWidth - 48) / SHEET_WIDTH)))
    }
    fit()
    if (typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(fit)
    observer.observe(node)
    return () => observer.disconnect()
  }, [showPreview, view])

  const caret = () => sourceRef.current?.selection() ?? { start: draft.markdown.length, end: draft.markdown.length }

  const applyMarkup = (kind: PlanMarkup) => {
    const range = caret()
    const next = insertPlanMarkup(draft.markdown, range.start, range.end, kind)
    onPatch({ markdown: next.value })
    requestAnimationFrame(() => sourceRef.current?.focusAt(next.cursor))
  }

  const onSourceScroll = (ratio: number) => {
    if (scrollEcho.current === 'source') {
      scrollEcho.current = null
      return
    }
    if (!syncOn) return
    const node = previewRef.current
    if (!node) return
    const max = node.scrollHeight - node.clientHeight
    const top = ratio * Math.max(0, max)
    if (Math.abs(node.scrollTop - top) < 1) return
    scrollEcho.current = 'preview'
    node.scrollTop = top
  }

  const onPreviewScroll = () => {
    const node = previewRef.current
    if (node) {
      const rootTop = node.getBoundingClientRect().top
      let current = 1
      node.querySelectorAll('[data-plan-sheet]').forEach((sheet) => {
        if (!(sheet instanceof HTMLElement)) return
        if (sheet.getBoundingClientRect().top - rootTop < 80) {
          const value = Number(sheet.dataset.planSheet)
          if (value) current = value
        }
      })
      setPageCursor((prev) => (prev === current ? prev : current))
    }
    if (!syncOn || !node) return
    if (scrollEcho.current === 'preview') {
      scrollEcho.current = null
      return
    }
    const max = node.scrollHeight - node.clientHeight
    const ratio = max <= 0 ? 0 : node.scrollTop / max
    scrollEcho.current = 'source'
    const moved = sourceRef.current?.scrollToRatio(ratio) ?? false
    if (!moved) scrollEcho.current = null
  }

  const fitWidth = () => {
    const width = stageRef.current?.clientWidth ?? 0
    if (width < 80) return
    setZoom(Math.min(1.4, Math.max(0.4, (width - 48) / SHEET_WIDTH)))
  }

  const goPage = (page: number) => {
    const next = Math.min(sheetCount, Math.max(1, page))
    setPageCursor(next)
    previewRef.current?.querySelector(`[data-plan-sheet="${next}"]`)?.scrollIntoView({ block: 'start' })
  }

  const reportOverflow = useCallback((page: number, over: boolean) => {
    if ((overflowMap.current.get(page) ?? false) === over) return
    overflowMap.current.set(page, over)
    const next = [...overflowMap.current.entries()].flatMap(([number, value]) => (value ? [number] : []))
    next.sort((left, right) => left - right)
    setOverflowPages(next)
  }, [])

  const setMeta = (key: keyof PlanMeta, value: string) => {
    const meta: Partial<PlanMeta> = {}
    meta[key] = value
    onPatch({ meta })
  }
  const setStyle = (style: Partial<PlanStyle>) => onPatch({ style })

  const onLogo = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!acceptPlanLogo(file.size)) {
      toast.info({ title: t('workbench.plan.logo.too_large'), appearance: 'pill' })
      return
    }
    const reader = new FileReader()
    reader.onload = () => {
      const value = typeof reader.result === 'string' ? reader.result : ''
      if (!/^data:image\/(?:png|jpeg|webp|gif);base64,/.test(value)) {
        toast.info({ title: t('workbench.plan.logo.too_large'), appearance: 'pill' })
        return
      }
      setMeta('logo', value)
    }
    reader.readAsDataURL(file)
  }

  const copySource = () => {
    onFileOpen(false)
    void navigator.clipboard?.writeText(draft.markdown).then(
      () => toast.info({ title: t('workbench.plan.copied'), appearance: 'pill' }),
      () => toast.info({ title: t('workbench.plan.save_failed'), appearance: 'pill' })
    )
  }

  const closeTool = () => {
    abortRef.current?.abort()
    setAiRunning(false)
    setTool(null)
  }

  const onOpenMarkdown = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ''
    onFileOpen(false)
    if (!file) return
    const reader = new FileReader()
    reader.onload = () => {
      const value = typeof reader.result === 'string' ? reader.result : ''
      const clipped = clipPlanMarkdown(value)
      onPatch({ markdown: clipped.text })
      if (clipped.clipped) toast.info({ title: t('workbench.plan.file.too_large'), appearance: 'pill' })
    }
    reader.onerror = () => toast.info({ title: t('workbench.plan.save_failed'), appearance: 'pill' })
    reader.readAsText(file)
  }

  const onBodyImage = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!acceptPlanImage(file.size) || Object.keys(draft.images).length >= 8) {
      toast.info({
        title: t(
          Object.keys(draft.images).length >= 8 ? 'workbench.plan.images.too_many' : 'workbench.plan.logo.too_large'
        ),
        appearance: 'pill'
      })
      return
    }
    const reader = new FileReader()
    reader.onload = () => {
      const value = typeof reader.result === 'string' ? reader.result : ''
      const slot = planImageSlot(draft.images, value, Date.now())
      if (!slot) {
        toast.info({ title: t('workbench.plan.logo.too_large'), appearance: 'pill' })
        return
      }
      const range = caret()
      const alt = file.name.replace(/\.[^.]+$/, '')
      const next = insertPlanImage(draft.markdown, range.start, alt, slot.id)
      onPatch({ markdown: next.value, images: slot.images })
      requestAnimationFrame(() => sourceRef.current?.focusAt(next.cursor))
    }
    reader.readAsDataURL(file)
  }

  const runAi = () => {
    const instruction = aiInstruction.trim()
    if (!instruction) {
      setAiError('workbench.plan.ai.empty')
      return
    }
    abortRef.current?.abort()
    const controller = new AbortController()
    abortRef.current = controller
    setAiRunning(true)
    setAiError('')
    setAiResult('')
    void requestPlanRewrite({ instruction, markdown: draft.markdown, signal: controller.signal })
      .then((next) => {
        if (controller.signal.aborted) return
        const clipped = clipPlanMarkdown(next)
        setAiResult(clipped.text)
        if (clipped.clipped) setAiError('workbench.plan.file.too_large')
      })
      .catch(() => {
        if (controller.signal.aborted) return
        setAiError('workbench.plan.ai.failed')
      })
      .finally(() => {
        if (!controller.signal.aborted) setAiRunning(false)
      })
  }

  const exportDocument = (kind: 'md' | 'html' | 'docx') => {
    setExportOpen(false)
    const title = draft.meta.title
    const task =
      kind === 'md'
        ? savePlanFile(planExportName(title, 'md'), draft.markdown)
        : kind === 'html'
          ? savePlanFile(planExportName(title, 'html'), planDocumentHtml(draft))
          : planDocumentDocx(draft).then((bytes) => savePlanFile(planExportName(title, 'docx'), bytes))
    void task.then(
      (result) => {
        if (result === 'saved') toast.info({ title: t('workbench.plan.export.done'), appearance: 'pill' })
      },
      () => toast.info({ title: t('workbench.plan.export.failed'), appearance: 'pill' })
    )
  }

  const printDocument = () => {
    setExportOpen(false)
    try {
      const preview = document.querySelector('[data-ui="workbench.plan.preview"]')
      printPlanPages(preview instanceof HTMLElement ? preview : null, planDocumentHtml(draft))
    } catch {
      toast.info({ title: t('workbench.plan.export.failed'), appearance: 'pill' })
    }
  }

  const saveDocument = () => {
    const ok = onSave()
    toast.info({ title: t(ok ? 'workbench.plan.saved' : 'workbench.plan.save_failed'), appearance: 'pill' })
    if (!ok) return
    setSavedFlash(true)
    if (savedTimer.current != null) window.clearTimeout(savedTimer.current)
    savedTimer.current = window.setTimeout(() => {
      savedTimer.current = null
      setSavedFlash(false)
    }, 1600)
  }
  const saveDocumentRef = useRef(saveDocument)
  saveDocumentRef.current = saveDocument

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== 's' || event.altKey || event.shiftKey) return
      if (!event.metaKey && !event.ctrlKey) return
      event.preventDefault()
      saveDocumentRef.current()
    }
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('keydown', onKey)
      if (savedTimer.current != null) window.clearTimeout(savedTimer.current)
    }
  }, [])

  const jumpToHeading = (index: number) => {
    sourceRef.current?.focusAt(headingOffset(draft.markdown, index))
  }

  return (
    <div data-ui="workbench.plan.editor" className="relative flex min-h-0 flex-1 flex-col">
      <div
        data-plan-toolbar=""
        className="flex h-10 min-w-0 shrink-0 items-center gap-2 overflow-x-auto border-b border-[#efefef] bg-white px-2">
        <div
          role="radiogroup"
          aria-label={t('workbench.plan.preview')}
          className="flex shrink-0 gap-0.5 rounded-lg bg-[#f5f5f5] p-1">
          {VIEWS.map((item) => {
            const ViewIcon = VIEW_ICON[item]
            return (
              <button
                key={item}
                type="button"
                role="radio"
                aria-checked={view === item}
                data-ui="workbench.plan.view"
                data-view={item}
                onClick={() => onView(item)}
                className={`inline-flex h-[22px] items-center gap-1 rounded-md px-2 text-[13px] ${FOCUS} ${view === item ? 'bg-white font-medium text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.08)]' : 'text-[#3f3f3f]'}`}>
                <ViewIcon className="size-3.5" strokeWidth={1.7} aria-hidden />
                {t(`workbench.plan.view.${item}`)}
              </button>
            )
          })}
        </div>
        <div ref={fileRef} className="relative shrink-0">
          <button type="button" className={GHOST} aria-expanded={fileOpen} onClick={() => onFileOpen(!fileOpen)}>
            <File className="size-3.5" strokeWidth={1.7} aria-hidden />
            {t('workbench.plan.file')}
          </button>
          <AnchoredMenu open={fileOpen} anchor={fileRef} align="start" menuRef={fileMenuRef} className="w-44">
            <button type="button" role="menuitem" className={`${GHOST} w-full justify-start`} onClick={onNew}>
              <FilePlus className="size-3.5" strokeWidth={1.7} aria-hidden />
              {t('workbench.plan.file.new')}
            </button>
            <button
              type="button"
              role="menuitem"
              className={`${GHOST} w-full justify-start`}
              onClick={() => markdownRef.current?.click()}>
              <FileUp className="size-3.5" strokeWidth={1.7} aria-hidden />
              {t('workbench.plan.file.open')}
            </button>
            <button type="button" role="menuitem" className={`${GHOST} w-full justify-start`} onClick={copySource}>
              <Copy className="size-3.5" strokeWidth={1.7} aria-hidden />
              {t('workbench.plan.file.copy')}
            </button>
          </AnchoredMenu>
          <input
            ref={markdownRef}
            className="sr-only"
            type="file"
            accept=".md,text/markdown,text/plain"
            onChange={onOpenMarkdown}
          />
        </div>
        <button type="button" className={`${QUIET} shrink-0`} onClick={saveDocument}>
          {savedFlash ? (
            <Check className="size-3.5" strokeWidth={1.7} aria-hidden />
          ) : (
            <Save className="size-3.5" strokeWidth={1.7} aria-hidden />
          )}
          {savedFlash ? t('workbench.plan.saved') : t('workbench.plan.save')}
        </button>
        <div className="ml-auto flex shrink-0 items-center gap-1">
          <button
            type="button"
            aria-label={t('workbench.plan.ai')}
            aria-expanded={tool === 'ai'}
            className={`${GHOST} ${tool === 'ai' ? 'bg-[#f2f3f5] text-[#171717]' : ''}`}
            onClick={() => (tool === 'ai' ? closeTool() : setTool('ai'))}>
            <Sparkles className="size-3.5" strokeWidth={1.7} aria-hidden />
            {t('workbench.plan.ai')}
          </button>
          <button
            type="button"
            aria-label={t('workbench.plan.images')}
            aria-haspopup="dialog"
            aria-expanded={tool === 'images'}
            className={`${GHOST} ${tool === 'images' ? 'bg-[#f2f3f5] text-[#171717]' : ''}`}
            onClick={() => setTool(tool === 'images' ? null : 'images')}>
            <ImageIcon className="size-3.5" strokeWidth={1.7} aria-hidden />
            {t('workbench.plan.images')}
          </button>
          <button
            type="button"
            aria-label={t('workbench.plan.themes')}
            aria-haspopup="dialog"
            aria-expanded={tool === 'themes'}
            className={`${GHOST} ${tool === 'themes' ? 'bg-[#f2f3f5] text-[#171717]' : ''}`}
            onClick={() => setTool(tool === 'themes' ? null : 'themes')}>
            <SwatchBook className="size-3.5" strokeWidth={1.7} aria-hidden />
            {t('workbench.plan.themes')}
          </button>
          <div ref={exportRef} className="relative">
            <button
              type="button"
              aria-label={t('workbench.plan.export')}
              aria-expanded={exportOpen}
              aria-haspopup="menu"
              className={FILL_SM}
              onClick={() => setExportOpen((open) => !open)}>
              <Download className="size-3.5" strokeWidth={1.7} aria-hidden />
              {t('workbench.plan.export')}
            </button>
            <AnchoredMenu open={exportOpen} anchor={exportRef} align="end" menuRef={exportMenuRef} className="w-72">
              <ExportItem
                title={t('workbench.plan.export.print')}
                detail={t('workbench.plan.export.print_detail')}
                icon={Printer}
                onClick={printDocument}
              />
              <ExportItem
                title={t('workbench.plan.export.word')}
                detail={t('workbench.plan.export.word_detail')}
                icon={FileType}
                onClick={() => exportDocument('docx')}
              />
              <ExportItem
                title={t('workbench.plan.export.html')}
                detail={t('workbench.plan.export.html_detail')}
                icon={FileCode}
                onClick={() => exportDocument('html')}
              />
              <ExportItem
                title={t('workbench.plan.export.markdown')}
                detail={t('workbench.plan.export.markdown_detail')}
                icon={FileText}
                onClick={() => exportDocument('md')}
              />
            </AnchoredMenu>
          </div>
          <input
            ref={imageRef}
            data-ui="workbench.plan.image"
            className="sr-only"
            type="file"
            accept="image/png,image/jpeg,image/webp,image/gif"
            onChange={onBodyImage}
          />
        </div>
      </div>
      <div
        className={`grid min-h-0 min-w-0 flex-1 ${showSource && showPreview ? 'grid-cols-[minmax(280px,0.9fr)_minmax(0,1.35fr)_300px]' : showInspector ? 'grid-cols-[minmax(0,1fr)_300px]' : 'grid-cols-1'}`}>
        {showSource ? (
          <div className="flex min-h-0 min-w-0 flex-col border-r border-[#efefef]">
            <div
              role="toolbar"
              aria-label={t('workbench.plan.source')}
              className="flex h-9 shrink-0 items-center gap-0.5 overflow-x-auto border-b border-[#efefef] px-2">
              <MarkupButton label={t('workbench.plan.toolbar.h1')} mark="H1" onClick={() => applyMarkup('h1')} />
              <MarkupButton label={t('workbench.plan.toolbar.h2')} mark="H2" onClick={() => applyMarkup('h2')} />
              <MarkupButton label={t('workbench.plan.toolbar.h3')} mark="H3" onClick={() => applyMarkup('h3')} />
              <MarkupButton label={t('workbench.plan.toolbar.h4')} mark="H4" onClick={() => applyMarkup('h4')} />
              <span className="mx-1 h-4 w-px shrink-0 bg-[#efefef]" aria-hidden />
              <MarkupButton label={t('workbench.plan.toolbar.bold')} mark="B" onClick={() => applyMarkup('bold')} />
              <MarkupButton label={t('workbench.plan.toolbar.italic')} mark="I" onClick={() => applyMarkup('italic')} />
              <MarkupButton label={t('workbench.plan.toolbar.sup')} mark="x²" onClick={() => applyMarkup('sup')} />
              <MarkupButton label={t('workbench.plan.toolbar.sub')} mark="x₂" onClick={() => applyMarkup('sub')} />
              <span className="mx-1 h-4 w-px shrink-0 bg-[#efefef]" aria-hidden />
              <MarkupButton label={t('workbench.plan.toolbar.list')} icon={List} onClick={() => applyMarkup('list')} />
              <MarkupButton
                label={t('workbench.plan.toolbar.ordered')}
                icon={ListOrdered}
                onClick={() => applyMarkup('ordered')}
              />
              <MarkupButton
                label={t('workbench.plan.toolbar.quote')}
                icon={Quote}
                onClick={() => applyMarkup('quote')}
              />
              <MarkupButton label={t('workbench.plan.toolbar.code')} icon={Code} onClick={() => applyMarkup('code')} />
              <MarkupButton label={t('workbench.plan.toolbar.link')} icon={Link} onClick={() => applyMarkup('link')} />
              <MarkupButton
                label={t('workbench.plan.toolbar.table')}
                icon={Table}
                onClick={() => applyMarkup('table')}
              />
              <MarkupButton label={t('workbench.plan.toolbar.rule')} icon={Minus} onClick={() => applyMarkup('rule')} />
              <MarkupButton
                label={t('workbench.plan.toolbar.break')}
                icon={SeparatorHorizontal}
                onClick={() => applyMarkup('break')}
              />
              <MarkupButton label={t('workbench.plan.images')} icon={ImageIcon} onClick={() => setTool('images')} />
              <button
                type="button"
                aria-label={t('workbench.plan.editor.size')}
                onClick={() => setFontSize((size) => (size === 12 ? 13 : size === 13 ? 15 : 12))}
                className={`${ICON_BUTTON} ml-auto`}>
                <TypeIcon className="size-3.5" strokeWidth={1.7} aria-hidden />
              </button>
            </div>
            <PlanSource
              handle={sourceRef}
              value={draft.markdown}
              label={t('workbench.plan.source')}
              fontSize={fontSize}
              onChange={(markdown) => onPatch({ markdown })}
              onMarkup={applyMarkup}
              onScrollRatio={onSourceScroll}
              onSelectionCount={setSelectedCount}
            />
            <footer className="flex h-8 shrink-0 items-center gap-3 border-t border-[#efefef] px-3 text-[12px] text-[#8a8a8a]">
              <button
                type="button"
                role="switch"
                aria-checked={syncOn}
                aria-label={t('workbench.plan.sync')}
                onClick={() => setSyncOn((on) => !on)}
                className={`inline-flex items-center gap-1.5 ${FOCUS}`}>
                <span className={`relative h-4 w-7 rounded-full ${syncOn ? 'bg-[#171717]' : 'bg-[#e5e5e5]'}`}>
                  <span
                    className={`absolute top-0.5 size-3 rounded-full bg-white ${syncOn ? 'left-3.5' : 'left-0.5'}`}
                  />
                </span>
                {t('workbench.plan.sync')}
              </button>
              <span>{t('workbench.plan.draft.stats', stats)}</span>
              {selectedCount > 0 ? <span>{t('workbench.plan.selection', { count: selectedCount })}</span> : null}
              <span className={`ml-auto inline-flex items-center gap-1 ${saveFailed ? 'text-[#b42318]' : ''}`}>
                {saveFailed ? (
                  <CircleAlert className="size-3.5" strokeWidth={1.7} aria-hidden />
                ) : (
                  <Check className="size-3.5" strokeWidth={1.7} aria-hidden />
                )}
                {saveFailed ? t('workbench.plan.save_failed') : t('workbench.plan.saved')}
              </span>
            </footer>
          </div>
        ) : null}
        {showPreview ? (
          <div ref={stageRef} className="relative flex min-h-0 min-w-0 flex-col bg-[#f5f5f5]">
            <div className="flex h-9 shrink-0 items-center justify-between gap-2 px-3">
              <button
                type="button"
                className={QUIET}
                aria-expanded={outlineOpen}
                onClick={() => setOutlineOpen((open) => !open)}>
                <ListTree className="size-3.5" strokeWidth={1.7} aria-hidden />
                {t('workbench.plan.outline')}
                <span className="text-[#8a8a8a]">{outlineCount}</span>
              </button>
              <span className="truncate text-[12px] text-[#8a8a8a]">
                {t('workbench.plan.pages', { count: sheetCount })}
                {' · '}
                {t('workbench.plan.paper')}
              </span>
            </div>
            {outlineOpen ? (
              <div className="absolute top-10 left-3 z-20 w-64 rounded-xl border border-[#e5e5e5] bg-white p-2 shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
                <HeadingList
                  markdown={draft.markdown}
                  onJump={(index) => {
                    jumpToHeading(index)
                    setOutlineOpen(false)
                  }}
                />
              </div>
            ) : null}
            {overflowPages.length > 0 ? (
              <p className="m-0 px-3 pb-2 text-center text-[12px] text-[#b42318]">
                {t('workbench.plan.overflow', { pages: overflowPages.join(', ') })}
              </p>
            ) : null}
            <div ref={previewRef} onScroll={onPreviewScroll} className="min-h-0 flex-1 overflow-auto px-4 py-4">
              <div data-ui="workbench.plan.preview">
                {coverOn ? (
                  <SheetSlot page={1} zoom={zoom}>
                    <CoverPage draft={draft} />
                  </SheetSlot>
                ) : null}
                {tocSheets.map((entries, index) => (
                  <SheetSlot key={`toc-${index}`} page={coverCount + index + 1} zoom={zoom}>
                    <TocSheet
                      draft={draft}
                      entries={entries}
                      page={coverCount + index + 1}
                      pageCount={sheetCount}
                      pageOffset={pageOffset}
                    />
                  </SheetSlot>
                ))}
                {bodySheets.map((blocks, index) => (
                  <SheetSlot key={index} page={pageOffset + index + 1} zoom={zoom}>
                    <BodyPage
                      draft={draft}
                      blocks={blocks}
                      page={pageOffset + index + 1}
                      pageCount={sheetCount}
                      startHeading={headingStarts[index] ?? 0}
                      startHeadingCounters={headingCursors[index] ?? [0, 0, 0, 0]}
                      startImage={sheetStarts[index]?.image ?? 0}
                      startTable={sheetStarts[index]?.table ?? 0}
                      onHeading={jumpToHeading}
                      onOverflow={reportOverflow}
                    />
                  </SheetSlot>
                ))}
              </div>
            </div>
            <div className="flex h-9 shrink-0 items-center justify-center gap-2 border-t border-[#efefef] bg-white px-2 text-[12px] text-[#3f3f3f]">
              <button
                type="button"
                aria-label={t('workbench.plan.page.prev')}
                disabled={shownPage <= 1}
                onClick={() => goPage(shownPage - 1)}
                className={`${ICON_BUTTON} disabled:opacity-40`}>
                <ChevronLeft className="size-3.5" strokeWidth={1.7} aria-hidden />
              </button>
              <div
                role="group"
                aria-label={t('workbench.plan.page.current', { page: shownPage, count: sheetCount })}
                className="inline-flex items-center gap-1">
                <input
                  aria-label={t('workbench.plan.page.jump')}
                  inputMode="numeric"
                  value={shownPage}
                  onChange={(event) => {
                    const value = Number(event.target.value)
                    if (Number.isFinite(value)) goPage(value)
                  }}
                  className={`h-6 w-8 rounded-md border-0 bg-[#f5f5f5] text-center text-[12px] text-[#171717] ${FOCUS}`}
                />
                <span aria-hidden>/ {sheetCount}</span>
              </div>
              <button
                type="button"
                aria-label={t('workbench.plan.page.next')}
                disabled={shownPage >= sheetCount}
                onClick={() => goPage(shownPage + 1)}
                className={`${ICON_BUTTON} disabled:opacity-40`}>
                <ChevronRight className="size-3.5" strokeWidth={1.7} aria-hidden />
              </button>
              <span className="mx-1 h-4 w-px bg-[#efefef]" aria-hidden />
              <button type="button" className={`${GHOST} h-6`} onClick={fitWidth}>
                {t('workbench.plan.zoom.fit')}
              </button>
              <input
                type="range"
                min={40}
                max={140}
                aria-label={t('workbench.plan.zoom.level', { percent: Math.round(zoom * 100) })}
                value={Math.round(zoom * 100)}
                onChange={(event) => setZoom(Number(event.target.value) / 100)}
              />
              <span className="w-10 text-right text-[#8a8a8a]">
                {t('workbench.plan.zoom.level', { percent: Math.round(zoom * 100) })}
              </span>
            </div>
          </div>
        ) : null}
        {showInspector ? (
          <aside className="flex min-h-0 min-w-0 flex-col border-l border-[#efefef] bg-white">
            <div
              role="tablist"
              aria-label={t('workbench.plan.panel.cover')}
              className="flex shrink-0 gap-0.5 border-b border-[#efefef] px-1.5 py-1">
              {PANELS.map((item) => {
                const PanelIcon = PANEL_ICON[item]
                const label = t(`workbench.plan.panel.${item}`)
                return (
                  <Tooltip key={item} content={label} delay={400} placement="bottom" sideOffset={6} asChild>
                    <button
                      type="button"
                      role="tab"
                      aria-selected={panel === item}
                      aria-label={label}
                      data-ui="workbench.plan.panel"
                      data-panel={item}
                      onClick={() => onPanel(item)}
                      className={`flex h-11 min-w-0 flex-1 flex-col items-center justify-center gap-0.5 rounded-lg ${FOCUS} ${panel === item ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7]'}`}>
                      <PanelIcon className="size-3.5 shrink-0" strokeWidth={1.7} aria-hidden />
                      <span className="w-full truncate text-center text-[11px] leading-4">{label}</span>
                    </button>
                  </Tooltip>
                )
              })}
            </div>
            <div role="tabpanel" className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
              {panel === 'cover' ? (
                <div className="grid gap-3">
                  <div className="flex items-center gap-1.5 text-[13px] font-medium text-[#171717]">
                    <PanelTop className="size-3.5" strokeWidth={1.7} aria-hidden />
                    {t('workbench.plan.meta.section')}
                  </div>
                  <PlanField
                    id="title"
                    label={t('workbench.plan.meta.title')}
                    value={draft.meta.title}
                    onChange={(value) => setMeta('title', value)}
                  />
                  <PlanField
                    id="subtitle"
                    label={t('workbench.plan.meta.subtitle')}
                    value={draft.meta.subtitle}
                    onChange={(value) => setMeta('subtitle', value)}
                  />
                  <PlanField
                    id="number"
                    label={t('workbench.plan.meta.number')}
                    value={draft.meta.number}
                    onChange={(value) => setMeta('number', value)}
                  />
                  <div className="grid grid-cols-[88px_minmax(0,1fr)] gap-2">
                    <PlanField
                      id="version"
                      label={t('workbench.plan.meta.version')}
                      value={draft.meta.version}
                      onChange={(value) => setMeta('version', value)}
                    />
                    <PlanField
                      id="date"
                      label={t('workbench.plan.meta.date')}
                      value={draft.meta.date}
                      onChange={(value) => setMeta('date', value)}
                      type="date"
                    />
                  </div>
                  <PlanField
                    id="author"
                    label={t('workbench.plan.meta.author')}
                    value={draft.meta.author}
                    placeholder={t('workbench.plan.meta.author_placeholder')}
                    onChange={(value) => setMeta('author', value)}
                  />
                  <PlanField
                    id="organization"
                    label={t('workbench.plan.meta.organization')}
                    value={draft.meta.organization}
                    onChange={(value) => setMeta('organization', value)}
                  />
                  <PlanField
                    id="department"
                    label={t('workbench.plan.meta.department')}
                    value={draft.meta.department}
                    onChange={(value) => setMeta('department', value)}
                  />
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-[13px] text-[#171717]">{t('workbench.plan.cover.toggle')}</span>
                    <button
                      type="button"
                      role="switch"
                      aria-checked={draft.showCover}
                      aria-label={t('workbench.plan.cover.toggle')}
                      onClick={() => onPatch({ showCover: !draft.showCover })}
                      className={`relative h-5 w-9 rounded-full ${FOCUS} ${draft.showCover ? 'bg-[#171717]' : 'bg-[#e5e5e5]'}`}>
                      <span
                        className={`absolute top-0.5 size-4 rounded-full bg-white ${draft.showCover ? 'left-4' : 'left-0.5'}`}
                      />
                    </button>
                  </div>
                  <CoverSwatches selected={draft.coverId} onSelect={(coverId) => onPatch({ coverId })} />
                  <div className="grid gap-1">
                    <span className="text-[12px] text-[#8a8a8a]">{t('workbench.plan.logo')}</span>
                    <div className="flex flex-wrap items-center gap-2">
                      <button type="button" className={QUIET} onClick={() => logoRef.current?.click()}>
                        <ImagePlus className="size-3.5" strokeWidth={1.7} aria-hidden />
                        {t('workbench.plan.logo.pick')}
                      </button>
                      <button
                        type="button"
                        className={GHOST}
                        disabled={!draft.meta.logo}
                        onClick={() => setMeta('logo', '')}>
                        <Trash2 className="size-3.5" strokeWidth={1.7} aria-hidden />
                        {t('workbench.plan.logo.clear')}
                      </button>
                    </div>
                    <input
                      ref={logoRef}
                      data-ui="workbench.plan.logo"
                      className="sr-only"
                      type="file"
                      accept="image/png,image/jpeg,image/webp,image/gif"
                      onChange={onLogo}
                    />
                    <p className="m-0 text-[12px] text-[#8a8a8a]">{t('workbench.plan.logo.hint')}</p>
                    <input
                      key={draft.meta.logo.startsWith('https://') ? draft.meta.logo : 'plan-logo-address'}
                      defaultValue={draft.meta.logo.startsWith('https://') ? draft.meta.logo : ''}
                      placeholder={t('workbench.plan.logo.address')}
                      onBlur={(event) => {
                        const value = event.target.value.trim()
                        if (!value) {
                          if (draft.meta.logo.startsWith('https://')) setMeta('logo', '')
                          return
                        }
                        if (/^https:\/\/[^\s]{1,480}$/.test(value)) setMeta('logo', value)
                        else toast.info({ title: t('workbench.plan.logo.rejected'), appearance: 'pill' })
                      }}
                      className={FIELD}
                    />
                  </div>
                  <LayoutSwitch layout={draft.layout} onChange={(layout) => onPatch({ layout })} />
                </div>
              ) : null}
              {panel === 'header' ? (
                <HeaderPanel draft={draft} onStyle={setStyle} onHeader={(value) => setMeta('header', value)} />
              ) : null}
              {panel === 'toc' ? <TocPanel draft={draft} onStyle={setStyle} /> : null}
              {panel === 'color' ? <InkPanel draft={draft} onStyle={setStyle} /> : null}
              {panel === 'style' ? <TypePanel draft={draft} onPatch={onPatch} onStyle={setStyle} /> : null}
              {panel === 'other' ? (
                <OtherPanel draft={draft} onStyle={setStyle} onWatermark={(value) => setMeta('watermark', value)} />
              ) : null}
            </div>
          </aside>
        ) : null}
      </div>
      {tool === 'ai' ? (
        <div
          role="region"
          aria-label={t('workbench.plan.ai')}
          className="absolute right-3 bottom-3 z-30 grid w-[340px] gap-3 rounded-xl border border-[#e5e5e5] bg-white p-3 shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
          <div className="flex items-center justify-between gap-2">
            <p className="m-0 text-[13px] font-medium text-[#171717]">{t('workbench.plan.ai')}</p>
            <button type="button" aria-label={t('common.close')} onClick={closeTool} className={ICON_BUTTON}>
              <X className="size-3.5" strokeWidth={1.7} aria-hidden />
            </button>
          </div>
          <label className="grid gap-1" htmlFor="plan-ai-instruction">
            <span className="text-[12px] text-[#8a8a8a]">{t('workbench.plan.ai.instruction')}</span>
            <textarea
              id="plan-ai-instruction"
              value={aiInstruction}
              placeholder={t('workbench.plan.ai.placeholder')}
              onChange={(event) => setAiInstruction(event.target.value)}
              className={`min-h-[88px] w-full resize-none rounded-lg border-0 bg-[#f5f5f5] px-2.5 py-2 text-[13px] leading-5 text-[#171717] placeholder:text-[#8a8a8a] ${FOCUS}`}
            />
          </label>
          {aiResult ? (
            <div
              aria-label={t('workbench.plan.ai.diff')}
              className="max-h-40 overflow-y-auto rounded-lg border border-[#e5e5e5] bg-white p-2 font-mono text-[12px] leading-5">
              {planLineDiff(draft.markdown, aiResult).map((row, index) => (
                <p
                  key={index}
                  className={`m-0 whitespace-pre-wrap ${row.kind === 'del' ? 'text-[#b42318]' : row.kind === 'add' ? 'text-[#171717]' : 'text-[#8a8a8a]'}`}>
                  {row.text || ' '}
                </p>
              ))}
            </div>
          ) : null}
          {aiError ? <p className="m-0 text-[12px] text-[#b42318]">{t(aiError)}</p> : null}
          <div className="flex justify-end gap-2">
            <button
              type="button"
              disabled={aiRunning}
              onClick={runAi}
              className={`inline-flex h-9 items-center justify-center rounded-lg border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] hover:bg-[#fafafa] disabled:opacity-40 ${FOCUS}`}>
              {aiRunning ? t('workbench.plan.ai.running') : t('workbench.plan.ai.run')}
            </button>
            <button
              type="button"
              className={FILL}
              disabled={!aiResult || aiRunning}
              onClick={() => {
                onPatch({ markdown: aiResult })
                closeTool()
              }}>
              {t('workbench.plan.ai.apply')}
            </button>
          </div>
        </div>
      ) : null}
      <Dialog
        open={tool === 'images'}
        onOpenChange={(open) => {
          if (!open) setTool(null)
        }}>
        <DialogContent closeLabel={t('common.close')} className="grid gap-3">
          <DialogHeader className="gap-1 text-left">
            <DialogTitle className="text-[16px] font-medium text-[#171717]">{t('workbench.plan.images')}</DialogTitle>
            <DialogDescription className="sr-only">{t('workbench.plan.images')}</DialogDescription>
          </DialogHeader>
          <div>
            <button type="button" className={QUIET} onClick={() => imageRef.current?.click()}>
              <ImagePlus className="size-3.5" strokeWidth={1.7} aria-hidden />
              {t('workbench.plan.logo.pick')}
            </button>
          </div>
          {Object.keys(draft.images).length === 0 ? (
            <p className="m-0 text-[13px] text-[#8a8a8a]">{t('workbench.plan.images.empty')}</p>
          ) : (
            <ul className="m-0 grid max-h-64 list-none gap-2 overflow-y-auto p-0">
              {Object.entries(draft.images).map(([id, src]) => (
                <li key={id} className="flex items-center gap-2 border-b border-[#efefef] pb-2">
                  <img src={src} alt="" className="size-10 shrink-0 rounded-md object-cover" />
                  <span className="min-w-0 flex-1 truncate text-[12px] text-[#8a8a8a]">{id}</span>
                  <button
                    type="button"
                    className={GHOST}
                    onClick={() => {
                      const range = caret()
                      const next = insertPlanImage(draft.markdown, range.start, id, id)
                      onPatch({ markdown: next.value })
                      requestAnimationFrame(() => sourceRef.current?.focusAt(next.cursor))
                    }}>
                    {t('workbench.plan.images.insert')}
                  </button>
                  <button
                    type="button"
                    className={GHOST}
                    onClick={() => onPatch(removePlanImage(draft.markdown, draft.images, id))}>
                    {t('workbench.plan.images.remove')}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </DialogContent>
      </Dialog>
      <Dialog
        open={tool === 'themes'}
        onOpenChange={(open) => {
          if (!open) setTool(null)
        }}>
        <DialogContent closeLabel={t('common.close')}>
          <DialogHeader className="gap-1 text-left">
            <DialogTitle className="text-[16px] font-medium text-[#171717]">{t('workbench.plan.themes')}</DialogTitle>
            <DialogDescription className="text-[12px] text-[#8a8a8a]">
              {t('workbench.plan.cover.section')}
            </DialogDescription>
          </DialogHeader>
          <CoverSwatches
            selected={draft.coverId}
            onSelect={(coverId) => {
              onPatch({ coverId, style: THEME_LOOK[coverId] })
              setTool(null)
            }}
          />
        </DialogContent>
      </Dialog>
    </div>
  )
}

/** Portaled so the horizontal toolbar scroller cannot clip the menu. */
function AnchoredMenu({
  open,
  anchor,
  align,
  menuRef,
  className,
  children
}: {
  open: boolean
  anchor: { readonly current: HTMLElement | null }
  align: 'start' | 'end'
  menuRef: RefObject<HTMLDivElement | null>
  className: string
  children: ReactNode
}) {
  const [box, setBox] = useState<{ top: number; left?: number; right?: number }>({ top: 40, left: 8 })
  useLayoutEffect(() => {
    if (!open) return
    const update = () => {
      const node = anchor.current
      if (!node) return
      const rect = node.getBoundingClientRect()
      setBox(
        align === 'end'
          ? { top: rect.bottom + 4, right: Math.max(8, window.innerWidth - rect.right) }
          : { top: rect.bottom + 4, left: rect.left }
      )
    }
    update()
    const scroller = anchor.current?.closest('[data-plan-toolbar]')
    window.addEventListener('resize', update)
    scroller?.addEventListener('scroll', update)
    return () => {
      window.removeEventListener('resize', update)
      scroller?.removeEventListener('scroll', update)
    }
  }, [open, anchor, align])
  if (!open || typeof document === 'undefined') return null
  return createPortal(
    <div
      ref={menuRef}
      role="menu"
      style={{ position: 'fixed', top: box.top, left: box.left, right: box.right, zIndex: 80 }}
      className={`rounded-xl border border-[#e5e5e5] bg-white p-1 shadow-[0_12px_32px_rgba(0,0,0,0.12)] [-webkit-app-region:no-drag] ${className}`}>
      {children}
    </div>,
    document.body
  )
}

function MarkupButton({
  label,
  onClick,
  icon: Icon,
  mark
}: {
  label: string
  onClick: () => void
  icon?: LucideIcon
  mark?: string
}) {
  return (
    <Tooltip content={label} delay={400} placement="bottom" sideOffset={6} asChild>
      <button type="button" aria-label={label} onClick={onClick} className={mark ? MARK_BUTTON : ICON_BUTTON}>
        {mark ? (
          <span aria-hidden>{mark}</span>
        ) : Icon ? (
          <Icon className="size-3.5" strokeWidth={1.7} aria-hidden />
        ) : null}
      </button>
    </Tooltip>
  )
}

function SheetSlot({ page, zoom, children }: { page: number; zoom: number; children: ReactNode }) {
  return (
    <div
      data-plan-sheet={page}
      data-plan-slot=""
      className="mx-auto mb-8"
      style={{ width: SHEET_WIDTH * zoom, height: SHEET_HEIGHT * zoom }}>
      <div
        data-plan-scale=""
        style={{ width: SHEET_WIDTH, height: SHEET_HEIGHT, transform: `scale(${zoom})`, transformOrigin: 'top left' }}>
        {children}
      </div>
    </div>
  )
}

function ExportItem({
  title,
  detail,
  onClick,
  icon: Icon
}: {
  title: string
  detail: string
  onClick: () => void
  icon: LucideIcon
}) {
  return (
    <button
      type="button"
      role="menuitem"
      aria-label={title}
      onClick={onClick}
      className={`${GHOST} h-auto w-full items-start justify-start py-1.5 text-left`}>
      <Icon className="mt-0.5 size-3.5 shrink-0" strokeWidth={1.7} aria-hidden />
      <span className="grid min-w-0">
        <span className="text-[13px] text-[#171717]">{title}</span>
        <span aria-hidden className="text-[12px] font-normal text-[#8a8a8a]">
          {detail}
        </span>
      </span>
    </button>
  )
}

function PlanField({
  id,
  label,
  value,
  onChange,
  placeholder,
  type = 'text',
  name,
  maxLength = 160
}: {
  id: string
  label: string
  value: string
  onChange: (value: string) => void
  placeholder?: string
  type?: 'text' | 'date'
  name?: string
  maxLength?: number
}) {
  return (
    <label className="grid min-w-0 gap-1" htmlFor={`plan-${id}`}>
      <span className="text-[12px] leading-4 text-[#8a8a8a]">{label}</span>
      <input
        id={`plan-${id}`}
        data-ui={`workbench.plan.field.${id}`}
        aria-label={name}
        type={type}
        value={value}
        placeholder={placeholder}
        maxLength={maxLength}
        onChange={(event) => onChange(event.target.value)}
        className={FIELD}
      />
    </label>
  )
}

function CoverSwatches({ selected, onSelect }: { selected: PlanCoverId; onSelect: (id: PlanCoverId) => void }) {
  const { t } = useTranslation()
  return (
    <div className="grid gap-1">
      <span className="text-[12px] text-[#8a8a8a]">{t('workbench.plan.cover.section')}</span>
      <div className="grid grid-cols-4 gap-2">
        {PLAN_COVERS.map((cover) => (
          <Tooltip key={cover.id} content={t(cover.labelKey)} delay={400} placement="bottom" sideOffset={6} asChild>
            <button
              type="button"
              aria-pressed={selected === cover.id}
              aria-label={t(cover.labelKey)}
              data-ui="workbench.plan.cover-choice"
              data-cover={cover.id}
              onClick={() => onSelect(cover.id)}
              className={`grid min-w-0 justify-items-center gap-1 rounded-lg p-1 hover:bg-[#f7f7f7] ${FOCUS} ${selected === cover.id ? 'bg-[#f2f3f5] ring-2 ring-[#171717]' : ''}`}>
              <span
                className="flex h-16 w-full flex-col justify-end rounded-md border border-[#efefef] px-1.5 pb-1.5"
                style={{ background: cover.paper }}>
                <span className="mb-1 h-0.5 w-4" style={{ background: cover.accent }} />
                <span className="h-1 w-8 rounded-full" style={{ background: cover.ink }} />
              </span>
              <span className="w-full truncate text-center text-[11px] text-[#3f3f3f]">{t(cover.labelKey)}</span>
            </button>
          </Tooltip>
        ))}
      </div>
    </div>
  )
}

function LayoutSwitch({ layout, onChange }: { layout: PlanLayout; onChange: (layout: PlanLayout) => void }) {
  const { t } = useTranslation()
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-[12px] text-[#8a8a8a]">{t('workbench.plan.layout')}</span>
      <div
        role="radiogroup"
        aria-label={t('workbench.plan.layout')}
        className="flex w-36 gap-0.5 rounded-lg bg-[#f5f5f5] p-1">
        {(['single', 'double'] as const).map((item) => (
          <button
            key={item}
            type="button"
            role="radio"
            aria-checked={layout === item}
            onClick={() => onChange(item)}
            className={`h-[22px] flex-1 rounded-md text-[13px] ${FOCUS} ${layout === item ? 'bg-white font-medium text-[#171717]' : 'text-[#3f3f3f]'}`}>
            {t(`workbench.plan.layout.${item}`)}
          </button>
        ))}
      </div>
    </div>
  )
}

function OtherPanel({
  draft,
  onStyle,
  onWatermark
}: {
  draft: PlanDraft
  onStyle: (change: Partial<PlanStyle>) => void
  onWatermark: (value: string) => void
}) {
  const { t } = useTranslation()
  const fileRef = useRef<HTMLInputElement>(null)
  const style = draft.style
  const onFile = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!acceptPlanLogo(file.size)) {
      toast.info({ title: t('workbench.plan.logo.too_large'), appearance: 'pill' })
      return
    }
    const reader = new FileReader()
    reader.onload = () => {
      const value = planAssetUrl(typeof reader.result === 'string' ? reader.result : '')
      if (!value) {
        toast.info({ title: t('workbench.plan.logo.rejected'), appearance: 'pill' })
        return
      }
      onStyle({ watermarkImage: value, watermarkKind: 'image', watermarkOn: true })
    }
    reader.readAsDataURL(file)
  }
  const bulletOptions = (['dot', 'square', 'check', 'arrow'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.style.bullet.${value}`)
  }))
  const numberOptions = (['decimal', 'paren', 'cjk'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.style.numbers.${value}`)
  }))
  const codeOptions = (['light', 'dark'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.style.code.${value}`)
  }))
  const tableOptions = (['minimal', 'striped', 'bordered'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.style.table.${value}`)
  }))
  const frameOptions = (['none', 'shadow', 'line', 'deep', 'card', 'round'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.style.figure.${value}`)
  }))
  const alignOptions = (['left', 'center', 'right'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.style.align.${value}`)
  }))
  const placeOptions = (['above', 'below'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.style.tablecap.${value}`)
  }))
  const figureName = t('workbench.plan.style.figure')
  const tableName = t('workbench.plan.style.tablecap')
  return (
    <div className="grid gap-3">
      <StyleSection title={t('workbench.plan.style.content')}>
        <Segmented
          label={t('workbench.plan.style.paging')}
          value={style.pagination}
          options={[
            { value: 'manual', label: t('workbench.plan.style.manual') },
            { value: 'auto', label: t('workbench.plan.style.auto') }
          ]}
          onChange={(pagination) => onStyle({ pagination })}
        />
        <div className="grid grid-cols-2 gap-2">
          <StyleSelect
            label={t('workbench.plan.style.bullet')}
            name={t('workbench.plan.style.bullet')}
            value={style.bullet}
            options={bulletOptions}
            onChange={(bullet) => onStyle({ bullet })}
          />
          <StyleSelect
            label={t('workbench.plan.style.numbers')}
            name={t('workbench.plan.style.numbers')}
            value={style.numbers}
            options={numberOptions}
            onChange={(numbers) => onStyle({ numbers })}
          />
          <StyleSelect
            label={t('workbench.plan.style.code')}
            name={t('workbench.plan.style.code')}
            value={style.code}
            options={codeOptions}
            onChange={(code) => onStyle({ code })}
          />
          <StyleSelect
            label={t('workbench.plan.style.table')}
            name={t('workbench.plan.style.table')}
            value={style.table}
            options={tableOptions}
            onChange={(table) => onStyle({ table })}
          />
        </div>
      </StyleSection>
      <StyleSection title={figureName}>
        <div className="grid grid-cols-3 gap-2">
          <StyleSelect
            label={t('workbench.plan.style.figure.frame')}
            name={t('workbench.plan.style.figure.frame')}
            value={style.imageFrame}
            options={frameOptions}
            onChange={(imageFrame) => onStyle({ imageFrame })}
          />
          <PrefixField
            id="plan-style-figure-prefix"
            label={t('workbench.plan.style.figure.prefix')}
            name={figureName + ' ' + t('workbench.plan.style.figure.prefix')}
            value={style.imagePrefix}
            onChange={(imagePrefix) => onStyle({ imagePrefix })}
          />
          <StyleSelect
            label={t('workbench.plan.style.align')}
            name={figureName + ' ' + t('workbench.plan.style.align')}
            value={style.imageAlign}
            options={alignOptions}
            onChange={(imageAlign) => onStyle({ imageAlign })}
          />
        </div>
        <div className="flex flex-wrap gap-3">
          <FlagRow
            label={t('workbench.plan.style.caption')}
            name={figureName + ' ' + t('workbench.plan.style.caption')}
            checked={style.imageCaption}
            onChange={(imageCaption) => onStyle({ imageCaption })}
          />
          <FlagRow
            label={t('workbench.plan.style.caption.number')}
            name={figureName + ' ' + t('workbench.plan.style.caption.number')}
            checked={style.imageNumber}
            onChange={(imageNumber) => onStyle({ imageNumber })}
          />
        </div>
      </StyleSection>
      <StyleSection title={tableName}>
        <div className="grid grid-cols-3 gap-2">
          <StyleSelect
            label={t('workbench.plan.style.tablecap.place')}
            name={t('workbench.plan.style.tablecap.place')}
            value={style.tablePlace}
            options={placeOptions}
            onChange={(tablePlace) => onStyle({ tablePlace })}
          />
          <PrefixField
            id="plan-style-table-prefix"
            label={t('workbench.plan.style.tablecap.prefix')}
            name={tableName + ' ' + t('workbench.plan.style.tablecap.prefix')}
            value={style.tablePrefix}
            onChange={(tablePrefix) => onStyle({ tablePrefix })}
          />
          <StyleSelect
            label={t('workbench.plan.style.align')}
            name={tableName + ' ' + t('workbench.plan.style.align')}
            value={style.tableAlign}
            options={alignOptions}
            onChange={(tableAlign) => onStyle({ tableAlign })}
          />
        </div>
        <div className="flex flex-wrap gap-3">
          <FlagRow
            label={t('workbench.plan.style.caption')}
            name={tableName + ' ' + t('workbench.plan.style.caption')}
            checked={style.tableCaption}
            onChange={(tableCaption) => onStyle({ tableCaption })}
          />
          <FlagRow
            label={t('workbench.plan.style.caption.number')}
            name={tableName + ' ' + t('workbench.plan.style.caption.number')}
            checked={style.tableNumber}
            onChange={(tableNumber) => onStyle({ tableNumber })}
          />
        </div>
      </StyleSection>
      <StyleSection
        title={t('workbench.plan.style.watermark')}
        extra={
          style.watermarkOn ? (
            <InkSwitch
              checked
              label={t('workbench.plan.style.watermark')}
              onChange={(watermarkOn) => onStyle({ watermarkOn })}
            />
          ) : (
            <span className="text-[12px] text-[#8a8a8a]">{t('workbench.plan.style.watermark.off')}</span>
          )
        }>
        {style.watermarkOn ? (
          <div className="grid gap-2">
            <Segmented
              label={t('workbench.plan.style.watermark')}
              showLabel={false}
              value={style.watermarkKind}
              options={[
                { value: 'text', label: t('workbench.plan.style.watermark.text') },
                { value: 'image', label: t('workbench.plan.style.watermark.image') }
              ]}
              onChange={(watermarkKind) => onStyle({ watermarkKind })}
            />
            {style.watermarkKind === 'text' ? (
              <PlanField
                id="watermark"
                label={t('workbench.plan.watermark')}
                value={draft.meta.watermark}
                onChange={onWatermark}
              />
            ) : (
              <div className="grid gap-2">
                <div className="flex flex-wrap items-center gap-2">
                  <button type="button" className={QUIET} onClick={() => fileRef.current?.click()}>
                    <ImagePlus className="size-3.5" strokeWidth={1.7} aria-hidden />
                    {t('workbench.plan.logo.pick')}
                  </button>
                  <button
                    type="button"
                    className={GHOST}
                    disabled={!style.watermarkImage}
                    onClick={() => onStyle({ watermarkImage: '' })}>
                    <Trash2 className="size-3.5" strokeWidth={1.7} aria-hidden />
                    {t('workbench.plan.logo.clear')}
                  </button>
                </div>
                <input
                  ref={fileRef}
                  className="sr-only"
                  type="file"
                  accept="image/png,image/jpeg,image/webp,image/gif"
                  aria-label={t('workbench.plan.style.watermark.image')}
                  onChange={onFile}
                />
                {style.watermarkImage ? (
                  <img src={style.watermarkImage} alt="" className="h-16 w-16 rounded-md object-cover" />
                ) : null}
              </div>
            )}
          </div>
        ) : (
          <div className="grid justify-items-center gap-3 rounded-lg bg-[#f5f5f5] px-3 py-4 text-center">
            <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('workbench.plan.style.watermark.hint')}</p>
            <button type="button" className={FILL} onClick={() => onStyle({ watermarkOn: true })}>
              {t('workbench.plan.style.watermark.on')}
            </button>
          </div>
        )}
      </StyleSection>
    </div>
  )
}

function HeaderPanel({
  draft,
  onStyle,
  onHeader
}: {
  draft: PlanDraft
  onStyle: (change: Partial<PlanStyle>) => void
  onHeader: (value: string) => void
}) {
  const { t } = useTranslation()
  const style = draft.style
  const headerName = t('workbench.plan.chrome.header')
  const footerName = t('workbench.plan.chrome.footer')
  const ruleOptions = (['solid', 'accent', 'double', 'none'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.chrome.rule.${value}`)
  }))
  const pageOptions = (['page', 'total', 'hyphen', 'simple', 'none'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.chrome.page.${value}`)
  }))
  const placeOptions = (['left', 'center', 'right'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.chrome.place.${value}`)
  }))
  return (
    <div className="grid gap-3">
      <StyleSection title={headerName}>
        <FlagRow
          label={t('workbench.plan.chrome.header.show')}
          name={t('workbench.plan.chrome.header.show')}
          checked={style.headerOn}
          onChange={(headerOn) => onStyle({ headerOn })}
        />
        <PlanField
          id="header-left"
          name={headerName + ' ' + t('workbench.plan.chrome.header.left')}
          label={t('workbench.plan.chrome.header.left')}
          value={style.headerLeft}
          maxLength={80}
          onChange={(headerLeft) => onStyle({ headerLeft })}
        />
        <PlanField
          id="header"
          name={headerName + ' ' + t('workbench.plan.header.label')}
          label={t('workbench.plan.header.label')}
          value={draft.meta.header}
          onChange={onHeader}
        />
        <PlanField
          id="header-right"
          name={headerName + ' ' + t('workbench.plan.chrome.header.right')}
          label={t('workbench.plan.chrome.header.right')}
          value={style.headerRight}
          maxLength={80}
          onChange={(headerRight) => onStyle({ headerRight })}
        />
        <StyleSelect
          label={t('workbench.plan.chrome.header.rule')}
          name={t('workbench.plan.chrome.header.rule')}
          value={style.headerRule}
          options={ruleOptions}
          onChange={(headerRule) => onStyle({ headerRule })}
        />
      </StyleSection>
      <StyleSection title={footerName}>
        <FlagRow
          label={t('workbench.plan.chrome.footer.show')}
          name={t('workbench.plan.chrome.footer.show')}
          checked={style.footerOn}
          onChange={(footerOn) => onStyle({ footerOn })}
        />
        <PlanField
          id="footer-left"
          name={footerName + ' ' + t('workbench.plan.chrome.footer.left')}
          label={t('workbench.plan.chrome.footer.left')}
          value={style.footerLeft}
          maxLength={80}
          onChange={(footerLeft) => onStyle({ footerLeft })}
        />
        <PlanField
          id="footer-center"
          name={footerName + ' ' + t('workbench.plan.chrome.footer.center')}
          label={t('workbench.plan.chrome.footer.center')}
          value={style.footerCenter}
          maxLength={80}
          onChange={(footerCenter) => onStyle({ footerCenter })}
        />
        <PlanField
          id="footer-right"
          name={footerName + ' ' + t('workbench.plan.chrome.footer.right')}
          label={t('workbench.plan.chrome.footer.right')}
          value={style.footerRight}
          maxLength={80}
          onChange={(footerRight) => onStyle({ footerRight })}
        />
        <StyleSelect
          label={t('workbench.plan.chrome.page')}
          name={t('workbench.plan.chrome.page')}
          value={style.pageFormat}
          options={pageOptions}
          onChange={(pageFormat) => onStyle({ pageFormat })}
        />
        <StyleSelect
          label={t('workbench.plan.chrome.place')}
          name={t('workbench.plan.chrome.place')}
          value={style.pagePlace}
          options={placeOptions}
          onChange={(pagePlace) => onStyle({ pagePlace })}
        />
      </StyleSection>
    </div>
  )
}

function TocPanel({ draft, onStyle }: { draft: PlanDraft; onStyle: (change: Partial<PlanStyle>) => void }) {
  const { t } = useTranslation()
  const style = draft.style
  const depthOptions = ([1, 2, 3, 4] as const).map((value) => ({
    value: String(value) as '1' | '2' | '3' | '4',
    label: t(`workbench.plan.toc.depth.${value}`)
  }))
  const leaderOptions = (['dots', 'dashes', 'line', 'none'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.toc.leader.${value}`)
  }))
  const formOptions = (['underline', 'accent', 'badge', 'minimal'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.toc.form.${value}`)
  }))
  return (
    <div className="grid gap-3">
      <FlagRow
        label={t('workbench.plan.toc.generate')}
        name={t('workbench.plan.toc.generate')}
        checked={style.tocOn}
        onChange={(tocOn) => onStyle({ tocOn })}
      />
      <PlanField
        id="toc-title"
        label={t('workbench.plan.toc.title')}
        value={style.tocTitle}
        maxLength={80}
        onChange={(tocTitle) => onStyle({ tocTitle })}
      />
      <FlagRow
        label={t('workbench.plan.toc.center')}
        name={t('workbench.plan.toc.center')}
        checked={style.tocCenter}
        onChange={(tocCenter) => onStyle({ tocCenter })}
      />
      <StyleSelect
        label={t('workbench.plan.toc.form')}
        name={t('workbench.plan.toc.form')}
        value={style.tocForm}
        options={formOptions}
        onChange={(tocForm) => onStyle({ tocForm })}
      />
      <StyleSelect
        label={t('workbench.plan.toc.depth')}
        name={t('workbench.plan.toc.depth')}
        value={String(style.tocDepth) as '1' | '2' | '3' | '4'}
        options={depthOptions}
        onChange={(value) => {
          const depth = Number(value)
          if (depth === 1 || depth === 2 || depth === 3 || depth === 4) onStyle({ tocDepth: depth })
        }}
      />
      <StyleSelect
        label={t('workbench.plan.toc.leader')}
        name={t('workbench.plan.toc.leader')}
        value={style.tocLeader}
        options={leaderOptions}
        onChange={(tocLeader) => onStyle({ tocLeader })}
      />
      <FlagRow
        label={t('workbench.plan.toc.pages')}
        name={t('workbench.plan.toc.pages')}
        checked={style.tocPages}
        onChange={(tocPages) => onStyle({ tocPages })}
      />
      <TocList markdown={draft.markdown} />
    </div>
  )
}

function TypePanel({
  draft,
  onPatch,
  onStyle
}: {
  draft: PlanDraft
  onPatch: (change: { bodySize: PlanBodySize }) => void
  onStyle: (change: Partial<PlanStyle>) => void
}) {
  const { t } = useTranslation()
  const style = draft.style
  const fontOptions = (['sans', 'serif', 'kai', 'hei', 'mono'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.type.font.${value}`)
  }))
  const numberingOptions = (['none', 'decimal', 'skip', 'cjk', 'mixed'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.type.numbering.${value}`)
  }))
  const h1Options = (['underline', 'accent', 'badge', 'minimal'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.type.h1.${value}`)
  }))
  const h2Options = (['bar', 'line', 'plain'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.type.h2.${value}`)
  }))
  const leadingOptions = (['tight', 'normal', 'loose'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.type.leading.${value}`)
  }))
  return (
    <div className="grid gap-3">
      <div className="grid gap-1">
        <span className="text-[12px] text-[#8a8a8a]">{t('workbench.plan.style.size')}</span>
        <div
          role="radiogroup"
          aria-label={t('workbench.plan.style.size')}
          className="flex gap-0.5 rounded-lg bg-[#f5f5f5] p-1">
          {SIZES.map((size) => (
            <button
              key={size}
              type="button"
              role="radio"
              aria-checked={draft.bodySize === size}
              onClick={() => onPatch({ bodySize: size })}
              className={`h-[22px] flex-1 rounded-md text-[13px] ${FOCUS} ${draft.bodySize === size ? 'bg-white font-medium text-[#171717]' : 'text-[#3f3f3f]'}`}>
              {t(`workbench.plan.style.${size}`)}
            </button>
          ))}
        </div>
      </div>
      <StyleSelect
        label={t('workbench.plan.type.font')}
        name={t('workbench.plan.type.font')}
        value={style.font}
        options={fontOptions}
        onChange={(font) => onStyle({ font })}
      />
      <StyleSelect
        label={t('workbench.plan.type.numbering')}
        name={t('workbench.plan.type.numbering')}
        value={style.numbering}
        options={numberingOptions}
        onChange={(numbering) => onStyle({ numbering })}
      />
      <StyleSelect
        label={t('workbench.plan.type.h1')}
        name={t('workbench.plan.type.h1')}
        value={style.h1Form}
        options={h1Options}
        onChange={(h1Form) => onStyle({ h1Form })}
      />
      <FlagRow
        label={t('workbench.plan.type.h1.break')}
        name={t('workbench.plan.type.h1.break')}
        checked={style.h1Break}
        onChange={(h1Break) => onStyle({ h1Break })}
      />
      <FlagRow
        label={t('workbench.plan.type.h1.center')}
        name={t('workbench.plan.type.h1.center')}
        checked={style.h1Center}
        onChange={(h1Center) => onStyle({ h1Center })}
      />
      <StyleSelect
        label={t('workbench.plan.type.h2')}
        name={t('workbench.plan.type.h2')}
        value={style.h2Form}
        options={h2Options}
        onChange={(h2Form) => onStyle({ h2Form })}
      />
      <Segmented
        label={t('workbench.plan.type.leading')}
        value={style.leading}
        options={leadingOptions}
        onChange={(leading) => onStyle({ leading })}
      />
      <FlagRow
        label={t('workbench.plan.type.indent')}
        name={t('workbench.plan.type.indent')}
        checked={style.indent}
        onChange={(indent) => onStyle({ indent })}
      />
    </div>
  )
}

const INK_SWATCHES = [
  { id: 'cover', key: 'workbench.plan.ink.cover' },
  { id: 'ink', key: 'workbench.plan.ink.ink', color: '#171717' },
  { id: 'pine', key: 'workbench.plan.ink.pine', color: '#1f3d32' },
  { id: 'wine', key: 'workbench.plan.ink.wine', color: '#8f1d1d' },
  { id: 'violet', key: 'workbench.plan.ink.violet', color: '#4c3d8f' },
  { id: 'amber', key: 'workbench.plan.ink.amber', color: '#8a5a12' }
] as const

function InkPanel({ draft, onStyle }: { draft: PlanDraft; onStyle: (change: Partial<PlanStyle>) => void }) {
  const { t } = useTranslation()
  const coverAccent = planCover(draft.coverId).accent
  const themeOptions = (['neutral', 'default', 'dark', 'forest', 'base'] as const).map((value) => ({
    value,
    label: t(`workbench.plan.ink.mermaid.${value}`)
  }))
  return (
    <div className="grid gap-3">
      <StyleSection title={t('workbench.plan.ink.section')}>
        <div className="grid grid-cols-3 gap-2">
          {INK_SWATCHES.map((item) => {
            const color = item.id === 'cover' ? coverAccent : item.color
            return (
              <button
                key={item.id}
                type="button"
                aria-pressed={draft.style.ink === item.id}
                aria-label={t(item.key)}
                onClick={() => onStyle({ ink: item.id })}
                className={`grid min-w-0 justify-items-center gap-1 rounded-lg p-1 hover:bg-[#f7f7f7] ${FOCUS} ${draft.style.ink === item.id ? 'bg-[#f2f3f5] ring-2 ring-[#171717]' : ''}`}>
                <span className="h-8 w-full rounded-md border border-[#efefef]" style={{ background: color }} />
                <span className="w-full truncate text-center text-[11px] text-[#3f3f3f]">{t(item.key)}</span>
              </button>
            )
          })}
        </div>
      </StyleSection>
      <StyleSelect
        label={t('workbench.plan.ink.mermaid')}
        name={t('workbench.plan.ink.mermaid')}
        value={draft.style.mermaid}
        options={themeOptions}
        onChange={(mermaid) => onStyle({ mermaid })}
      />
    </div>
  )
}

function StyleSection({ title, extra, children }: { title: string; extra?: ReactNode; children: ReactNode }) {
  return (
    <section className="grid gap-2 border-t border-[#efefef] pt-3 first:border-t-0 first:pt-0">
      <div className="flex items-center justify-between gap-2">
        <h3 className="m-0 text-[13px] font-medium text-[#171717]">{title}</h3>
        {extra}
      </div>
      {children}
    </section>
  )
}

function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
  showLabel = true
}: {
  label: string
  value: T
  options: readonly { value: T; label: string }[]
  onChange: (value: T) => void
  showLabel?: boolean
}) {
  return (
    <div className="grid gap-1">
      <span className={showLabel ? 'text-[12px] leading-4 text-[#8a8a8a]' : 'sr-only'}>{label}</span>
      <div role="radiogroup" aria-label={label} className="flex gap-0.5 rounded-lg bg-[#f5f5f5] p-1">
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={value === option.value}
            onClick={() => onChange(option.value)}
            className={`h-[22px] min-w-0 flex-1 truncate rounded-md px-1 text-[13px] ${FOCUS} ${value === option.value ? 'bg-white font-medium text-[#171717]' : 'text-[#3f3f3f]'}`}>
            {option.label}
          </button>
        ))}
      </div>
    </div>
  )
}

function StyleSelect<T extends string>({
  label,
  name,
  value,
  options,
  onChange
}: {
  label: string
  name: string
  value: T
  options: readonly { value: T; label: string }[]
  onChange: (value: T) => void
}) {
  return (
    <div className="grid min-w-0 gap-1">
      <span aria-hidden className="text-[12px] leading-4 text-[#8a8a8a]">
        {label}
      </span>
      <SearchSelect
        searchable={options.length > 8}
        allowClear={false}
        aria-label={name}
        value={value}
        options={options.map((option) => ({ value: option.value, label: option.label }))}
        onChange={(raw) => {
          const next = options.find((option) => option.value === raw)
          if (next) onChange(next.value)
        }}
      />
    </div>
  )
}

function PrefixField({
  id,
  label,
  name,
  value,
  onChange
}: {
  id: string
  label: string
  name: string
  value: string
  onChange: (value: string) => void
}) {
  return (
    <label className="grid min-w-0 gap-1" htmlFor={id}>
      <span className="text-[12px] leading-4 text-[#8a8a8a]">{label}</span>
      <input
        id={id}
        aria-label={name}
        value={value}
        maxLength={8}
        onChange={(event) => onChange(event.target.value.slice(0, 8))}
        className={COMPACT}
      />
    </label>
  )
}

function FlagRow({
  label,
  name,
  checked,
  onChange
}: {
  label: string
  name: string
  checked: boolean
  onChange: (checked: boolean) => void
}) {
  return (
    <div className="flex items-center gap-2">
      <InkSwitch checked={checked} label={name} onChange={onChange} />
      <span className="text-[13px] text-[#171717]">{label}</span>
    </div>
  )
}

function InkSwitch({
  checked,
  label,
  onChange
}: {
  checked: boolean
  label: string
  onChange: (checked: boolean) => void
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-5 w-9 shrink-0 rounded-full ${FOCUS} ${checked ? 'bg-[#171717]' : 'bg-[#e5e5e5]'}`}>
      <span className={`absolute top-0.5 size-4 rounded-full bg-white ${checked ? 'left-4' : 'left-0.5'}`} />
    </button>
  )
}

function WatermarkMark({ draft }: { draft: PlanDraft }) {
  if (!draft.style.watermarkOn) return null
  if (draft.style.watermarkKind === 'image') {
    if (!draft.style.watermarkImage) return null
    return (
      <img
        alt=""
        src={draft.style.watermarkImage}
        className="pointer-events-none absolute top-1/2 left-1/2 max-h-40 max-w-[240px] -translate-x-1/2 -translate-y-1/2 opacity-20"
      />
    )
  }
  if (!draft.meta.watermark) return null
  return (
    <p className="pointer-events-none absolute top-1/2 left-1/2 m-0 -translate-x-1/2 -translate-y-1/2 -rotate-12 text-[28px] tracking-[0.3em] text-black/10">
      {draft.meta.watermark}
    </p>
  )
}

function HeadingList({ markdown, onJump }: { markdown: string; onJump: (index: number) => void }) {
  const { t } = useTranslation()
  const headings = planHeadings(markdown)
  if (headings.length === 0) return null
  return (
    <div className="border-b border-[#efefef]">
      <div className="flex items-center gap-1.5 px-3 pt-2 text-[12px] text-[#8a8a8a]">
        <ListTree className="size-3.5" strokeWidth={1.7} aria-hidden />
        <span>{t('workbench.plan.panel.toc')}</span>
      </div>
      <ol className="m-0 max-h-72 list-none overflow-y-auto px-2 py-1">
        {headings.map((heading, index) => (
          <li key={`${heading.text}-${index}`}>
            <button
              type="button"
              onClick={() => onJump(index)}
              className={`block w-full truncate rounded-md py-0.5 text-left text-[12px] leading-5 text-[#3f3f3f] hover:bg-[#f7f7f7] ${FOCUS}`}
              style={{ paddingLeft: 8 + (heading.level - 1) * 12 }}>
              {heading.text}
            </button>
          </li>
        ))}
      </ol>
    </div>
  )
}

function headingOffset(markdown: string, index: number): number {
  const rows = markdown.split('\n')
  let seen = 0
  let offset = 0
  for (const row of rows) {
    if (/^#{1,4}\s+/.test(row)) {
      if (seen === index) return offset
      seen += 1
    }
    offset += row.length + 1
  }
  return 0
}

function TocList({ markdown }: { markdown: string }) {
  const { t } = useTranslation()
  const headings = planHeadings(markdown)
  if (headings.length === 0) return <p className="m-0 text-[13px] text-[#8a8a8a]">{t('workbench.plan.toc.empty')}</p>
  return (
    <ol className="m-0 grid list-none gap-1 p-0">
      {headings.map((heading, index) => (
        <li
          key={`${heading.text}-${index}`}
          className="truncate text-[13px] text-[#171717]"
          style={{ paddingLeft: (heading.level - 1) * 12 }}>
          {heading.text}
        </li>
      ))}
    </ol>
  )
}

function CoverPage({ draft }: { draft: PlanDraft }) {
  const { t } = useTranslation()
  const cover = planCover(draft.coverId)
  const title = draft.meta.title || t('workbench.plan.untitled')
  const vertical = draft.coverId === 'academic'
  const centered = draft.coverId === 'mono' || draft.coverId === 'formal'
  const sideBar = draft.coverId === 'brief' || draft.coverId === 'tech'
  return (
    <article
      data-ui="workbench.plan.cover"
      data-cover={draft.coverId}
      data-layout={draft.layout}
      className={PAPER}
      style={{ background: cover.paper, color: cover.ink }}>
      {draft.coverId === 'formal' || draft.coverId === 'approval' ? (
        <div className="h-2 shrink-0" style={{ background: cover.accent }} />
      ) : null}
      {draft.coverId === 'gradient' ? (
        <div
          className="h-36 shrink-0"
          style={{ background: `linear-gradient(135deg, ${cover.accent}, ${cover.paper})` }}
        />
      ) : null}
      {sideBar ? <div className="absolute top-0 bottom-0 left-0 w-2" style={{ background: cover.accent }} /> : null}
      <WatermarkMark draft={draft} />
      <div className={`flex min-h-0 flex-1 flex-col px-14 py-14 ${sideBar ? 'pl-16' : ''}`}>
        {draft.meta.logo ? (
          <img src={draft.meta.logo} alt="" className="mb-6 h-10 w-10 rounded-md object-cover" />
        ) : null}
        {vertical && draft.meta.number ? (
          <p className="m-0 mt-6 text-center text-[13px]" style={{ color: cover.mute }}>
            {t('workbench.plan.cover.number', { number: draft.meta.number })}
          </p>
        ) : null}
        {!vertical ? (
          <p className={`m-0 text-[12px] ${centered ? 'text-center' : ''}`} style={{ color: cover.mute }}>
            {[draft.meta.number, draft.meta.version, draft.meta.date].filter(Boolean).join('  ·  ')}
          </p>
        ) : null}
        <h2
          data-ui="workbench.plan.cover.title"
          className={`m-0 font-medium ${vertical ? 'mt-8 self-center text-[40px]' : centered ? 'mt-16 text-center text-[34px] leading-10' : 'mt-8 text-[32px] leading-10'}`}
          style={vertical ? { writingMode: 'vertical-rl', letterSpacing: '0.16em', maxHeight: '58%' } : undefined}>
          {title}
        </h2>
        {!vertical && draft.meta.subtitle ? (
          <p
            className={`m-0 mt-3 text-[14px] leading-6 ${centered ? 'text-center' : ''}`}
            style={{ color: cover.mute }}>
            {draft.meta.subtitle}
          </p>
        ) : null}
        {!vertical ? (
          <div className={`mt-6 h-px w-16 ${centered ? 'mx-auto' : ''}`} style={{ background: cover.accent }} />
        ) : null}
        <div
          className="mt-auto grid w-full gap-3 pt-10"
          style={{ gridTemplateColumns: !vertical && draft.layout === 'double' ? '1fr 1fr' : '1fr' }}>
          {vertical ? (
            <>
              <FactLine label={t('workbench.plan.footer.department')} value={draft.meta.department} />
              <FactLine label={t('workbench.plan.footer.author')} value={draft.meta.author} />
              <FactLine label={t('workbench.plan.meta.organization')} value={draft.meta.organization} />
            </>
          ) : (
            <>
              <CoverFact
                label={t('workbench.plan.footer.department')}
                value={draft.meta.department}
                mute={cover.mute}
              />
              <CoverFact label={t('workbench.plan.footer.author')} value={draft.meta.author} mute={cover.mute} />
              {draft.layout === 'double' ? (
                <CoverFact
                  label={t('workbench.plan.meta.organization')}
                  value={draft.meta.organization}
                  mute={cover.mute}
                />
              ) : null}
            </>
          )}
        </div>
        {vertical && draft.meta.version ? (
          <p className="m-0 mt-8 text-center text-[12px]" style={{ color: cover.mute }}>
            {draft.meta.version}
          </p>
        ) : null}
      </div>
    </article>
  )
}

function FactLine({ label, value }: { label: string; value: string }) {
  if (!value.trim()) return null
  return (
    <p className="m-0 grid grid-cols-[128px_minmax(0,1fr)] items-end gap-3 text-[13px]">
      <span className="text-[#8a8a8a]">{label}</span>
      <span className="border-b border-[#171717] pb-1 text-center font-medium">{value}</span>
    </p>
  )
}

function CoverFact({ label, value, mute }: { label: string; value: string; mute: string }) {
  if (!value.trim()) return null
  return (
    <p className="m-0 border-t pt-2 text-[12px]" style={{ borderColor: mute, color: mute }}>
      <span className="mb-1 block">{label}</span>
      <span className="text-[13px] text-inherit">{value}</span>
    </p>
  )
}

function BodyPage({
  draft,
  blocks,
  page,
  pageCount,
  startHeading,
  startHeadingCounters,
  startImage,
  startTable,
  onHeading,
  onOverflow
}: {
  draft: PlanDraft
  blocks: PlanBlock[]
  page: number
  pageCount: number
  startHeading: number
  startHeadingCounters: PlanHeadingCounter
  startImage: number
  startTable: number
  onHeading: (index: number) => void
  onOverflow: (page: number, over: boolean) => void
}) {
  const bodyRef = useRef<HTMLDivElement>(null)
  const accent = planAccent(planCover(draft.coverId).accent, draft.style.ink)
  const size = draft.bodySize === 'sm' ? 'text-[12px]' : draft.bodySize === 'lg' ? 'text-[15px]' : 'text-[13px]'
  const font = bodyFont(draft.style.font)
  let headingIndex = startHeading
  let headings: PlanHeadingCounter = startHeadingCounters
  let imageSeen = startImage
  let tableSeen = startTable
  const blockStamp = blocks.map((block) => block.kind).join('')
  const styleStamp = `${draft.style.pagination}:${draft.style.table}:${draft.style.code}:${draft.style.imageCaption}:${draft.style.tableCaption}:${draft.style.imageFrame}:${draft.style.font}:${draft.style.leading}:${draft.style.indent}:${draft.style.numbering}:${draft.style.h1Form}:${draft.style.h2Form}:${draft.style.h1Center}:${draft.style.headerOn}:${draft.style.footerOn}`
  useEffect(() => {
    const node = bodyRef.current
    if (!node) return
    const measure = () => onOverflow(page, node.scrollHeight > node.clientHeight + 2)
    measure()
    if (typeof ResizeObserver !== 'function') return
    const observer = new ResizeObserver(measure)
    observer.observe(node)
    return () => observer.disconnect()
  }, [draft.markdown, draft.bodySize, page, onOverflow, blockStamp, styleStamp])
  return (
    <article className={PAPER}>
      <WatermarkMark draft={draft} />
      <SheetHeader draft={draft} accent={accent} />
      <div
        ref={bodyRef}
        className={`grid min-h-0 flex-1 content-start gap-3 overflow-hidden px-8 py-4 ${leadingClass(draft.style.leading)} ${size} ${font.className}`}
        style={font.style}>
        {blocks.map((block, index) => {
          const next = blocks[index + 1]
          if (block.kind === 'p' && next?.kind === 'table' && draft.style.tableCaption) return null
          const current = block.kind === 'h' ? headingIndex++ : undefined
          let headingLabel = ''
          if (block.kind === 'h') {
            headings = planBumpHeading(headings, block.level)
            headingLabel = planHeadingNumber(draft.style.numbering, headings, block.level)
          }
          let imageOrdinal = 0
          let tableLabel: string | null = null
          if (block.kind === 'image') {
            imageSeen += 1
            imageOrdinal = imageSeen
          }
          if (block.kind === 'table') {
            tableSeen += 1
            const previous = blocks[index - 1]
            if (draft.style.tableCaption && previous?.kind === 'p') {
              tableLabel = planCaptionLabel(draft.style.tablePrefix, tableSeen, previous.text, draft.style.tableNumber)
            }
          }
          return (
            <BlockView
              key={index}
              block={block}
              accent={accent}
              images={draft.images}
              style={draft.style}
              headingIndex={current}
              headingLabel={headingLabel}
              imageOrdinal={imageOrdinal}
              tableLabel={tableLabel}
              onHeading={onHeading}
            />
          )
        })}
      </div>
      <SheetFooter draft={draft} page={page} pageCount={pageCount} />
    </article>
  )
}

function SheetHeader({ draft, accent }: { draft: PlanDraft; accent: string }) {
  if (!draft.style.headerOn) return null
  const rule = headerRuleProps(draft.style.headerRule, accent)
  const logo = draft.meta.logo ? (
    <img src={draft.meta.logo} alt="" className="h-6 w-6 shrink-0 rounded-md object-cover" />
  ) : null
  const custom = draft.style.headerLeft.trim() || draft.style.headerRight.trim() || draft.meta.header.trim()
  if (!custom) {
    return (
      <div
        className={`m-0 flex shrink-0 items-center gap-2 px-8 py-3 text-[11px] text-[#8a8a8a] ${rule.className}`}
        style={rule.style}>
        {logo}
        <span className="min-w-0 truncate">{draft.meta.title}</span>
      </div>
    )
  }
  return (
    <div
      className={`m-0 grid shrink-0 grid-cols-3 items-center gap-2 px-8 py-3 text-[11px] text-[#8a8a8a] ${rule.className}`}
      style={rule.style}>
      <span className="flex min-w-0 items-center gap-2">
        {logo}
        <span className="truncate">{draft.style.headerLeft}</span>
      </span>
      <span className="truncate text-center">{draft.meta.header}</span>
      <span className="truncate text-right">{draft.style.headerRight}</span>
    </div>
  )
}

function SheetFooter({ draft, page, pageCount }: { draft: PlanDraft; page: number; pageCount: number }) {
  const { t } = useTranslation()
  if (!draft.style.footerOn) return null
  const label = pageText((key, values) => t(key, values), draft.style.pageFormat, page, pageCount)
  const slots: Record<'left' | 'center' | 'right', string> = {
    left: draft.style.footerLeft || draft.meta.number,
    center: draft.style.footerCenter,
    right: draft.style.footerRight
  }
  if (label) slots[draft.style.pagePlace] = label
  return (
    <p className="m-0 mt-auto grid shrink-0 grid-cols-3 items-center border-t border-[#efefef] px-8 py-2 text-[11px] text-[#8a8a8a]">
      <span className="truncate">{slots.left}</span>
      <span className="truncate text-center">{slots.center}</span>
      <span className="truncate text-right">{slots.right}</span>
    </p>
  )
}

function pageText(
  translate: (key: string, values?: { page: number; count: number }) => string,
  format: PlanStyle['pageFormat'],
  page: number,
  count: number
): string {
  if (format === 'none') return ''
  if (format === 'total') return translate('workbench.plan.chrome.num.total', { page, count })
  if (format === 'hyphen') return translate('workbench.plan.chrome.num.hyphen', { page, count })
  if (format === 'simple') return translate('workbench.plan.chrome.num.simple', { page, count })
  return translate('workbench.plan.sheet', { page, count })
}

function headerRuleProps(
  rule: PlanStyle['headerRule'],
  accent: string
): { className: string; style?: { borderColor: string } } {
  if (rule === 'none') return { className: '' }
  if (rule === 'accent') return { className: 'border-b-2', style: { borderColor: accent } }
  if (rule === 'double') return { className: 'border-b-4 border-double border-[#171717]' }
  return { className: 'border-b border-[#efefef]' }
}

function bodyFont(font: PlanStyle['font']): { className: string; style?: { fontFamily: string } } {
  if (font === 'mono') return { className: 'font-mono' }
  if (font === 'serif') return { className: '', style: { fontFamily: '"Songti SC","STSong",Georgia,serif' } }
  if (font === 'kai') return { className: '', style: { fontFamily: '"Kaiti SC","STKaiti","KaiTi",serif' } }
  if (font === 'hei') return { className: '', style: { fontFamily: '"Heiti SC","STHeiti","SimHei",sans-serif' } }
  return { className: '' }
}

function leadingClass(leading: PlanStyle['leading']): string {
  if (leading === 'tight') return 'leading-5'
  if (leading === 'loose') return 'leading-8'
  return 'leading-6'
}

function TocSheet({
  draft,
  entries,
  page,
  pageCount,
  pageOffset
}: {
  draft: PlanDraft
  entries: { level: number; text: string; sheet: number }[]
  page: number
  pageCount: number
  pageOffset: number
}) {
  const { t } = useTranslation()
  const accent = planAccent(planCover(draft.coverId).accent, draft.style.ink)
  const title = draft.style.tocTitle || t('workbench.plan.toc.fallback')
  const centered = draft.style.tocCenter
  return (
    <article data-ui="workbench.plan.toc" className={PAPER}>
      <WatermarkMark draft={draft} />
      <SheetHeader draft={draft} accent={accent} />
      <div className="grid min-h-0 flex-1 content-start gap-2 overflow-hidden px-8 py-6">
        <h2
          className={`min-w-0 truncate ${tocTitleClass(draft.style.tocForm, centered)}`}
          style={tocTitleStyle(draft.style.tocForm, accent)}>
          {title}
        </h2>
        {entries.length === 0 ? (
          <p className="m-0 text-[13px] text-[#8a8a8a]">{t('workbench.plan.toc.empty')}</p>
        ) : (
          entries.map((entry, index) => (
            <div
              key={`${entry.sheet}-${entry.level}-${index}`}
              className="flex min-w-0 items-end gap-2 text-[13px] text-[#171717]"
              style={{ paddingLeft: (entry.level - 1) * 12 }}>
              <span className="min-w-0 truncate">{entry.text}</span>
              <span className={leaderClass(draft.style.tocLeader)} />
              {draft.style.tocPages ? (
                <span className="shrink-0 text-[#8a8a8a] tabular-nums">{pageOffset + entry.sheet + 1}</span>
              ) : null}
            </div>
          ))
        )}
      </div>
      <SheetFooter draft={draft} page={page} pageCount={pageCount} />
    </article>
  )
}

function tocTitleClass(form: PlanStyle['tocForm'], center: boolean): string {
  const align = center ? 'text-center' : ''
  if (form === 'badge')
    return `m-0 w-fit rounded-md px-2 py-0.5 text-[20px] font-medium ${center ? 'mx-auto' : ''} ${align}`
  if (form === 'minimal') return `m-0 text-[20px] font-medium ${align}`
  if (form === 'accent') return `m-0 border-l-4 pl-2 text-[20px] font-medium ${align}`
  return `m-0 border-b border-[#efefef] pb-1 text-[20px] font-medium ${align}`
}

function tocTitleStyle(
  form: PlanStyle['tocForm'],
  accent: string
): { borderColor?: string; background?: string; color?: string } | undefined {
  if (form === 'accent') return { borderColor: accent }
  if (form === 'badge') return { background: accent + '14', color: accent }
  return undefined
}

function leaderClass(leader: PlanStyle['tocLeader']): string {
  if (leader === 'none') return 'min-w-2 flex-1'
  const kind = leader === 'dashes' ? 'border-dashed' : leader === 'line' ? 'border-solid' : 'border-dotted'
  return `mb-1 min-w-4 flex-1 border-b ${kind} border-[#d4d4d4]`
}

function BlockView({
  block,
  accent,
  images,
  style,
  headingIndex,
  headingLabel,
  imageOrdinal,
  tableLabel,
  onHeading
}: {
  block: PlanBlock
  accent: string
  images: Record<string, string>
  style: PlanStyle
  headingIndex?: number
  headingLabel: string
  imageOrdinal: number
  tableLabel: string | null
  onHeading: (index: number) => void
}) {
  if (block.kind === 'h') {
    const look = headingLook(block.level, style, accent)
    const centered = block.level === 1 && style.h1Center
    return (
      <h3
        className={`m-0 ${look.className} ${centered && style.h1Form === 'badge' ? 'mx-auto' : ''}`}
        style={look.style}
        data-plan-heading={headingIndex}>
        <button
          type="button"
          className={`w-full ${centered ? 'text-center' : 'text-left'} ${FOCUS}`}
          onClick={() => onHeading(headingIndex ?? 0)}>
          {headingLabel ? <span>{headingLabel + ' '}</span> : null}
          {inline(block.text)}
        </button>
      </h3>
    )
  }
  if (block.kind === 'p')
    return <p className={`m-0 whitespace-pre-wrap ${style.indent ? 'indent-[2em]' : ''}`}>{inline(block.text)}</p>
  if (block.kind === 'quote') {
    return (
      <blockquote className="m-0 border-l-2 pl-3 text-[#3f3f3f]" style={{ borderColor: accent }}>
        {inline(block.text)}
      </blockquote>
    )
  }
  if (block.kind === 'ul' || block.kind === 'ol') {
    const Tag = block.kind === 'ul' ? 'ul' : 'ol'
    return (
      <Tag className="m-0 grid list-none gap-1 p-0">
        {block.items.map((item, index) => (
          <li key={index} className="grid grid-cols-[1.25rem_minmax(0,1fr)] gap-1">
            <span className="text-center text-[#3f3f3f]">{planListMark(block.kind, style, index)}</span>
            <span>{inline(item)}</span>
          </li>
        ))}
      </Tag>
    )
  }
  if (block.kind === 'code') {
    if (block.lang === 'mermaid') return <MermaidBlock source={block.text} theme={style.mermaid} />
    if (block.lang === 'math' || block.lang === 'latex' || block.lang === 'tex') {
      return <MathBlock source={block.text} display />
    }
    const dark = style.code === 'dark'
    return (
      <pre
        className={`m-0 overflow-x-auto rounded-lg p-3 text-[12px] leading-5 ${dark ? 'bg-[#171717] text-[#f5f5f5]' : 'bg-[#f5f5f5]'}`}>
        {block.text}
      </pre>
    )
  }
  if (block.kind === 'math') return <MathBlock source={block.text} display />
  if (block.kind === 'image') {
    const src = resolvePlanImage(block.ref, images)
    if (!src) return <p className="m-0 text-[#8a8a8a]">{block.alt}</p>
    const caption = style.imageCaption
      ? planCaptionLabel(style.imagePrefix, imageOrdinal, block.alt, style.imageNumber)
      : ''
    return (
      <figure className={`m-0 flex flex-col gap-1 ${alignClass(style.imageAlign)}`}>
        <span className={`block w-fit max-w-full ${frameClass(style.imageFrame)}`}>
          <img
            alt={block.alt}
            src={src}
            className={`max-h-64 max-w-full object-contain ${style.imageFrame === 'round' ? 'rounded-2xl' : 'rounded-md'}`}
          />
        </span>
        {caption ? <figcaption className="text-[12px] text-[#8a8a8a]">{caption}</figcaption> : null}
      </figure>
    )
  }
  if (block.kind === 'hr') return <hr className="m-0 border-[#efefef]" />
  if (block.kind === 'table') {
    const bordered = style.table === 'bordered'
    const striped = style.table === 'striped'
    const caption = tableLabel ? (
      <p className={`m-0 text-[12px] text-[#8a8a8a] ${alignText(style.tableAlign)}`}>{tableLabel}</p>
    ) : null
    return (
      <div className="grid gap-1">
        {style.tablePlace === 'above' ? caption : null}
        <table className={`w-full border-collapse text-left text-[12px] ${bordered ? 'border border-[#e5e5e5]' : ''}`}>
          <tbody>
            {block.rows.map((row, index) => (
              <tr
                key={index}
                className={
                  index === 0
                    ? bordered
                      ? 'bg-[#f7f7f7] font-medium'
                      : 'border-b border-[#efefef] bg-[#f7f7f7] font-medium'
                    : striped && index % 2 === 0
                      ? 'bg-[#f7f7f7]'
                      : bordered
                        ? ''
                        : 'border-b border-[#efefef]'
                }>
                {row.map((cell, cellIndex) => (
                  <td key={cellIndex} className={bordered ? 'border border-[#e5e5e5] px-1 py-1' : 'px-1 py-1'}>
                    {inline(cell)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        {style.tablePlace === 'below' ? caption : null}
      </div>
    )
  }
  return null
}

function headingLook(
  level: 1 | 2 | 3 | 4,
  style: PlanStyle,
  accent: string
): { className: string; style?: { borderColor: string; background?: string; color?: string } } {
  if (level === 1 && style.h1Form === 'accent') {
    return { className: 'border-l-4 pl-2 text-[20px] font-medium', style: { borderColor: accent } }
  }
  if (level === 1 && style.h1Form === 'badge') {
    return {
      className: 'w-fit rounded-md px-2 py-0.5 text-[20px] font-medium',
      style: { borderColor: accent, background: accent + '14', color: accent }
    }
  }
  if (level === 1 && style.h1Form === 'minimal') return { className: 'text-[20px] font-medium' }
  if (level === 1) return { className: 'border-b border-[#efefef] pb-1 text-[20px] font-medium' }
  if (level === 2 && style.h2Form === 'bar') {
    return { className: 'border-l-2 pl-2 text-[16px] font-medium', style: { borderColor: accent } }
  }
  if (level === 2 && style.h2Form === 'line')
    return { className: 'border-b border-[#efefef] pb-1 text-[16px] font-medium' }
  if (level === 2) return { className: 'text-[16px] font-medium' }
  if (level === 3) return { className: 'text-[14px] font-medium' }
  return { className: 'text-[13px] font-medium' }
}

function alignClass(align: PlanStyle['imageAlign']): string {
  if (align === 'left') return 'items-start text-left'
  if (align === 'right') return 'items-end text-right'
  return 'items-center text-center'
}

function alignText(align: PlanStyle['tableAlign']): string {
  if (align === 'left') return 'text-left'
  if (align === 'right') return 'text-right'
  return 'text-center'
}

function frameClass(frame: PlanStyle['imageFrame']): string {
  if (frame === 'shadow') return 'shadow-[0_4px_16px_rgba(0,0,0,0.08)]'
  if (frame === 'line') return 'rounded-md border border-[#e5e5e5]'
  if (frame === 'deep') return 'shadow-[0_12px_32px_rgba(0,0,0,0.18)]'
  if (frame === 'card') return 'rounded-lg bg-[#f5f5f5] p-2'
  if (frame === 'round') return 'overflow-hidden rounded-2xl'
  return ''
}

function inline(text: string): ReactNode[] {
  return text
    .split(/(\*\*[^*]+\*\*|`[^`]+`|\*[^*]+\*|\$[^$\n]+\$|<sup>[^<]*<\/sup>|<sub>[^<]*<\/sub>)/g)
    .map((part, index) => {
      if (part.startsWith('**') && part.endsWith('**') && part.length > 4)
        return <strong key={index}>{part.slice(2, -2)}</strong>
      if (part.startsWith('`') && part.endsWith('`') && part.length > 2)
        return <code key={index}>{part.slice(1, -1)}</code>
      if (part.startsWith('<sup>') && part.endsWith('</sup>')) return <sup key={index}>{part.slice(5, -6)}</sup>
      if (part.startsWith('<sub>') && part.endsWith('</sub>')) return <sub key={index}>{part.slice(5, -6)}</sub>
      if (part.startsWith('*') && part.endsWith('*') && part.length > 2) return <em key={index}>{part.slice(1, -1)}</em>
      if (isInlineMath(part)) return <MathBlock key={index} source={part.slice(1, -1)} display={false} />
      return <span key={index}>{part}</span>
    })
}

function isInlineMath(part: string): boolean {
  if (!(part.startsWith('$') && part.endsWith('$') && part.length > 2)) return false
  const body = part.slice(1, -1)
  if (!body || body.startsWith(' ') || body.endsWith(' ')) return false
  return !/^\d+(?:\.\d+)?$/.test(body)
}

function MathBlock({ source, display }: { source: string; display: boolean }) {
  const host = useRef<HTMLSpanElement>(null)
  useEffect(() => {
    const node = host.current
    if (!node) return
    let cancel = false
    void import('katex')
      .then((mod) => {
        if (cancel) return
        const html = mod.default.renderToString(source, { displayMode: display, throwOnError: false })
        const parsed = new DOMParser().parseFromString(html, 'text/html')
        node.replaceChildren(...Array.from(parsed.body.childNodes).map((child) => document.importNode(child, true)))
      })
      .catch(() => {
        if (!cancel) node.textContent = source
      })
    return () => {
      cancel = true
    }
  }, [source, display])
  return <span ref={host} />
}

let mermaidTheme: PlanStyle['mermaid'] | null = null

function MermaidBlock({ source, theme }: { source: string; theme: PlanStyle['mermaid'] }) {
  const host = useRef<HTMLDivElement>(null)
  const [failed, setFailed] = useState(false)
  useEffect(() => {
    const node = host.current
    if (!node) return
    let cancel = false
    setFailed((current) => (current ? false : current))
    const id = `planMermaid${Math.random().toString(36).slice(2)}`
    void import('mermaid')
      .then(async (mod) => {
        const mermaid = mod.default
        if (mermaidTheme !== theme) {
          mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme })
          mermaidTheme = theme
        }
        const rendered = await mermaid.render(id, source)
        if (cancel) return
        const parsed = new DOMParser().parseFromString(rendered.svg, 'image/svg+xml')
        const svg = parsed.querySelector('svg')
        if (!svg || parsed.querySelector('parsererror')) {
          setFailed(true)
          return
        }
        node.replaceChildren(document.importNode(svg, true))
      })
      .catch(() => {
        if (!cancel) setFailed(true)
      })
    return () => {
      cancel = true
    }
  }, [source, theme])
  return (
    <div className="grid gap-1">
      {failed ? (
        <pre className="m-0 overflow-x-auto rounded-lg bg-[#f5f5f5] p-3 text-[12px] leading-5">{source}</pre>
      ) : null}
      <div ref={host} className={failed ? 'hidden' : 'overflow-x-auto'} />
    </div>
  )
}
