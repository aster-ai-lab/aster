import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  EmptyState,
  PopConfirm,
  SearchInput,
  SegmentedControl
} from '@a3s-lab/ui'
import {
  Blocks,
  Briefcase,
  CodeXml,
  Database,
  Download,
  FlaskConical,
  GraduationCap,
  Home,
  LayoutGrid,
  PenTool,
  Plus,
  RefreshCw,
  RotateCw,
  Search,
  Sparkles,
  Settings,
  Star,
  Store,
  Trash2,
  Upload,
  type LucideIcon
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { PageEntry } from '@renderer/components/layout/PageEntry'
import Scrollbar from '@renderer/components/Scrollbar'
import { MemberOrgError } from '@renderer/data/memberOrgs'
import { PluginConfigDialog } from '@renderer/pages/plugins/PluginConfigDialog'
import { pluginShipsOkf } from '@renderer/pages/plugins/pluginOkf'
import { PluginOkfConfig } from '@renderer/pages/plugins/PluginOkfConfig'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  CAPABILITY_PAGE_SIZE,
  capabilityCategoryLabelKey,
  installCapability,
  listCapabilities,
  prepareBuiltinSkill,
  uninstallCapability,
  type BuiltinSkillName,
  type CapabilityEntry,
  type CapabilityKind,
  type CapabilityPage,
  type CapabilityQuery
} from './capabilitiesData'
import { CapabilityMark } from './capabilityMark'
import { ConnectorItemDialog } from './ConnectorItemDialog'
import { ConnectorManageDialog } from './ConnectorManageDialog'
import { SkillUploadDialog } from './SkillUploadDialog'
import { SkillUseDialog } from './SkillUseDialog'

const SEARCH_DEBOUNCE_MS = 300

type CapabilityFilter = 'all' | 'featured' | (string & {})
type CatalogView = 'market' | 'installed'

const VIEW_OPTIONS = [
  { value: 'market', labelKey: 'plugins.tab.market' },
  { value: 'installed', labelKey: 'plugins.tab.installed' }
] as const satisfies ReadonlyArray<{ value: CatalogView; labelKey: string }>

const CATEGORY_ICONS: Record<string, LucideIcon> = {
  all: Home,
  featured: Star,
  office: Briefcase,
  creative: Sparkles,
  design: PenTool,
  code: CodeXml,
  business: Store,
  productivity: LayoutGrid,
  science: FlaskConical,
  education: GraduationCap,
  search: Search,
  data: Database,
  other: Blocks
}

function CreateIcon() {
  return <Plus size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
}

function SkillMenuItem({ label, icon: Icon, onSelect }: { label: string; icon: LucideIcon; onSelect: () => void }) {
  return (
    <DropdownMenuItem
      onSelect={onSelect}
      className="h-9 cursor-default gap-2.5 rounded-[8px] px-2.5 text-[13px] text-[#3f3f3f] outline-none focus:bg-[#f7f7f7] focus:text-[#171717] data-[highlighted]:bg-[#f7f7f7] data-[highlighted]:text-[#171717]">
      <Icon className="size-4 text-current" strokeWidth={1.7} aria-hidden />
      {label}
    </DropdownMenuItem>
  )
}

type CatalogSnapshot = {
  stamp: string
  page: CapabilityPage
}

function requestStamp(kind: CapabilityKind, q: string, filter: string, view: string, page: number): string {
  return [kind, q, filter, view, String(page)].join('\0')
}

/** Previous page kept on screen while the next request is in flight. The settled response is trusted as-is. */
function matchesVisibleRequest(
  entry: CapabilityEntry,
  kind: CapabilityKind,
  filter: CapabilityFilter,
  view: CatalogView,
  q: string
): boolean {
  if (entry.kind !== kind) return false
  if (filter === 'featured' && entry.featured !== true) return false
  if (filter !== 'all' && filter !== 'featured' && entry.category !== filter) return false
  if (view === 'installed' && !entry.installed) return false
  const needle = q.trim().toLocaleLowerCase()
  if (!needle) return true
  return `${entry.name}\n${entry.description}\n${entry.slug}`.toLocaleLowerCase().includes(needle)
}

type EmptyAction = 'create' | 'market' | 'all' | 'clear-search'

function emptyCopy(
  kind: CapabilityKind,
  filter: CapabilityFilter,
  view: CatalogView,
  q: string
): { titleKey: string; descriptionKey: string; actionKey: string; action: EmptyAction } {
  if (q.trim()) {
    return {
      titleKey: 'capabilities.no_results',
      descriptionKey: 'capabilities.no_results_hint',
      actionKey: 'capabilities.empty.clear_search',
      action: 'clear-search'
    }
  }
  if (view === 'installed') {
    return {
      titleKey: 'capabilities.empty.installed',
      descriptionKey: 'capabilities.empty.installed_hint',
      actionKey: 'capabilities.empty.browse',
      action: 'market'
    }
  }
  if (filter === 'featured') {
    return {
      titleKey: 'capabilities.empty.featured',
      descriptionKey: 'capabilities.empty.featured_hint',
      actionKey: 'capabilities.empty.show_all',
      action: 'all'
    }
  }
  if (filter !== 'all') {
    return {
      titleKey: 'capabilities.empty.category',
      descriptionKey: 'capabilities.empty.category_hint',
      actionKey: 'capabilities.empty.show_all',
      action: 'all'
    }
  }
  return kind === 'skill'
    ? {
        titleKey: 'capabilities.empty.skills',
        descriptionKey: 'capabilities.empty.skills_hint',
        actionKey: 'capabilities.create',
        action: 'create'
      }
    : {
        titleKey: 'capabilities.empty.connectors',
        descriptionKey: 'capabilities.empty.connectors_hint',
        actionKey: 'capabilities.create',
        action: 'create'
      }
}

function CapabilityEmpty({
  title,
  description,
  actionLabel,
  onAction
}: {
  title: string
  description: string
  actionLabel: string
  onAction: () => void
}) {
  return (
    <div data-ui="capabilities.empty" role="status" className="flex flex-1 flex-col">
      <EmptyState
        illustration="inbox"
        title={title}
        description={description}
        actionLabel={actionLabel}
        onAction={onAction}
        className="py-16"
      />
    </div>
  )
}

function catalogQuery(
  kind: CapabilityKind,
  filter: CapabilityFilter,
  q: string,
  page: number,
  view: CatalogView
): CapabilityQuery {
  return {
    kind,
    q,
    page,
    pageSize: CAPABILITY_PAGE_SIZE,
    featured: filter === 'featured' ? true : undefined,
    category: filter !== 'all' && filter !== 'featured' ? filter : undefined,
    installed: view === 'installed' ? true : undefined
  }
}

export default function CapabilitiesPage({
  kind = 'connector',
  onUseSkill
}: {
  kind?: CapabilityKind
  onUseSkill?: (request: { name: string; prompt: string }) => void
}) {
  const { t } = useTranslation()
  const [view, setView] = useState<CatalogView>('market')
  const [query, setQuery] = useState('')
  const [committedQuery, setCommittedQuery] = useState('')
  const [composing, setComposing] = useState(false)
  const committedQueryRef = useRef('')
  const [activeFilter, setActiveFilter] = useState<CapabilityFilter>('all')
  const [page, setPage] = useState(1)
  const [pendingSlug, setPendingSlug] = useState<string | null>(null)
  const [download, setDownload] = useState<{ slug: string; received: number; total: number } | null>(null)
  const [failedSlug, setFailedSlug] = useState<string | null>(null)
  const pendingSlugRef = useRef<string | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const [manageOpen, setManageOpen] = useState(false)
  const [skillMenuOpen, setSkillMenuOpen] = useState(false)
  const [uploadOpen, setUploadOpen] = useState(false)
  const [selected, setSelected] = useState<CapabilityEntry | null>(null)
  const [okfPlugin, setOkfPlugin] = useState<CapabilityEntry | null>(null)
  const [configPlugin, setConfigPlugin] = useState<CapabilityEntry | null>(null)
  const refreshingRef = useRef(false)
  const scrollRef = useRef<HTMLDivElement>(null)

  const publishQuery = useCallback((raw: string) => {
    const next = raw.trim()
    if (committedQueryRef.current === next) return
    committedQueryRef.current = next
    setCommittedQuery(next)
    setPage(1)
  }, [])

  useEffect(() => {
    if (composing) return
    const timer = window.setTimeout(() => publishQuery(query), SEARCH_DEBOUNCE_MS)
    return () => window.clearTimeout(timer)
  }, [query, composing, publishQuery])

  const requestKey = ['studio/capabilities', kind, committedQuery, activeFilter, view, page] as const
  const { data, error, isValidating, mutate } = useSWR(
    requestKey,
    ([, nextKind, q, filter, nextView, nextPage]) =>
      listCapabilities(catalogQuery(nextKind, filter, q, nextPage, nextView)).then(
        (result): CatalogSnapshot => ({
          stamp: requestStamp(nextKind, q, filter, nextView, nextPage),
          page: result
        })
      ),
    { keepPreviousData: true }
  )
  const catalog = data?.page
  const settled = data?.stamp === requestStamp(kind, committedQuery, activeFilter, view, page)

  useEffect(() => {
    scrollRef.current?.scrollTo?.({ top: 0 })
  }, [kind, activeFilter, committedQuery, view, page])

  useEffect(() => {
    if (activeFilter === 'all' || activeFilter === 'featured') return
    const categories = catalog?.categories
    if (!categories) return
    if (!categories.some((category) => category.id === activeFilter)) setActiveFilter('all')
  }, [catalog, activeFilter])

  const toggleInstall = async (entry: CapabilityEntry): Promise<boolean> => {
    if (entry.builtin) return false
    if (pendingSlugRef.current) return false
    pendingSlugRef.current = entry.slug
    setPendingSlug(entry.slug)
    setFailedSlug((current) => (current === entry.slug ? null : current))
    setDownload((current) => (current?.slug === entry.slug ? null : current))
    try {
      if (entry.installed) await uninstallCapability(entry.slug)
      else {
        await installCapability(entry.slug, globalThis.fetch, (progress) => {
          setDownload({ slug: entry.slug, received: progress.received, total: progress.total })
        })
      }
      const installed = !entry.installed
      // Install already reconciles on the shell's install route; uninstall does
      // not. Re-read the catalog after an uninstall — the read passes through
      // the shell, whose reconcile drops the removed skill directories and
      // mcp.json entries instead of letting them linger until the next fetch.
      await mutate(
        (current) =>
          current && {
            ...current,
            page: {
              ...current.page,
              items: current.page.items.map((row) => (row.slug === entry.slug ? { ...row, installed } : row))
            }
          },
        { revalidate: entry.installed }
      )
      setSelected((current) => (current && current.slug === entry.slug ? { ...current, installed } : current))
      setFailedSlug((current) => (current === entry.slug ? null : current))
      return true
    } catch (failure) {
      if (!entry.installed && failure instanceof MemberOrgError && failure.status === 403) {
        toast.error(t('capabilities.install_team_refused', { name: entry.name }))
        return false
      }
      toast.error(
        t(entry.installed ? 'capabilities.uninstall_failed' : 'capabilities.install_failed', {
          name: entry.name
        }) + (failure instanceof Error && failure.message ? ` · ${failure.message}` : '')
      )
      if (!entry.installed && (entry.kind === 'skill' || entry.kind === 'plugin')) setFailedSlug(entry.slug)
      return false
    } finally {
      pendingSlugRef.current = null
      setPendingSlug(null)
      setDownload((current) => (current?.slug === entry.slug ? null : current))
    }
  }

  const selectFilter = (next: CapabilityFilter) => {
    setActiveFilter(next)
    setPage(1)
  }

  const selectView = (next: CatalogView) => {
    setView(next)
    setPage(1)
  }

  const refresh = async () => {
    if (refreshingRef.current) return
    refreshingRef.current = true
    setRefreshing(true)
    setComposing(false)
    setQuery('')
    committedQueryRef.current = ''
    setCommittedQuery('')
    setActiveFilter('all')
    setView('market')
    setPage(1)
    scrollRef.current?.scrollTo?.({ top: 0 })
    try {
      await mutate()
    } finally {
      refreshingRef.current = false
      setRefreshing(false)
    }
  }

  const launchBuiltin = async (name: BuiltinSkillName) => {
    try {
      await prepareBuiltinSkill(name)
    } catch {
      toast.error(t('capabilities.skill.builtin_failed'))
    }
    onUseSkill?.({ name, prompt: '' })
  }

  const useSkillPrompt = async (entry: CapabilityEntry, prompt: string) => {
    if (!entry.installed) {
      const installed = await toggleInstall(entry)
      if (!installed) return
    }
    onUseSkill?.({ name: entry.name, prompt })
    setSelected(null)
  }

  const entries = catalog?.items ?? []
  const visible = settled
    ? entries
    : entries.filter((entry) => matchesVisibleRequest(entry, kind, activeFilter, view, committedQuery))
  const showError = Boolean(error) && !settled && !isValidating
  const showEmpty = settled && visible.length === 0
  const empty = emptyCopy(kind, activeFilter, view, committedQuery)
  const runEmptyAction = () => {
    if (empty.action === 'clear-search') {
      setComposing(false)
      setQuery('')
      publishQuery('')
      return
    }
    if (empty.action === 'market') {
      selectView('market')
      return
    }
    if (empty.action === 'all') {
      selectFilter('all')
      return
    }
    if (kind === 'skill') setSkillMenuOpen(true)
    else setManageOpen(true)
  }
  const pageCount = Math.max(1, Math.ceil((catalog?.total ?? 0) / (catalog?.pageSize || CAPABILITY_PAGE_SIZE)))
  const categories = [
    { id: 'all' as const, label: t('capabilities.category.all') },
    { id: 'featured' as const, label: t('capabilities.category.featured') },
    ...(catalog?.categories ?? []).map((category) => {
      const labelKey = capabilityCategoryLabelKey(category.id)
      return { id: category.id, label: labelKey ? t(labelKey) : category.id }
    })
  ]
  const heading = categories.find((category) => category.id === activeFilter)?.label ?? t('capabilities.category.all')

  if (okfPlugin) {
    return <PluginOkfConfig slug={okfPlugin.slug} name={okfPlugin.name} onBack={() => setOkfPlugin(null)} />
  }

  return (
    <div
      data-ui="capabilities.page"
      className="@container/capabilities relative flex h-full min-h-0 min-w-0 flex-col bg-white">
      <div className="shrink-0 bg-white">
        <div className="flex flex-col gap-4 px-6 pt-4 pb-4">
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-4">
              <h1 className="m-0 min-w-0 truncate text-[24px] leading-tight font-bold text-foreground">
                {t(kind === 'connector' ? 'capabilities.tab.connectors' : 'capabilities.tab.skills')}
              </h1>
              <div className="flex shrink-0 items-center gap-2">
                <SearchInput
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  onCompositionStart={() => setComposing(true)}
                  onCompositionEnd={() => setComposing(false)}
                  onKeyDown={(event) => {
                    if (event.key !== 'Enter' || event.nativeEvent.isComposing || composing) return
                    publishQuery(event.currentTarget.value)
                  }}
                  onClear={() => {
                    setComposing(false)
                    setQuery('')
                    publishQuery('')
                  }}
                  clearLabel={t('common.clear')}
                  placeholder={t(
                    kind === 'connector' ? 'capabilities.search.connectors' : 'capabilities.search.skills'
                  )}
                  containerClassName="h-[30px] w-[220px] border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0 dark:bg-[#2c2c2c]"
                />
                <SegmentedControl
                  aria-label={t(kind === 'connector' ? 'capabilities.tab.connectors' : 'capabilities.tab.skills')}
                  size="sm"
                  className="h-[30px]"
                  value={view}
                  options={VIEW_OPTIONS.map(({ value, labelKey }) => ({ value, label: t(labelKey) }))}
                  onValueChange={selectView}
                />
                <button
                  type="button"
                  aria-label={t('common.refresh')}
                  data-ui="capabilities.refresh"
                  aria-busy={refreshing || undefined}
                  onClick={() => void refresh()}
                  className="grid size-[30px] place-items-center rounded-[8px] text-[#171717] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10">
                  <RefreshCw
                    className={cn('size-4 text-current', refreshing && 'animate-spin')}
                    style={refreshing ? { animationDuration: '0.7s' } : undefined}
                    strokeWidth={1.7}
                    aria-hidden
                  />
                </button>
                {kind === 'connector' ? (
                  <button
                    type="button"
                    data-ui="capabilities.create"
                    onClick={() => setManageOpen(true)}
                    className="inline-flex h-[30px] items-center justify-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] leading-none text-white outline-none hover:bg-[#2a2a2a] focus-visible:ring-2 focus-visible:ring-black/10">
                    <CreateIcon />
                    {t('capabilities.create')}
                  </button>
                ) : (
                  <DropdownMenu open={skillMenuOpen} onOpenChange={setSkillMenuOpen}>
                    <DropdownMenuTrigger asChild>
                      <button
                        type="button"
                        data-ui="capabilities.create"
                        className="inline-flex h-[30px] items-center justify-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] leading-none text-white outline-none hover:bg-[#2a2a2a] focus-visible:ring-2 focus-visible:ring-black/10">
                        <CreateIcon />
                        {t('capabilities.create')}
                      </button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent
                      align="end"
                      sideOffset={6}
                      aria-label={t('capabilities.create')}
                      data-ui="capabilities.create.menu"
                      className="min-w-[148px] rounded-[12px] border border-[#ececec] bg-white p-1 text-[#3f3f3f] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
                      <SkillMenuItem
                        label={t('capabilities.skill.menu.upload')}
                        icon={Upload}
                        onSelect={() => {
                          setSkillMenuOpen(false)
                          // Opening the dialog in this same select event lets its focus trap cancel the menu close.
                          window.setTimeout(() => setUploadOpen(true), 0)
                        }}
                      />
                      <SkillMenuItem
                        label={t('capabilities.skill.menu.create')}
                        icon={Sparkles}
                        onSelect={() => {
                          setSkillMenuOpen(false)
                          void launchBuiltin('skill-creator')
                        }}
                      />
                    </DropdownMenuContent>
                  </DropdownMenu>
                )}
              </div>
            </div>
            <p className="m-0 text-[15px] text-muted-foreground">
              {t(kind === 'connector' ? 'capabilities.subtitle.connectors' : 'capabilities.subtitle.skills')}
            </p>
          </div>
        </div>
      </div>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col @[640px]/capabilities:flex-row">
        <nav
          aria-label={t(kind === 'connector' ? 'capabilities.tab.connectors' : 'capabilities.tab.skills')}
          data-ui="capabilities.categories"
          className={cn(
            'flex min-h-0 min-w-0 gap-0.5 overflow-x-auto bg-white px-3 py-2',
            '@[640px]/capabilities:w-[260px] @[640px]/capabilities:min-w-[260px] @[640px]/capabilities:shrink-0 @[640px]/capabilities:flex-col @[640px]/capabilities:overflow-x-hidden @[640px]/capabilities:overflow-y-auto @[640px]/capabilities:border-r @[640px]/capabilities:border-[#efefef] @[640px]/capabilities:pt-2 @[640px]/capabilities:pb-4'
          )}>
          {categories.map((category) => {
            const Icon = CATEGORY_ICONS[category.id] ?? LayoutGrid
            const selected = category.id === activeFilter
            return (
              <button
                key={category.id}
                type="button"
                aria-current={selected ? 'page' : undefined}
                onClick={() => selectFilter(category.id)}
                className={cn(
                  'flex h-9 shrink-0 items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] transition-colors outline-none',
                  'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
                  selected
                    ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                    : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
                )}>
                <Icon className="size-4 shrink-0 text-current" strokeWidth={1.7} />
                <span className="truncate">{category.label}</span>
              </button>
            )
          })}
        </nav>

        <Scrollbar ref={scrollRef} className="@container/catalog-grid min-h-0 min-w-0 flex-1">
          <div className="flex min-h-full flex-col px-6 pt-4 pb-6">
            <section className="mt-1 flex flex-1 flex-col">
              <h2 className="m-0 text-[18px] font-semibold text-[#1a1a1a]">{heading}</h2>
              {showError ? (
                <div data-ui="capabilities.error" className="flex flex-col items-center gap-3 py-16 text-center">
                  <p className="m-0 text-sm text-[#8a8a8a]">
                    {t(
                      error instanceof MemberOrgError && error.status === 401
                        ? 'capabilities.sign_in_required'
                        : 'capabilities.load_failed'
                    )}
                  </p>
                  <button
                    type="button"
                    onClick={() => void mutate()}
                    className="inline-flex h-[30px] items-center justify-center rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] leading-none text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                    {t('common.retry')}
                  </button>
                </div>
              ) : showEmpty ? (
                <CapabilityEmpty
                  title={t(empty.titleKey)}
                  description={t(empty.descriptionKey)}
                  actionLabel={t(empty.actionKey)}
                  onAction={runEmptyAction}
                />
              ) : visible.length === 0 ? (
                <p className="m-0 py-16 text-center text-sm text-[#8a8a8a]">{t('common.loading')}</p>
              ) : (
                <div className="mt-4 flex flex-col gap-4">
                  <div
                    data-ui="capabilities.grid"
                    className="grid w-full grid-cols-1 gap-3 @[720px]/catalog-grid:grid-cols-2">
                    {visible.map((entry) => (
                      <CapabilityCard
                        key={entry.slug}
                        entry={entry}
                        pending={pendingSlug !== null}
                        busy={pendingSlug === entry.slug}
                        progress={download?.slug === entry.slug ? download : null}
                        failed={failedSlug === entry.slug}
                        onOpen={setSelected}
                        onConfigure={setOkfPlugin}
                        onConfigureFields={kind === 'connector' ? undefined : setConfigPlugin}
                        onToggleInstall={(row) => void toggleInstall(row)}
                      />
                    ))}
                  </div>
                  {settled && catalog && catalog.total > catalog.pageSize ? (
                    <div className="flex items-center justify-center gap-3">
                      <button
                        type="button"
                        disabled={page <= 1}
                        onClick={() => setPage((current) => Math.max(1, current - 1))}
                        className="h-[30px] rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                        {t('capabilities.prev')}
                      </button>
                      <span className="text-[13px] text-[#8a8a8a]">
                        {t('capabilities.page', { page: catalog.page, pages: pageCount })}
                      </span>
                      <button
                        type="button"
                        disabled={page >= pageCount}
                        onClick={() => setPage((current) => current + 1)}
                        className="h-[30px] rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                        {t('capabilities.next')}
                      </button>
                    </div>
                  ) : null}
                </div>
              )}
            </section>
          </div>
        </Scrollbar>
      </div>

      {kind === 'connector' ? <ConnectorManageDialog open={manageOpen} onOpenChange={setManageOpen} /> : null}
      <ConnectorItemDialog
        entry={kind === 'connector' ? selected : null}
        pending={pendingSlug !== null}
        onOpenChange={(open) => {
          if (!open) setSelected(null)
        }}
        onInstall={toggleInstall}
        onUninstall={toggleInstall}
      />
      {kind === 'skill' ? <SkillUploadDialog open={uploadOpen} onOpenChange={setUploadOpen} /> : null}
      <SkillUseDialog
        entry={kind === 'skill' ? selected : null}
        pending={pendingSlug !== null}
        onOpenChange={(open) => {
          if (!open) setSelected(null)
        }}
        onInstall={toggleInstall}
        onUsePrompt={(entry, prompt) => void useSkillPrompt(entry, prompt)}
      />
      <PluginConfigDialog entry={configPlugin} onOpenChange={(open) => !open && setConfigPlugin(null)} />
      <PageEntry id={`capabilities.${kind}`} ready={settled || showError} />
    </div>
  )
}

function FeaturedRibbon({ label }: { label: string }) {
  return (
    <div
      data-ui="capabilities.ribbon"
      className="pointer-events-none absolute top-1.5 -left-[26px] z-10 w-[72px] -rotate-45 bg-[#3dba6c] py-px text-center text-[9px] leading-[14px] font-medium text-white shadow-[0_1px_2px_rgba(0,0,0,0.12)]">
      {label}
    </div>
  )
}

function CapabilityCard({
  entry,
  pending,
  busy,
  progress,
  failed,
  onOpen,
  onConfigure,
  onConfigureFields,
  onToggleInstall
}: {
  entry: CapabilityEntry
  pending: boolean
  busy: boolean
  progress: { received: number; total: number } | null
  failed: boolean
  onOpen?: (entry: CapabilityEntry) => void
  onConfigure?: (entry: CapabilityEntry) => void
  onConfigureFields?: (entry: CapabilityEntry) => void
  onToggleInstall: (entry: CapabilityEntry) => void
}) {
  const { t } = useTranslation()
  const fieldConfigure = Boolean(onConfigureFields) && entry.installed && entry.config.length > 0
  const okfConfigure = Boolean(onConfigure) && pluginShipsOkf(entry)
  const action = entry.installed
    ? 'capabilities.uninstall'
    : failed
      ? 'capabilities.retry'
      : busy && !progress
        ? 'capabilities.installing'
        : 'capabilities.install'
  return (
    <article
      data-ui="capabilities.card"
      tabIndex={onOpen ? 0 : undefined}
      onClick={onOpen ? () => onOpen(entry) : undefined}
      onKeyDown={
        onOpen
          ? (event) => {
              if (event.key !== 'Enter' && event.key !== ' ') return
              event.preventDefault()
              onOpen(entry)
            }
          : undefined
      }
      className={cn(
        'relative flex w-full min-w-0 flex-row flex-wrap items-center gap-3 overflow-hidden rounded-[14px] border border-[#ececec] bg-white px-4 py-3 shadow-[0_8px_22px_rgba(20,40,80,0.04)] transition-[border-color,box-shadow] duration-200 outline-none hover:shadow-[0_10px_28px_rgba(20,40,80,0.10)]',
        onOpen && 'cursor-pointer hover:border-[#e0e0e0] focus-visible:ring-2 focus-visible:ring-black/10',
        (fieldConfigure || okfConfigure) && 'pr-20'
      )}>
      {fieldConfigure ? (
        <button
          type="button"
          data-ui="plugins.configure"
          aria-label={t('plugins.okf.configure')}
          onClick={(event) => {
            event.stopPropagation()
            onConfigureFields?.(entry)
          }}
          onPointerDown={(event) => event.stopPropagation()}
          onMouseDown={(event) => event.stopPropagation()}
          onKeyDown={(event) => event.stopPropagation()}
          className={cn(
            'absolute top-2 z-20 flex h-7 items-center gap-1 rounded-[8px] border border-[#e5e5e5] bg-white px-2 text-[12px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10',
            okfConfigure ? 'right-20' : 'right-2'
          )}>
          <Settings className="size-3.5" strokeWidth={1.7} aria-hidden />
          {t('plugins.okf.configure')}
        </button>
      ) : null}
      {okfConfigure ? (
        <button
          type="button"
          data-ui="plugins.okf.configure"
          aria-label={t('plugins.okf.configure')}
          onClick={(event) => {
            event.stopPropagation()
            onConfigure?.(entry)
          }}
          onPointerDown={(event) => event.stopPropagation()}
          onMouseDown={(event) => event.stopPropagation()}
          onKeyDown={(event) => event.stopPropagation()}
          className="absolute top-2 right-2 z-20 flex h-7 items-center gap-1 rounded-[8px] border border-[#e5e5e5] bg-white px-2 text-[12px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
          <Settings className="size-3.5" strokeWidth={1.7} aria-hidden />
          {t('plugins.okf.configure')}
        </button>
      ) : null}
      <CapabilityMark entry={entry} />
      <div className="min-w-[10rem] flex-1">
        <h3 className="m-0 truncate text-[14px] font-medium text-[#1a1a1a]">{entry.name}</h3>
        <p className="m-0 mt-1 line-clamp-2 text-[13px] leading-5 text-[#8a8a8a]">{entry.description}</p>
        {progress ? (
          <div
            className="mt-2 w-full min-w-0"
            data-ui="capabilities.progress"
            role="progressbar"
            aria-valuemin={0}
            aria-valuenow={progress.received}
            aria-valuemax={progress.total}>
            <div className="h-1 w-full overflow-hidden rounded-full bg-[#f0f0f0]">
              <div
                className="h-full bg-[#171717]"
                style={{ width: `${Math.min(100, (progress.received / progress.total) * 100)}%` }}
              />
            </div>
            <p className="m-0 mt-1 text-[12px] leading-4 text-[#8a8a8a]">
              {progress.received}/{progress.total}
            </p>
          </div>
        ) : busy ? (
          <p data-ui="capabilities.applying" aria-busy="true" className="m-0 mt-2 text-[12px] leading-4 text-[#8a8a8a]">
            {t('capabilities.installing')}
          </p>
        ) : null}
      </div>
      {entry.builtin ? (
        entry.installed ? null : (
          <span className="shrink-0 text-[13px] text-[#8a8a8a]">{t('capabilities.builtin.lazy')}</span>
        )
      ) : entry.installed ? (
        <span
          className="inline-flex shrink-0"
          onPointerDown={(event) => event.stopPropagation()}
          onMouseDown={(event) => event.stopPropagation()}
          onClick={(event) => event.stopPropagation()}>
          <PopConfirm
            tone="danger"
            title={t('capabilities.uninstall_confirm', { name: entry.name })}
            confirmLabel={t('capabilities.uninstall')}
            cancelLabel={t('common.cancel')}
            disabled={pending}
            onConfirm={() => onToggleInstall(entry)}>
            <button
              type="button"
              disabled={pending}
              aria-pressed
              aria-label={t('capabilities.uninstall')}
              onKeyDown={(event) => event.stopPropagation()}
              className="flex h-[30px] shrink-0 items-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none hover:bg-[#262626] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
              <Trash2 className="lucide-custom size-3.5 text-white" strokeWidth={1.7} aria-hidden />
              {t('capabilities.uninstall')}
            </button>
          </PopConfirm>
        </span>
      ) : (
        <button
          type="button"
          disabled={pending}
          aria-pressed={false}
          aria-label={t(failed ? 'capabilities.retry' : 'capabilities.install')}
          onClick={(event) => {
            event.stopPropagation()
            onToggleInstall(entry)
          }}
          onKeyDown={(event) => event.stopPropagation()}
          className="flex h-[30px] shrink-0 items-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
          {failed ? (
            <RotateCw className="size-3.5" strokeWidth={1.7} aria-hidden />
          ) : (
            <Download className="size-3.5" strokeWidth={1.7} aria-hidden />
          )}
          {t(action)}
        </button>
      )}
      {entry.featured ? <FeaturedRibbon label={t('capabilities.category.featured')} /> : null}
    </article>
  )
}
