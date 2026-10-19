import {
  Button,
  Checkbox,
  Combobox,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  type RowMotion,
  rowMotionClassName,
  rowMotionStyle,
  SearchInput,
  SegmentedControl
} from '@a3s-lab/ui'
import { useNavigate } from '@tanstack/react-router'
import {
  AppWindow,
  Archive,
  BookOpen,
  Bot,
  Boxes,
  Bug,
  Building2,
  ChartColumn,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Check,
  CloudOff,
  Container,
  Copy,
  Cpu,
  Factory,
  FileCheck2,
  FileText,
  GitBranch,
  Globe,
  GraduationCap,
  History,
  Inbox,
  KeyRound,
  Landmark,
  Layers,
  LayoutDashboard,
  LayoutGrid,
  Link2,
  ListChecks,
  LogIn,
  type LucideIcon,
  MapPin,
  MessageSquare,
  MoreHorizontal,
  Network,
  PenLine,
  Plug,
  Puzzle,
  RadioTower,
  RefreshCw,
  Receipt,
  ReceiptText,
  Repeat,
  Rocket,
  Route,
  ScrollText,
  SearchX,
  Server,
  ServerCog,
  Settings,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  SquareFunction,
  Tag,
  Trees,
  Truck,
  Users,
  Waypoints,
  Workflow
} from 'lucide-react'
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR, { useSWRConfig, type SWRResponse } from 'swr'

import { CollectionPagination } from '@renderer/components/collection/CollectionPagination'
import { CollectionSelectionBar } from '@renderer/components/collection/CollectionSelectionBar'
import { ghostIconButtonClassName, quietButtonClassName } from '@renderer/components/collection/collectionStyles'
import { CollectionToolbar } from '@renderer/components/collection/CollectionToolbar'
import { useOptimisticRows } from '@renderer/components/collection/optimisticRows'
import { useCollectionPage } from '@renderer/components/collection/useCollectionPage'
import {
  useCollectionSelection,
  type CollectionRowSelection
} from '@renderer/components/collection/useCollectionSelection'
import { PageEntry } from '@renderer/components/layout/PageEntry'
import { useRestingDialog } from '@renderer/components/restingDialog'
import {
  canSeePlatformAdmin,
  canManagePlatformDirectory,
  listMemberOrgs,
  PERSONAL_CENTER,
  readCenterId,
  readCenterMemberId,
  subscribeCenter,
  writeCenterId
} from '@renderer/data/memberOrgs'
import {
  findAgentRuntimes,
  isPlatformAbsent,
  listCellRevisions,
  listCustomerPeople,
  listFunctionReleases,
  listModelRoutes,
  listPlatformItems,
  listPlatformLogLevels,
  listPlatformLogs,
  listPlatformUsage,
  LOG_LEVELS,
  LOG_PAGE_LIMIT,
  LOG_WINDOWS,
  type LogLevel,
  type LogWindow,
  platformErrorDetail,
  platformErrorText,
  readAgentRuntimeLogs,
  readPlatformOverview,
  readPlatformCatalogPage,
  type PlatformCatalogResource,
  type PlatformItem,
  type PlatformPage as PlatformRowPage,
  USAGE_WINDOWS,
  type UsageWindow
} from '@renderer/data/platformCatalog'
import { cn } from '@renderer/utils/style'

import {
  FLEET_FACETS,
  GATEWAY_FACETS,
  ORDER_FACETS,
  PLATFORM_EXPANDABLE,
  PLATFORM_FACET_NAV,
  PLATFORM_GROUPS,
  REFERENCE_FACETS,
  SERVICE_FACETS,
  SIGN_IN_FACETS,
  SMS_VIEWS,
  SYSTEM_FACETS,
  USAGE_FACETS,
  WORKLOAD_FACETS,
  readPlatformExpanded,
  readPlatformFacet,
  readPlatformMenu,
  subscribePlatformView,
  writePlatformExpanded,
  writePlatformFacet,
  writePlatformMenu,
  type PlatformMenuId
} from './platformNav'
import { SupplierEditorProvider, useSupplierActions } from './SupplierEditor'
import { SupplierRelations } from './SupplierRelations'

const MENU_ICONS = {
  overview: LayoutDashboard,
  customers: Users,
  channels: Building2,
  suppliers: Truck,
  orders: Receipt,
  tickets: Inbox,
  services: Layers,
  models: Cpu,
  workloads: Workflow,
  gateway: Waypoints,
  fleet: Server,
  logs: ScrollText,
  usage: ChartColumn,
  system: Settings,
  signIn: LogIn
} as const

/** Sub-menu icons, keyed by the section's scope and ID (`services.agents`, `signIn.sms.records`). */
const SECTION_ICONS: Readonly<Record<string, LucideIcon>> = {
  'orders.orders': ReceiptText,
  'orders.subscriptions': Repeat,
  'services.agents': Bot,
  'services.applets': AppWindow,
  'services.connectors': Plug,
  'services.skills': Sparkles,
  'services.plugins': Puzzle,
  'workloads.workflows': GitBranch,
  'workloads.functions': SquareFunction,
  'workloads.runtimes': Container,
  'workloads.cells': Boxes,
  'gateway.routes': Route,
  'gateway.platform': Globe,
  'gateway.standards': FileCheck2,
  'fleet.fleet': ServerCog,
  'fleet.topology': Network,
  'usage.customers': Users,
  'usage.models': Cpu,
  'system.apps': LayoutGrid,
  'system.bootstrap': Rocket,
  'system.runtime': SlidersHorizontal,
  'system.debug': Bug,
  'system.reference': BookOpen,
  'system.reference.industries': Factory,
  'system.reference.chains': Link2,
  'system.reference.tags': Tag,
  'system.reference.regions': MapPin,
  'system.reference.organs': Landmark,
  'system.reference.parks': Trees,
  'system.reference.universities': GraduationCap,
  'signIn.oauth': KeyRound,
  'signIn.captcha': ShieldCheck,
  'signIn.sms': MessageSquare,
  'signIn.sms.profiles': Settings2,
  'signIn.sms.providers': RadioTower,
  'signIn.sms.signatures': PenLine,
  'signIn.sms.templates': FileText,
  'signIn.sms.scenarios': ListChecks,
  'signIn.sms.records': History
}

function navClass(selected: boolean) {
  return cn(
    'flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] transition-colors outline-none',
    'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
    selected ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
  )
}

/** Official-team platform views follow directory and explicit resource grants. */
export default function PlatformPage() {
  const { t } = useTranslation()
  const [centerId, setCenterId] = useState(readCenterId)
  const [menu, setMenuState] = useState<PlatformMenuId>(readPlatformMenu)
  const [expanded, setExpanded] = useState(readPlatformExpanded)
  const [navQuery, setNavQuery] = useState('')
  const [, setViewVersion] = useState(0)
  const setMenu = (id: PlatformMenuId) => {
    setMenuState(id)
    writePlatformMenu(id)
  }
  useEffect(
    () =>
      subscribeCenter(() => {
        setCenterId(readCenterId())
        setViewVersion((version) => version + 1)
      }),
    []
  )
  useEffect(
    () =>
      subscribePlatformView(() => {
        setMenuState(readPlatformMenu())
        setExpanded(readPlatformExpanded())
        setViewVersion((version) => version + 1)
      }),
    []
  )
  const { data: orgs, error: orgError, mutate: reloadOrgs } = useSWR('studio/member-orgs', () => listMemberOrgs())
  const current = centerId === PERSONAL_CENTER ? undefined : orgs?.find((org) => org.id === centerId)
  const allowed = !orgError && canSeePlatformAdmin(current)
  const directory = canManagePlatformDirectory(current)
  const visibleMenu = directory ? menu : 'suppliers'

  if (orgError) {
    return (
      <section data-ui="platform.page" className="flex h-full flex-1 flex-col bg-white px-8 py-6">
        <h1 className="m-0 text-[24px] font-bold">{t('platform.title')}</h1>
        <FailureNotice marker="platform.access.error" error={orgError} onRetry={() => void reloadOrgs()} />
      </section>
    )
  }

  if (!orgs) {
    return (
      <section data-ui="platform.page" className="relative min-h-0 flex-1 bg-white">
        <PageEntry id="platform" ready={false} />
      </section>
    )
  }

  if (!allowed) {
    return (
      <section data-ui="platform.page" className="flex h-full min-h-0 flex-1 flex-col bg-white px-8 py-6">
        <h1 className="m-0 text-[24px] leading-tight font-bold text-foreground">{t('platform.title')}</h1>
        <NoticeLine marker="platform.access.denied" icon={LogIn} text={t('platform.access.denied')} />
      </section>
    )
  }

  const needle = navQuery.trim().toLowerCase()
  const groups = PLATFORM_GROUPS.map((group) => ({
    ...group,
    ids: group.ids.filter((id) => {
      if (!directory && id !== 'suppliers') return false
      const config = PLATFORM_FACET_NAV[id]
      const labels = [
        t(`platform.nav.${id}`),
        ...(config?.facets.map((facet) => t(`${config.prefix}.${facet}`)) ?? []),
        ...(id === 'system' ? REFERENCE_FACETS.map((facet) => t(`platform.reference.${facet}`)) : []),
        ...(id === 'signIn' ? SMS_VIEWS.map((facet) => t(`platform.signIn.sms.${facet}`)) : [])
      ]
      return !needle || labels.some((label) => label.toLowerCase().includes(needle))
    })
  })).filter((group) => group.ids.length > 0)

  return (
    <SupplierEditorProvider
      key={`${readCenterMemberId()}/${centerId}`}
      scope={`${readCenterMemberId()}/${centerId}`}
      active={visibleMenu === 'suppliers'}>
      <section data-ui="platform.page" className="flex h-full min-h-0 flex-1 bg-white">
        <aside className="flex h-full min-h-0 w-[260px] shrink-0 flex-col border-r border-[#efefef] bg-white">
          <header data-ui="platform.heading" className="mx-3 flex items-center gap-2.5 px-2.5 pt-4 pb-3">
            <span className="flex size-8 shrink-0 items-center justify-center rounded-[8px] bg-[#f2f3f5] text-[#3f3f3f]">
              <Building2 className="size-4 text-current" strokeWidth={1.7} aria-hidden />
            </span>
            <div className="min-w-0">
              <h1
                id="platform-sidebar-title"
                className="m-0 truncate text-[14px] leading-5 font-medium tracking-[-0.01em] text-foreground">
                {t('platform.title')}
              </h1>
              {current?.name ? (
                <p className="m-0 truncate text-[12px] leading-4 text-[#8a8a8a]">{current.name}</p>
              ) : null}
            </div>
          </header>
          <SearchInput
            size="sm"
            value={navQuery}
            onChange={(event) => setNavQuery(event.target.value)}
            onClear={() => setNavQuery('')}
            clearLabel={t('common.clear')}
            aria-label={t('platform.nav.search')}
            placeholder={t('platform.nav.search')}
            containerClassName="mx-5 mb-3 bg-[#f5f5f5]"
          />
          <nav
            aria-labelledby="platform-sidebar-title"
            className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-3 pb-4">
            {groups.map((group) => (
              <div key={group.id} role="group" aria-labelledby={`platform-group-${group.id}`} className="grid gap-0.5">
                <p id={`platform-group-${group.id}`} className="m-0 px-2.5 pb-1 text-[12px] leading-4 text-[#8a8a8a]">
                  {t(`platform.group.${group.id}`)}
                </p>
                {group.ids.map((id) => {
                  const Icon = MENU_ICONS[id]
                  const selected = visibleMenu === id
                  const attention = directory ? MENU_ATTENTION.get(id) : undefined
                  const attentionId = `platform-nav-attention-${id}`
                  const expandable = PLATFORM_EXPANDABLE.has(id)
                  const open = expandable && (Boolean(needle) || expanded.has(id))
                  const sectionsId = `platform-nav-sections-${id}`
                  return (
                    <div key={id}>
                      <div className="relative">
                        <button
                          type="button"
                          className={cn(navClass(selected), expandable && 'pr-9')}
                          aria-current={selected ? 'page' : undefined}
                          aria-describedby={attention ? attentionId : undefined}
                          onClick={() => setMenu(id)}>
                          <Icon className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
                          <span className="min-w-0 flex-1 truncate">{t(`platform.nav.${id}`)}</span>
                          {attention ? <MenuAttention id={attentionId} source={attention} /> : null}
                        </button>
                        {expandable ? (
                          <NavDisclosure
                            open={open}
                            controls={sectionsId}
                            label={t('platform.nav.sections', { name: t(`platform.nav.${id}`) })}
                            disabled={Boolean(needle)}
                            onToggle={() => writePlatformExpanded(id, !open)}
                          />
                        ) : null}
                      </div>
                      {expandable ? (
                        <NavCollapse id={sectionsId} open={open}>
                          <FacetNavigation
                            menu={id}
                            active={selected}
                            revealAll={Boolean(needle)}
                            expanded={expanded}
                            onSelect={() => setMenu(id)}
                          />
                        </NavCollapse>
                      ) : null}
                    </div>
                  )
                })}
              </div>
            ))}
            {groups.length === 0 ? (
              <p role="status" className="px-2.5 text-[13px] text-[#8a8a8a]">
                {t('platform.nav.noMatch')}
              </p>
            ) : null}
          </nav>
        </aside>
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <PlatformMenu menu={visibleMenu} onSelect={setMenu} />
        </div>
      </section>
    </SupplierEditorProvider>
  )
}

/** Disclosure for a sidebar sub-menu: a chevron that turns down while the sub-menu is open. */
function NavDisclosure({
  open,
  controls,
  label,
  disabled,
  onToggle
}: {
  open: boolean
  controls: string
  label: string
  disabled?: boolean
  onToggle: () => void
}) {
  return (
    <button
      type="button"
      aria-expanded={open}
      aria-controls={controls}
      aria-label={label}
      disabled={disabled}
      onClick={onToggle}
      className="absolute top-1/2 right-1.5 flex size-6 -translate-y-1/2 items-center justify-center rounded-[6px] text-[#8a8a8a] transition-colors outline-none hover:bg-black/[0.05] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:pointer-events-none">
      <ChevronRight
        className={cn(
          'size-3.5 transition-transform duration-200 ease-out motion-reduce:transition-none',
          open && 'rotate-90'
        )}
        strokeWidth={1.7}
        aria-hidden
      />
    </button>
  )
}

/** Animates a sub-menu's height; a closed sub-menu stays mounted but is inert and hidden from assistive tech. */
function NavCollapse({ id, open, children }: { id: string; open: boolean; children: ReactNode }) {
  return (
    <div
      id={id}
      inert={!open}
      aria-hidden={open ? undefined : true}
      className={cn(
        'grid transition-[grid-template-rows,opacity] duration-200 ease-out motion-reduce:transition-none',
        open ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'
      )}>
      <div className="min-h-0 overflow-hidden">{children}</div>
    </div>
  )
}

function FacetNavigation({
  menu,
  active,
  revealAll,
  expanded,
  onSelect
}: {
  menu: PlatformMenuId
  active: boolean
  revealAll: boolean
  expanded: ReadonlySet<string>
  onSelect: () => void
}) {
  const { t } = useTranslation()
  const config = PLATFORM_FACET_NAV[menu]
  if (!config) return null
  const current = readPlatformFacet(menu, config.facets)
  return (
    <ul
      aria-label={t('platform.nav.sections', { name: t(`platform.nav.${menu}`) })}
      className="m-0 ml-[18px] grid list-none gap-0.5 border-l border-[#efefef] py-1 pl-2">
      {config.facets.map((facet) => {
        const selected = active && current === facet
        const scope = `${menu}.${facet}`
        const nested =
          scope === 'system.reference'
            ? { facets: REFERENCE_FACETS, prefix: 'platform.reference' }
            : scope === 'signIn.sms'
              ? { facets: SMS_VIEWS, prefix: 'platform.signIn.sms' }
              : undefined
        const open = Boolean(nested) && (revealAll || expanded.has(scope))
        const nestedId = `platform-nav-sections-${menu}-${facet}`
        const label = t(`${config.prefix}.${facet}`)
        return (
          <li key={facet}>
            <div className="relative">
              <button
                type="button"
                aria-current={selected ? 'location' : undefined}
                className={cn(navClass(selected), 'h-8 gap-2 text-[13px]', nested && 'pr-9')}
                aria-label={t('platform.nav.openSection', { name: label })}
                onClick={() => {
                  writePlatformFacet(menu, facet)
                  if (nested) writePlatformExpanded(scope, true)
                  onSelect()
                }}>
                <SectionIcon scope={scope} />
                <span className="min-w-0 flex-1 truncate">{label}</span>
              </button>
              {nested ? (
                <NavDisclosure
                  open={open}
                  controls={nestedId}
                  label={t('platform.nav.sections', { name: label })}
                  disabled={revealAll}
                  onToggle={() => writePlatformExpanded(scope, !open)}
                />
              ) : null}
            </div>
            {nested ? (
              <NavCollapse id={nestedId} open={open}>
                <ul
                  aria-label={t('platform.nav.sections', { name: label })}
                  className="m-0 ml-[15px] grid list-none gap-0.5 border-l border-[#efefef] py-1 pl-2">
                  {nested.facets.map((child) => {
                    const chosen = selected && readPlatformFacet(scope, nested.facets) === child
                    return (
                      <li key={child}>
                        <button
                          type="button"
                          className={cn(navClass(chosen), 'h-8 gap-2 text-[13px]')}
                          aria-current={chosen ? 'location' : undefined}
                          aria-label={t('platform.nav.openSection', { name: t(`${nested.prefix}.${child}`) })}
                          onClick={() => {
                            writePlatformFacet(menu, facet)
                            writePlatformFacet(scope, child)
                            onSelect()
                          }}>
                          <SectionIcon scope={`${scope}.${child}`} />
                          <span className="min-w-0 flex-1 truncate">{t(`${nested.prefix}.${child}`)}</span>
                        </button>
                      </li>
                    )
                  })}
                </ul>
              </NavCollapse>
            ) : null}
          </li>
        )
      })}
    </ul>
  )
}

function SectionIcon({ scope }: { scope: string }) {
  const Icon = SECTION_ICONS[scope]
  return Icon ? <Icon className="size-3.5 shrink-0" strokeWidth={1.7} aria-hidden /> : null
}

/** One page's segment, remembered across visits and restarts. */
function usePlatformFacet<T extends string>(scope: string, facets: readonly T[], fallback?: T) {
  const [facet, setFacet] = useState<T>(() => readPlatformFacet(scope, facets, fallback))
  useEffect(
    () => subscribePlatformView(() => setFacet(readPlatformFacet(scope, facets, fallback))),
    [scope, facets, fallback]
  )
  const choose = (next: string) => {
    const picked = facets.find((id) => id === next)
    if (!picked) return
    setFacet(picked)
    writePlatformFacet(scope, picked)
  }
  return [facet, choose] as const
}

function PlatformMenu({ menu, onSelect }: { menu: PlatformMenuId; onSelect: (id: PlatformMenuId) => void }) {
  switch (menu) {
    case 'overview':
      return <OverviewPage onSelect={onSelect} />
    case 'channels':
      return <ChannelsPage />
    case 'orders':
      return <FacetPage key="orders" menu="orders" facets={ORDER_FACETS} facetPrefix="platform.orders" />
    case 'services':
      return <FacetPage key="services" menu="services" facets={SERVICE_FACETS} facetPrefix="platform.services" />
    case 'workloads':
      return <FacetPage key="workloads" menu="workloads" facets={WORKLOAD_FACETS} facetPrefix="platform.workloads" />
    case 'fleet':
      return <FacetPage key="fleet" menu="fleet" facets={FLEET_FACETS} facetPrefix="platform.fleet" />
    case 'system':
      return <SystemPage />
    case 'signIn':
      return <FacetPage key="signIn" menu="signIn" facets={SIGN_IN_FACETS} facetPrefix="platform.signIn" />
    case 'logs':
      return <LogsPage />
    case 'usage':
      return <UsagePage />
    case 'gateway':
      return <GatewayPage />
    default:
      return <CatalogPage menu={menu} />
  }
}

const searchFieldClassName =
  'h-[30px] w-[220px] shrink-0 border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0 dark:bg-[#2c2c2c]'

const featureCardClassName = 'rounded-[16px] border border-[#efefef] bg-white p-3 shadow-[0_4px_16px_rgba(0,0,0,0.04)]'

function PlatformFrame({
  title,
  description,
  actions,
  overlay,
  children
}: {
  title: string
  description?: string
  actions?: ReactNode
  /** Floats at the bottom centre of the list viewport without taking layout height, e.g. the selection bar. */
  overlay?: ReactNode
  children: ReactNode
}) {
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header className="flex shrink-0 flex-wrap items-start justify-between gap-4 px-6 pt-4 pb-2">
        <div className="min-w-0">
          <h2 className="m-0 truncate text-[24px] leading-tight font-bold tracking-[-0.01em] text-foreground">
            {title}
          </h2>
          {description ? (
            <p className="m-0 mt-1 max-w-2xl text-[15px] leading-6 text-[#8a8a8a]">{description}</p>
          ) : null}
        </div>
        {actions ? (
          <div className="flex max-w-full shrink-0 flex-wrap items-center justify-end gap-2">{actions}</div>
        ) : null}
      </header>
      <div className="relative flex min-h-0 flex-1 flex-col">
        {/* Extra end padding lets the last row and the pager scroll clear of the floating overlay. */}
        <div className={cn('min-h-0 flex-1 overflow-auto px-6', overlay ? 'pb-20' : 'pb-6')}>{children}</div>
        {overlay ? (
          <div className="pointer-events-none absolute inset-x-6 bottom-4 z-10 flex justify-center">{overlay}</div>
        ) : null}
      </div>
    </div>
  )
}

function RefreshButton({ busy, updatedAt, onClick }: { busy: boolean; updatedAt?: number; onClick: () => void }) {
  const { t } = useTranslation()
  return (
    <>
      {updatedAt ? <UpdatedAt at={updatedAt} /> : null}
      <button
        type="button"
        aria-label={t('common.refresh')}
        title={t('common.refresh')}
        aria-busy={busy || undefined}
        disabled={busy}
        onClick={onClick}
        className={ghostIconButtonClassName}>
        <RefreshCw
          className={cn('size-4 text-current', busy && 'animate-spin')}
          style={busy ? { animationDuration: '0.7s' } : undefined}
          strokeWidth={1.7}
          aria-hidden
        />
      </button>
    </>
  )
}

type OverviewRow = { menu: PlatformMenuId; label: string; value: number | undefined; hint?: string }

const RECENT_LOGS = 5

function readyShare(rows: PlatformItem[] | undefined, ready: (item: PlatformItem) => boolean) {
  if (!rows?.length) return undefined
  return `${rows.filter(ready).length}/${rows.length}`
}

function OverviewPage({ onSelect }: { onSelect: (id: PlatformMenuId) => void }) {
  const { t } = useTranslation()
  const { data, error, isValidating, mutate } = useTrackedSWR(OVERVIEW_KEY, () => readPlatformOverview())
  const workflows = useTrackedSWR(ATTENTION_SOURCES.workloads.swrKey, () => listPlatformItems('workloads', 'workflows'))
  const routes = useTrackedSWR(ATTENTION_SOURCES.routes.swrKey, () => listPlatformItems('gateway'))
  const nodes = useTrackedSWR(ATTENTION_SOURCES.nodes.swrKey, () => listPlatformItems('fleet', 'fleet'))
  const logs = useSWR('studio/platform/logs', () => listPlatformItems('logs'))
  const usage = useTrackedSWR(usageKey(OVERVIEW_USAGE_DAYS), () => listPlatformUsage(OVERVIEW_USAGE_DAYS))
  const suppliers = useTrackedSWR(ATTENTION_SOURCES.suppliers.swrKey, () => listPlatformItems('suppliers'))
  const signIn = useTrackedSWR(ATTENTION_SOURCES.signIn.swrKey, () => listPlatformItems('sign-in', 'oauth'))
  const checks: AttentionCheck[] = [
    { key: 'workloads', ...ATTENTION_SOURCES.workloads, source: workflows },
    { key: 'routes', ...ATTENTION_SOURCES.routes, source: routes },
    { key: 'nodes', ...ATTENTION_SOURCES.nodes, source: nodes },
    { key: 'suppliers', ...ATTENTION_SOURCES.suppliers, source: suppliers },
    { key: 'signIn', ...ATTENTION_SOURCES.signIn, source: signIn }
  ]
  const cards: { title: string; description: string; rows: OverviewRow[] }[] = [
    {
      title: t('platform.overview.business'),
      description: t('platform.overview.businessDescription'),
      rows: [
        { menu: 'customers', label: t('platform.nav.customers'), value: data?.customers },
        { menu: 'channels', label: t('platform.nav.channels'), value: data?.partners },
        { menu: 'suppliers', label: t('platform.nav.suppliers'), value: data?.suppliers }
      ]
    },
    {
      title: t('platform.overview.platform'),
      description: t('platform.overview.platformDescription'),
      rows: [
        { menu: 'models', label: t('platform.nav.models'), value: data?.models },
        { menu: 'services', label: t('platform.nav.services'), value: data?.services }
      ]
    },
    {
      title: t('platform.overview.runtime'),
      description: t('platform.overview.runtimeDescription'),
      rows: [
        {
          menu: 'workloads',
          label: t('platform.workloads.workflows'),
          value: workflows.data?.length,
          hint: readyShare(workflows.data, (item) => fieldValue(item, 'health') === 'healthy')
        },
        {
          menu: 'gateway',
          label: t('platform.overview.routes'),
          value: routes.data?.length,
          hint: readyShare(routes.data, (item) => fieldValue(item, 'health') === 'healthy')
        },
        {
          menu: 'fleet',
          label: t('platform.overview.nodes'),
          value: nodes.data?.length,
          hint: readyShare(nodes.data, (item) => {
            const availability = fieldValue(item, 'availability')
            return Boolean(availability) && availability !== 'offline'
          })
        }
      ]
    }
  ]
  const busy =
    Boolean(data) &&
    (isValidating ||
      workflows.isValidating ||
      routes.isValidating ||
      nodes.isValidating ||
      logs.isValidating ||
      usage.isValidating ||
      suppliers.isValidating ||
      signIn.isValidating)
  const refresh = () => {
    void mutate()
    void workflows.mutate()
    void routes.mutate()
    void nodes.mutate()
    void logs.mutate()
    void usage.mutate()
    void suppliers.mutate()
    void signIn.mutate()
  }
  return (
    <PlatformFrame
      title={t('platform.nav.overview')}
      description={t('platform.overview.description')}
      actions={<RefreshButton busy={busy} updatedAt={loadedAt.get(OVERVIEW_KEY)} onClick={refresh} />}>
      {!data && !error ? (
        <div className="relative min-h-40">
          <PageEntry id="platform.overview" ready={false} />
        </div>
      ) : error ? (
        <FailureNotice marker="platform.overview" error={error} onRetry={refresh} />
      ) : (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3" data-ui="platform.overview">
          <OverviewAttention checks={checks} onOpen={onSelect} />
          {cards.map((card) => (
            <article key={card.title} className={featureCardClassName}>
              <h3 className="m-0 text-[18px] leading-6 font-semibold text-[#1a1a1a]">{card.title}</h3>
              <p className="m-0 mt-1 text-[15px] leading-6 text-[#8a8a8a]">{card.description}</p>
              <ul className="m-0 mt-3 grid gap-0.5 p-0">
                {card.rows.map((row) => (
                  <li key={row.menu} className="list-none">
                    <button
                      type="button"
                      className="flex h-9 w-full items-center justify-between gap-3 rounded-[8px] px-2.5 text-left outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset"
                      onClick={() => onSelect(row.menu)}>
                      <span className="text-[13px] text-[#3f3f3f]">{row.label}</span>
                      <span className="flex items-baseline gap-2">
                        {row.hint ? (
                          <span className="text-[12px] text-[#8a8a8a]">
                            {t('platform.overview.ready', { value: row.hint })}
                          </span>
                        ) : null}
                        <span className="text-[13px] font-medium text-[#1a1a1a]">{row.value ?? '—'}</span>
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </article>
          ))}
          <OverviewUsage rows={usage.data} failed={Boolean(usage.error)} onOpen={() => onSelect('usage')} />
          <RecentLogs rows={logs.data} failed={Boolean(logs.error)} onOpen={() => onSelect('logs')} />
        </div>
      )}
    </PlatformFrame>
  )
}

const OVERVIEW_USAGE_DAYS: UsageWindow = '7'
const OVERVIEW_TOP = 5
const OVERVIEW_KEY = 'studio/platform-overview'

type AttentionSource = {
  menu: PlatformMenuId
  facet?: string
  swrKey: string
  fields: readonly string[]
  load: () => Promise<PlatformItem[]>
}

/** Lists whose rows can need an operator; the keys are the ones the list pages read, so both share one cache. */
const ATTENTION_SOURCES = {
  workloads: {
    menu: 'workloads',
    facet: 'workflows',
    swrKey: 'studio/platform/workloads/workflows',
    fields: ['health'],
    load: () => listPlatformItems('workloads', 'workflows')
  },
  routes: {
    menu: 'gateway',
    facet: 'routes',
    swrKey: 'studio/platform/gateway',
    fields: ['health'],
    load: () => listPlatformItems('gateway')
  },
  nodes: {
    menu: 'fleet',
    facet: 'fleet',
    swrKey: 'studio/platform/fleet/fleet',
    fields: ['state', 'availability'],
    load: () => listPlatformItems('fleet', 'fleet')
  },
  suppliers: {
    menu: 'suppliers',
    swrKey: 'studio/platform/suppliers',
    fields: ['status', 'credential'],
    load: () => listPlatformItems('suppliers')
  },
  signIn: {
    menu: 'signIn',
    facet: 'oauth',
    swrKey: 'studio/platform/sign-in/oauth',
    fields: ['status'],
    load: () => listPlatformItems('sign-in', 'oauth')
  }
} as const satisfies Record<string, AttentionSource>

const MENU_ATTENTION = new Map<PlatformMenuId, AttentionSource>(
  Object.values(ATTENTION_SOURCES).map((source) => [source.menu, source])
)

function attentionCount(rows: PlatformItem[] | undefined, fields: readonly string[]) {
  return (rows ?? []).filter((item) => fields.some((field) => ATTENTION_STATUSES.has(fieldValue(item, field)))).length
}

type AttentionCheck = AttentionSource & {
  key: string
  source: Pick<SWRResponse<PlatformItem[]>, 'data' | 'error'>
}

/** Reads only what is already cached, so the sidebar never adds a request of its own. */
function MenuAttention({ id, source }: { id: string; source: AttentionSource }) {
  const { t } = useTranslation()
  // A null fetcher on the first subscriber consumes SWR's mutate revalidation event,
  // leaving the visible page's refresh inert. Keep the shared fetcher but do not fetch on badge mount.
  const { data, error } = useSWR<PlatformItem[]>(source.swrKey, source.load, { revalidateOnMount: false })
  if (error) return null
  const count = attentionCount(data, source.fields)
  if (count === 0) return null
  return (
    <span
      data-ui="platform.nav.attention"
      className="flex shrink-0 items-center gap-1.5 text-[12px] font-medium text-[#171717]">
      <span aria-hidden className="size-1.5 rounded-full bg-[#171717]" />
      <span aria-hidden>{count}</span>
      <span id={id} hidden>
        {t('platform.nav.attention', { count })}
      </span>
    </span>
  )
}

/** Problems first: each check counts rows whose state an operator has to act on, and opens that menu. */
function OverviewAttention({ checks, onOpen }: { checks: AttentionCheck[]; onOpen: (menu: PlatformMenuId) => void }) {
  const { t } = useTranslation()
  const memory = useListMemory()
  const settled = checks.every((check) => check.source.data || check.source.error)
  const unread = checks.filter((check) => check.source.error).length
  const flagged = checks.flatMap((check) => {
    const count = attentionCount(check.source.data, check.fields)
    return count > 0 ? [{ ...check, count }] : []
  })
  if (!settled && flagged.length === 0) return null
  return (
    <article data-ui="platform.overview.attention" className={cn(featureCardClassName, 'md:col-span-2 xl:col-span-3')}>
      <h3 className="m-0 text-[18px] leading-6 font-semibold text-[#1a1a1a]">
        {t('platform.overview.attentionTitle')}
      </h3>
      {flagged.length === 0 ? (
        <p className="m-0 mt-2 flex items-center gap-2 text-[13px] text-[#8a8a8a]">
          {unread > 0 ? null : <Check aria-hidden className="size-4 text-[#171717]" strokeWidth={1.75} />}
          {unread > 0
            ? t('platform.overview.attentionUnread', { count: unread })
            : t('platform.overview.attentionClear')}
        </p>
      ) : (
        <ul className="m-0 mt-2 grid gap-0.5 p-0 md:grid-cols-2 xl:grid-cols-3">
          {flagged.map((check) => (
            <li key={check.key} className="list-none">
              <button
                type="button"
                onClick={() => {
                  if (check.facet) writePlatformFacet(check.menu, check.facet)
                  memory.delete(check.swrKey)
                  onOpen(check.menu)
                }}
                className="flex h-9 w-full items-center gap-2 rounded-[8px] px-2.5 text-left outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
                <span aria-hidden className="size-1.5 shrink-0 rounded-full bg-[#171717]" />
                <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-[#171717]">
                  {t(`platform.overview.issue.${check.key}`, { count: check.count })}
                </span>
                <ChevronRight aria-hidden className="size-4 shrink-0 text-[#8a8a8a]" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </article>
  )
}

/** When each list last came back from the server, kept across menu switches. */
const loadedAt = new Map<string, number>()

function markLoaded(key: string) {
  loadedAt.set(key, Date.now())
}

function useTrackedSWR<T>(key: string, load: () => Promise<T>) {
  return useSWR(key, load, { onSuccess: () => markLoaded(key) })
}

const FRESHNESS_TICK_MS = 30_000

function UpdatedAt({ at }: { at: number }) {
  const { t } = useTranslation()
  const [, setNow] = useState(0)
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), FRESHNESS_TICK_MS)
    return () => window.clearInterval(timer)
  }, [])
  const iso = new Date(at).toISOString()
  return (
    <span
      data-ui="platform.updated-at"
      className="text-[12px] whitespace-nowrap text-[#8a8a8a]"
      title={formatInstant(iso)}>
      {t('platform.updated', { value: formatAgo(iso, t) })}
    </span>
  )
}

function OverviewUsage({
  rows,
  failed,
  onOpen
}: {
  rows: PlatformItem[] | undefined
  failed: boolean
  onOpen: () => void
}) {
  const { t } = useTranslation()
  const totals = rows ? usageTotals(rows) : undefined
  let body: ReactNode
  if (!totals || totals.tokens === 0) {
    const empty = failed ? 'platform.overview.usageFailed' : totals ? 'platform.usage.empty' : ''
    body = <p className="m-0 py-6 text-center text-[13px] text-[#8a8a8a]">{empty ? t(empty) : '—'}</p>
  } else {
    const stats = [
      { key: 'tokens', label: t('platform.usage.stat.tokens'), value: formatTokens(String(totals.tokens)) },
      { key: 'requests', label: t('platform.usage.stat.requests'), value: totals.requests.toLocaleString() },
      { key: 'success', label: t('platform.usage.stat.success'), value: totals.success },
      { key: 'customers', label: t('platform.usage.stat.customers'), value: String(totals.active) }
    ]
    body = (
      <>
        <dl className="m-0 mt-3 grid grid-cols-2 gap-x-6 gap-y-3 px-2.5 lg:grid-cols-4">
          {stats.map((stat) => (
            <div key={stat.key} className="min-w-0">
              <dt className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{stat.label}</dt>
              <dd className="m-0 mt-1 text-[18px] leading-6 font-semibold text-[#1a1a1a]">{stat.value}</dd>
            </div>
          ))}
        </dl>
        <div className="mt-4 grid gap-4 lg:grid-cols-2">
          <UsageRanking
            marker="platform.overview.topModels"
            title={t('platform.overview.topModels')}
            rows={totals.models.slice(0, OVERVIEW_TOP)}
          />
          <UsageRanking
            marker="platform.overview.topCustomers"
            title={t('platform.overview.topCustomers')}
            rows={totals.customers.filter((item) => Number(fieldValue(item, 'tokens')) > 0).slice(0, OVERVIEW_TOP)}
          />
        </div>
        {totals.partial > 0 ? (
          <p className="m-0 mt-3 px-2.5 text-[12px] text-[#8a8a8a]">
            {t('platform.usage.partial', { value: totals.partial })}
          </p>
        ) : null}
      </>
    )
  }
  return (
    <article data-ui="platform.overview.usage" className={cn(featureCardClassName, 'md:col-span-2 xl:col-span-3')}>
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="m-0 text-[18px] leading-6 font-semibold text-[#1a1a1a]">{t('platform.overview.usage')}</h3>
          <p className="m-0 mt-1 text-[15px] leading-6 text-[#8a8a8a]">{t('platform.overview.usageDescription')}</p>
        </div>
        <button type="button" className={textButtonClassName} onClick={onOpen}>
          {t('platform.overview.viewAll')}
        </button>
      </header>
      {body}
    </article>
  )
}

/** Rows arrive ranked by tokens; the bar is the row's share of the window's tokens. */
function UsageRanking({ marker, title, rows }: { marker: string; title: string; rows: PlatformItem[] }) {
  return (
    <section data-ui={marker} className="min-w-0">
      <p className={cn(detailLabelClassName, 'px-2.5')}>{title}</p>
      <ol className="m-0 mt-1 p-0">
        {rows.map((item, index) => {
          const share = Number(fieldValue(item, 'share')) || 0
          return (
            <li key={item.id} className="grid list-none gap-1 px-2.5 py-1.5">
              <div className="flex items-baseline gap-3">
                <span className="w-4 shrink-0 text-[12px] text-[#8a8a8a] tabular-nums">{index + 1}</span>
                <span className="min-w-0 flex-1 truncate text-[13px] text-[#1a1a1a]" title={item.name}>
                  {item.name}
                </span>
                <span className="shrink-0 text-[12px] text-[#8a8a8a] tabular-nums">{share.toFixed(1)}%</span>
                <span className="w-14 shrink-0 text-right text-[13px] font-medium text-[#1a1a1a] tabular-nums">
                  {formatTokens(fieldValue(item, 'tokens'))}
                </span>
              </div>
              <div className="ml-7 h-1 overflow-hidden rounded-full bg-[#f2f3f5]">
                <div className="h-full rounded-full bg-[#3f3f3f]" style={{ width: `${Math.min(100, share)}%` }} />
              </div>
            </li>
          )
        })}
      </ol>
    </section>
  )
}

function RecentLogs({
  rows,
  failed,
  onOpen
}: {
  rows: PlatformItem[] | undefined
  failed: boolean
  onOpen: () => void
}) {
  const { t } = useTranslation()
  const recent = (rows ?? []).slice(0, RECENT_LOGS)
  return (
    <article className={cn(featureCardClassName, 'md:col-span-2 xl:col-span-3')}>
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="m-0 text-[18px] leading-6 font-semibold text-[#1a1a1a]">{t('platform.overview.logs')}</h3>
          <p className="m-0 mt-1 text-[15px] leading-6 text-[#8a8a8a]">{t('platform.overview.logsDescription')}</p>
        </div>
        <button type="button" className={textButtonClassName} onClick={onOpen}>
          {t('platform.overview.viewAll')}
        </button>
      </header>
      {recent.length === 0 ? (
        <p className="m-0 py-6 text-center text-[13px] text-[#8a8a8a]">
          {failed ? t('platform.overview.logsFailed') : rows ? t('platform.empty') : '—'}
        </p>
      ) : (
        <ul className="m-0 mt-3 p-0">
          {recent.map((item) => (
            <li
              key={item.id}
              className="flex h-9 list-none items-center gap-3 border-b border-[#efefef] px-2.5 last:border-b-0">
              <span className="w-16 shrink-0">
                <StatusChip value={fieldValue(item, 'level')} />
              </span>
              <span className="min-w-0 flex-1 truncate text-[13px] text-[#1a1a1a]" title={item.name}>
                {item.name}
              </span>
              <span className="hidden shrink-0 text-[12px] text-[#8a8a8a] md:inline">
                {fieldValue(item, 'service')}
              </span>
              <span className="shrink-0 text-[12px] text-[#8a8a8a]">{formatInstant(fieldValue(item, 'time'))}</span>
            </li>
          ))}
        </ul>
      )}
    </article>
  )
}

const MENU_RESOURCE: Partial<Record<PlatformMenuId, string>> = {
  customers: 'customers',
  suppliers: 'suppliers',
  tickets: 'tickets',
  models: 'models',
  gateway: 'gateway',
  logs: 'logs'
}

type Column = {
  key: string
  labelKey: string
  chip?: boolean
  muted?: boolean
  filter?: boolean
  /** The server already filters this column; the page keeps its own control. */
  noFilter?: boolean
  select?: boolean
  /** `ago` reads as elapsed time, with the exact time on hover. */
  format?: 'time' | 'instant' | 'ago' | 'bytes' | 'duration' | 'tokens'
  width?: string
}
const NUMERIC_KEYS = new Set([
  'requests',
  'errors',
  'backends',
  'replicas',
  'cpu',
  'concurrency',
  'seats',
  'installs',
  'tools',
  'models',
  'routes',
  'nodes',
  'online',
  'tokens',
  'success',
  'share',
  'customers'
])
const NUMERIC_FORMATS = new Set<Column['format']>(['tokens', 'bytes', 'duration'])
/** Closed sets whose labels stay a word or two long. */
const ENUM_KEYS = new Set([
  'kind',
  'plan',
  'grade',
  'mode',
  'credential',
  'bound',
  'platform',
  'deployment',
  'transport'
])

/** Quantities read down a column, so they align on the right with equal-width digits. */
function isNumeric(column: Column) {
  return !column.chip && (NUMERIC_KEYS.has(column.key) || NUMERIC_FORMATS.has(column.format))
}

/** Short, predictable columns get a fixed width so free text (the name first) keeps the rest. */
function columnWidth(column: Column) {
  if (column.width) return column.width
  if (column.select) return 'w-[140px]'
  if (column.chip || column.filter || ENUM_KEYS.has(column.key)) return 'w-[112px]'
  if (column.format === 'time' || column.format === 'ago') return 'w-[144px]'
  if (column.format === 'instant') return 'w-[168px]'
  if (isNumeric(column)) return 'w-[88px]'
  return undefined
}

type BodyKind = 'table' | 'settings' | 'card' | 'providers' | 'rules'

const STATE_COLUMN: Column = { key: 'state', labelKey: 'platform.col.state', chip: true }
const STATUS_COLUMN: Column = { key: 'status', labelKey: 'platform.col.state', chip: true }
const DETAIL_COLUMN: Column = { key: 'detail', labelKey: 'platform.col.detail' }
const CREATED_COLUMN: Column = { key: 'created', labelKey: 'platform.col.created', muted: true }
const UPDATED_COLUMN: Column = { key: 'updated', labelKey: 'platform.col.updated', muted: true, format: 'time' }
const SHELF_UPDATED_COLUMN: Column = { key: 'updated', labelKey: 'platform.col.updated', muted: true }
const CATEGORY_COLUMN: Column = { key: 'category', labelKey: 'platform.col.category', select: true }
const INSTALLS_COLUMN: Column = { key: 'installs', labelKey: 'platform.col.installs', width: 'w-[80px]' }

function viewFor(
  menu: PlatformMenuId,
  facet?: string,
  smsView?: string
): { body: BodyKind; nameLabelKey: string; columns: Column[]; cardTitleKey?: string } {
  if (menu === 'signIn' && facet === 'captcha') {
    return { body: 'rules', nameLabelKey: 'platform.col.name', columns: [] }
  }
  if (menu === 'signIn' && facet === 'sms') {
    if (smsView === 'records') {
      return {
        body: 'table',
        nameLabelKey: 'platform.col.name',
        columns: [
          { key: 'state', labelKey: 'platform.col.state', chip: true },
          { key: 'profile', labelKey: 'platform.signIn.sms.profiles' },
          { key: 'detail', labelKey: 'platform.col.detail', muted: true }
        ]
      }
    }
    if (smsView === 'providers') {
      return {
        body: 'table',
        nameLabelKey: 'platform.col.name',
        columns: [DETAIL_COLUMN, STATUS_COLUMN]
      }
    }
    if (smsView === 'profiles') {
      return {
        body: 'table',
        nameLabelKey: 'platform.col.name',
        columns: [
          { key: 'provider', labelKey: 'platform.signIn.sms.providers' },
          STATUS_COLUMN,
          { key: 'signature', labelKey: 'platform.signIn.sms.signatures' },
          { key: 'scenario', labelKey: 'platform.signIn.sms.scenarios' }
        ]
      }
    }
    return { body: 'table', nameLabelKey: 'platform.col.name', columns: [DETAIL_COLUMN, STATUS_COLUMN] }
  }
  if (menu === 'signIn') return { body: 'providers', nameLabelKey: 'platform.col.name', columns: [] }
  if (menu === 'system' && (facet === 'bootstrap' || facet === 'runtime')) {
    return { body: 'settings', nameLabelKey: 'platform.col.name', columns: [] }
  }
  if (menu === 'system' && facet === 'debug') {
    return { body: 'card', nameLabelKey: 'platform.col.name', columns: [], cardTitleKey: 'platform.system.debug' }
  }
  if (menu === 'system' && facet === 'reference') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'code', labelKey: 'platform.col.code' },
        { key: 'detail', labelKey: 'platform.col.detail' }
      ]
    }
  }
  if (menu === 'system' && facet === 'apps') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.version',
      columns: [
        { key: 'platform', labelKey: 'platform.col.platform', filter: true },
        { key: 'file', labelKey: 'platform.col.file' },
        { key: 'size', labelKey: 'platform.col.size', format: 'bytes' },
        STATE_COLUMN,
        { key: 'uploaded', labelKey: 'platform.col.uploaded', muted: true }
      ]
    }
  }
  if (menu === 'customers') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'kind', labelKey: 'platform.col.kind', filter: true },
        { key: 'plan', labelKey: 'platform.col.plan', select: true },
        { key: 'seats', labelKey: 'platform.col.seats' },
        { key: 'owner', labelKey: 'platform.col.owner' },
        { ...STATUS_COLUMN, filter: true },
        CREATED_COLUMN
      ]
    }
  }
  if (menu === 'suppliers') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'kind', labelKey: 'platform.col.kind', filter: true },
        { key: 'protocol', labelKey: 'platform.col.protocol' },
        { key: 'models', labelKey: 'platform.col.models' },
        { key: 'credential', labelKey: 'platform.col.credential' },
        STATUS_COLUMN,
        CREATED_COLUMN
      ]
    }
  }
  if (menu === 'models') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'brand', labelKey: 'platform.col.brand', select: true },
        { key: 'context', labelKey: 'platform.col.context', format: 'tokens', width: 'w-[96px]' },
        { key: 'price', labelKey: 'platform.col.price' },
        { key: 'routes', labelKey: 'platform.col.routes', width: 'w-[72px]' },
        { key: 'served', labelKey: 'platform.col.served' },
        { key: 'state', labelKey: 'platform.col.supply', chip: true, filter: true }
      ]
    }
  }
  if (menu === 'gateway' && facet === 'platform') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'path', labelKey: 'platform.col.match' },
        { key: 'target', labelKey: 'platform.col.target' },
        { key: 'requests', labelKey: 'platform.col.requests', width: 'w-[96px]' },
        { key: 'errors', labelKey: 'platform.col.errors', width: 'w-[80px]' },
        { key: 'latency', labelKey: 'platform.col.latency', format: 'duration', width: 'w-[96px]' },
        { key: 'backends', labelKey: 'platform.col.backends', width: 'w-[88px]' }
      ]
    }
  }
  if (menu === 'gateway' && facet === 'standards') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.standard',
      columns: [
        { key: 'plane', labelKey: 'platform.col.plane', select: true },
        { key: 'coverage', labelKey: 'platform.col.coverage', chip: true, filter: true },
        { key: 'paths', labelKey: 'platform.col.paths' }
      ]
    }
  }
  if (menu === 'gateway') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.host',
      columns: [
        { key: 'path', labelKey: 'platform.col.match' },
        { key: 'target', labelKey: 'platform.col.target' },
        { key: 'project', labelKey: 'platform.col.project', muted: true },
        { key: 'replicas', labelKey: 'platform.col.replicas', width: 'w-[80px]' },
        { key: 'health', labelKey: 'platform.col.health', chip: true },
        { ...STATE_COLUMN, filter: true }
      ]
    }
  }
  if (menu === 'logs') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.message',
      columns: [
        { key: 'time', labelKey: 'platform.col.time', muted: true, format: 'instant', width: 'w-[168px]' },
        { key: 'level', labelKey: 'platform.col.level', chip: true, noFilter: true, width: 'w-[88px]' },
        { key: 'service', labelKey: 'platform.col.service', select: true, width: 'w-[140px]' }
      ]
    }
  }
  if (menu === 'usage') {
    const counts: Column[] = [
      { key: 'tokens', labelKey: 'platform.usage.col.tokens', format: 'tokens', width: 'w-[96px]' },
      { key: 'requests', labelKey: 'platform.usage.col.requests', width: 'w-[96px]' },
      { key: 'success', labelKey: 'platform.usage.col.success', width: 'w-[88px]' },
      { key: 'share', labelKey: 'platform.usage.col.share', muted: true, width: 'w-[80px]' }
    ]
    if (facet === 'models') {
      return {
        body: 'table',
        nameLabelKey: 'platform.usage.col.model',
        columns: [...counts, { key: 'customers', labelKey: 'platform.usage.col.customers', width: 'w-[88px]' }]
      }
    }
    return {
      body: 'table',
      nameLabelKey: 'platform.usage.col.customer',
      columns: [
        ...counts,
        { key: 'top_model', labelKey: 'platform.usage.col.topModel' },
        { key: 'cloud', labelKey: 'platform.usage.col.cloud', filter: true, width: 'w-[104px]' }
      ]
    }
  }
  if (menu === 'orders' && facet === 'subscriptions') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'kind', labelKey: 'platform.col.kind' },
        { key: 'plan', labelKey: 'platform.col.plan', select: true },
        { key: 'seats', labelKey: 'platform.col.seats' },
        { key: 'owner', labelKey: 'platform.col.owner' },
        { key: 'subscription_status', labelKey: 'platform.col.subscription', chip: true, filter: true },
        CREATED_COLUMN
      ]
    }
  }
  if (menu === 'services' && (facet === 'plugins' || facet === 'applets')) {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        CATEGORY_COLUMN,
        { key: 'publisher', labelKey: 'platform.col.publisher' },
        { key: 'version', labelKey: 'platform.col.version', muted: true, width: 'w-[88px]' },
        INSTALLS_COLUMN,
        { ...STATUS_COLUMN, filter: true },
        SHELF_UPDATED_COLUMN
      ]
    }
  }
  if (menu === 'services' && facet === 'skills') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        CATEGORY_COLUMN,
        { key: 'version', labelKey: 'platform.col.version', muted: true, width: 'w-[88px]' },
        { key: 'author', labelKey: 'platform.col.author' },
        INSTALLS_COLUMN,
        { ...STATUS_COLUMN, filter: true },
        SHELF_UPDATED_COLUMN
      ]
    }
  }
  if (menu === 'services' && facet === 'connectors') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        CATEGORY_COLUMN,
        { key: 'provider', labelKey: 'platform.col.provider' },
        { key: 'deployment', labelKey: 'platform.col.deployment', filter: true },
        { key: 'tools', labelKey: 'platform.col.tools', width: 'w-[72px]' },
        INSTALLS_COLUMN,
        STATUS_COLUMN,
        SHELF_UPDATED_COLUMN
      ]
    }
  }
  if (menu === 'services') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        CATEGORY_COLUMN,
        { key: 'protocol', labelKey: 'platform.col.protocol' },
        { key: 'version', labelKey: 'platform.col.version', muted: true },
        { ...STATUS_COLUMN, filter: true },
        SHELF_UPDATED_COLUMN
      ]
    }
  }
  if (menu === 'workloads' && facet === 'workflows') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'health', labelKey: 'platform.col.health', chip: true, filter: true },
        { key: 'replicas', labelKey: 'platform.col.replicas', width: 'w-[80px]' },
        { key: 'cpu', labelKey: 'platform.col.cpu', width: 'w-[72px]' },
        { key: 'memory', labelKey: 'platform.col.memory', format: 'bytes', width: 'w-[88px]' },
        { key: 'project', labelKey: 'platform.col.project' },
        { key: 'environment', labelKey: 'platform.col.environment', select: true }
      ]
    }
  }
  if (menu === 'workloads' && facet === 'functions') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'version', labelKey: 'platform.col.release', muted: true, width: 'w-[96px]' },
        { key: 'mode', labelKey: 'platform.col.mode', filter: true },
        { key: 'concurrency', labelKey: 'platform.col.concurrency', width: 'w-[80px]' },
        { key: 'timeout', labelKey: 'platform.col.timeout', format: 'duration', width: 'w-[88px]' },
        STATE_COLUMN,
        UPDATED_COLUMN
      ]
    }
  }
  if (menu === 'workloads' && facet === 'cells') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { ...STATE_COLUMN, filter: true },
        { key: 'revision', labelKey: 'platform.col.revision', muted: true, width: 'w-[80px]' },
        { key: 'project', labelKey: 'platform.col.project' },
        { key: 'environment', labelKey: 'platform.col.environment', select: true },
        UPDATED_COLUMN
      ]
    }
  }
  if (menu === 'workloads' && facet === 'runtimes') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.execution',
      columns: [
        { ...STATUS_COLUMN, filter: true },
        { key: 'bound', labelKey: 'platform.col.bound', width: 'w-[112px]' },
        { key: 'conversation', labelKey: 'platform.col.conversation', muted: true },
        { key: 'operation', labelKey: 'platform.col.operation', muted: true }
      ]
    }
  }
  if (menu === 'workloads') return { body: 'table', nameLabelKey: 'platform.col.name', columns: [DETAIL_COLUMN] }
  if (menu === 'fleet' && facet === 'topology') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'nodes', labelKey: 'platform.col.nodes' },
        { key: 'online', labelKey: 'platform.col.online' },
        { key: 'maintenance', labelKey: 'platform.col.maintenance', chip: true },
        { key: 'version', labelKey: 'platform.col.version', muted: true },
        UPDATED_COLUMN
      ]
    }
  }
  if (menu === 'fleet') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        STATE_COLUMN,
        { key: 'availability', labelKey: 'platform.col.availability', chip: true },
        { key: 'pool', labelKey: 'platform.col.pool', select: true },
        { key: 'version', labelKey: 'platform.col.version', muted: true },
        { key: 'observed', labelKey: 'platform.col.observed', muted: true, format: 'ago' }
      ]
    }
  }
  if (menu === 'channels') {
    return {
      body: 'table',
      nameLabelKey: 'platform.col.name',
      columns: [
        { key: 'grade', labelKey: 'platform.col.grade', filter: true },
        { key: 'tenant', labelKey: 'platform.col.tenant' },
        { key: 'brand', labelKey: 'platform.col.brand' },
        { key: 'owner', labelKey: 'platform.col.owner' },
        { key: 'contact', labelKey: 'platform.col.contact', muted: true },
        { ...STATUS_COLUMN, filter: true },
        CREATED_COLUMN
      ]
    }
  }
  return { body: 'table', nameLabelKey: 'platform.col.name', columns: [DETAIL_COLUMN] }
}

function CatalogPage({ menu }: { menu: PlatformMenuId }) {
  if (menu === 'suppliers' || menu === 'models')
    return <RemoteCatalogPage key={`${readCenterMemberId()}:${readCenterId()}:${menu}`} resource={menu} />
  const resource = MENU_RESOURCE[menu] ?? menu
  const view = viewFor(menu)
  return (
    <RecordPage
      titleKey={`platform.nav.${menu}`}
      descriptionKey={`platform.${menu}.description`}
      marker={`platform.${menu}`}
      swrKey={`studio/platform/${resource}`}
      load={() => listPlatformItems(resource)}
      emptyKey={`platform.${menu}.empty`}
      body={view.body}
      nameLabelKey={view.nameLabelKey}
      columns={view.columns}
      detailExtra={catalogDetail(menu)}
    />
  )
}

/** Supplier/model rows, facets, ordering and totals are owned by the server. */
function RemoteCatalogPage({ resource }: { resource: PlatformCatalogResource }) {
  const { t } = useTranslation()
  const supplier = useSupplierActions()
  const scope = `studio/platform/${readCenterMemberId()}/${readCenterId()}/${resource}/page`
  const memory = useListMemory()
  const [state, setState] = useState(() => memory.get(scope) ?? freshList(scope))
  const list = state.scope === scope ? state : (memory.get(scope) ?? freshList(scope))
  const update = (patch: Partial<ListState>) => {
    const next = { ...list, ...patch }
    memory.set(scope, next)
    setState(next)
  }
  const query = {
    page: list.page,
    limit: list.pageSize,
    q: list.query,
    filter: list.filter,
    brand: resource === 'models' ? list.choice : '',
    sort: list.sort?.key ?? '',
    direction: list.sort?.direction === 'descending' ? ('desc' as const) : ('asc' as const)
  }
  const key = JSON.stringify([
    scope,
    query.page,
    query.limit,
    query.q.trim(),
    query.filter,
    query.brand,
    query.sort,
    query.direction
  ])
  const admittedKey = resource === 'suppliers' && !supplier?.access ? null : key
  const {
    data: fetched,
    error,
    isLoading,
    isValidating,
    mutate
  } = useSWR(admittedKey, () => readPlatformCatalogPage(resource, query), {
    onSuccess: () => markLoaded(key)
  })
  // A write's reconcile drops the cached page before re-reading it; the same query keeps its rows meanwhile.
  const [held, setHeld] = useState<{ key: string; page: typeof fetched } | null>(null)
  if (fetched && held?.page !== fetched) setHeld({ key, page: fetched })
  const data = fetched ?? (!error && held?.key === key ? held.page : undefined)
  const optimistic = useOptimisticRows(scope, data?.rows ?? NO_ROWS, itemId)
  const total = Math.max(0, (data?.total ?? 0) - optimistic.removed)
  const busy = isLoading || isValidating
  const [searchGeneration, setSearchGeneration] = useState(0)
  useEffect(() => {
    if (error || !data || data.page === list.page) return
    const next = { ...list, page: data.page, openId: null }
    memory.set(scope, next)
    setState(next)
  }, [data, error, list, memory, scope])
  const view = useMemo(() => viewFor(resource), [resource])
  const filterKey = resource === 'suppliers' ? 'kind' : 'state'
  const filterColumn = view.columns.find((column) => column.key === filterKey)!
  const filters = [...(data?.facets[filterKey] ?? [])]
  if (list.filter && !filters.some((option) => option.value === list.filter))
    filters.push({ value: list.filter, count: 0 })
  const brands = [...(data?.facets.brand ?? [])]
  if (list.choice && !brands.some((option) => option.value === list.choice))
    brands.push({ value: list.choice, count: 0 })
  const narrowed = Boolean(list.query.trim() || list.filter || list.choice)
  const sortable = new Set(
    resource === 'suppliers' ? ['name', 'created', 'kind', 'protocol'] : ['name', 'brand', 'context', 'state']
  )
  const reset = () => {
    setSearchGeneration((generation) => generation + 1)
    update({ query: '', filter: '', choice: '', page: 1, openId: null })
  }
  const changeView = (patch: Partial<ListState>) => update({ ...patch, page: 1, openId: null })
  const rowActions = useMemo(() => {
    if (resource !== 'suppliers' || !supplier?.access?.read) return undefined
    const blocked = supplier.busy || supplier.resumable
    const writes = [
      supplier.access.update ? ('edit' as const) : null,
      supplier.access.delete ? ('delete' as const) : null
    ].filter((mode) => mode !== null)
    return (item: PlatformItem) => (
      <div className="flex items-center justify-end gap-1">
        <Button
          size="sm"
          variant="ghost"
          disabled={blocked}
          aria-label={t('platform.supplierForm.readAction', { name: item.name })}
          onClick={() => supplier.open('read', item.id)}>
          {t('platform.supplierForm.read')}
        </Button>
        {writes.length ? (
          <RowActionMenu
            label={t('platform.supplierForm.moreAction', { name: item.name })}
            disabled={blocked}
            actions={writes.map((mode) => ({
              key: mode,
              label: t(mode === 'edit' ? 'platform.supplierForm.edit' : 'platform.supplierForm.delete'),
              accessibleName: t(
                mode === 'edit' ? 'platform.supplierForm.editAction' : 'platform.supplierForm.deleteAction',
                { name: item.name }
              ),
              danger: mode === 'delete',
              onSelect: () => supplier.open(mode, item.id)
            }))}
          />
        ) : null}
      </div>
    )
  }, [resource, supplier, t])
  const selection = useCollectionSelection(!error && !supplier?.authorityError ? optimistic.rows : NO_ROWS, key)
  const selectionOpen = selection.ids.length > 0
  const selectionHold = useRef<readonly string[]>([])
  const selectionPresence = useRestingDialog(selectionOpen)
  if (selectionOpen) selectionHold.current = selection.ids
  const shownSelectionIds = selectionOpen ? selection.ids : selectionHold.current
  let content: ReactNode
  if (resource === 'suppliers' && supplier?.authorityError)
    content = (
      <FailureNotice
        marker="platform.suppliers.authority"
        error={supplier.authorityError}
        onRetry={supplier.retryAuthority}
      />
    )
  else if (error)
    content = <FailureNotice marker={`platform.${resource}`} error={error} onRetry={() => void mutate()} />
  else if (!data)
    content = (
      <div className="relative min-h-40" aria-busy="true">
        <PageEntry id={`platform.${resource}`} ready={false} />
        <ListSkeleton />
      </div>
    )
  else if (optimistic.rows.length === 0)
    content = (
      <NoticeLine
        marker={`platform.${resource}`}
        icon={narrowed ? SearchX : Inbox}
        text={
          list.query.trim()
            ? t('platform.nomatchQuery', { query: list.query.trim() })
            : t(narrowed ? 'platform.nomatch' : `platform.${resource}.empty`)
        }
        action={
          narrowed ? (
            <button type="button" className={textButtonClassName} onClick={reset}>
              {t('platform.clearFilters')}
            </button>
          ) : undefined
        }
      />
    )
  else
    content = (
      <div className="overflow-x-auto">
        <RecordTable
          marker={`platform.${resource}`}
          label={t(`platform.nav.${resource}`)}
          rows={optimistic.rows}
          motion={optimistic.motion}
          selection={selection}
          nameLabelKey={view.nameLabelKey}
          columns={view.columns}
          sort={list.sort}
          sortableKeys={sortable}
          onSort={(key) =>
            changeView({
              sort:
                list.sort?.key !== key
                  ? { key, direction: 'ascending' }
                  : list.sort.direction === 'ascending'
                    ? { key, direction: 'descending' }
                    : null
            })
          }
          openId={list.openId}
          onToggle={(id) => update({ openId: list.openId === id ? null : id })}
          detailExtra={catalogDetail(resource)}
          rowActions={rowActions}
        />
      </div>
    )
  return (
    <PlatformFrame
      title={t(`platform.nav.${resource}`)}
      description={t(`platform.${resource}.description`)}
      overlay={
        selectionPresence.present ? (
          <CollectionSelectionBar ids={shownSelectionIds} state={selectionPresence.state} onClear={selection.clear} />
        ) : undefined
      }>
      <CollectionToolbar
        label={t('collection.controls', { name: t(`platform.nav.${resource}`) })}
        search={
          <CatalogSearch
            key={searchGeneration}
            resource={resource}
            committed={list.query}
            onCommit={(value) => changeView({ query: value })}
          />
        }
        filters={
          !error && (data || narrowed) ? (
            <>
              {resource === 'models' && (brands.length > 1 || list.choice) ? (
                <Combobox
                  size="sm"
                  className={choiceFieldClassName}
                  aria-label={t('platform.col.brand')}
                  value={list.choice || ALL_CHOICE}
                  options={[
                    { value: ALL_CHOICE, label: t('common.all') },
                    ...brands.map((option) => ({ value: option.value, label: option.value, count: option.count }))
                  ]}
                  onChange={(value) => {
                    const next = Array.isArray(value) ? value[0] : value
                    changeView({ choice: next === ALL_CHOICE ? '' : (next ?? '') })
                  }}
                />
              ) : null}
              {filters.length > 1 || list.filter ? (
                <SegmentedControl
                  size="sm"
                  className="h-[30px]"
                  aria-label={t(filterColumn.labelKey)}
                  value={list.filter}
                  onValueChange={(value) => changeView({ filter: value })}
                  options={[
                    { value: '', label: t('common.all') },
                    ...filters.map((option) => ({
                      value: option.value,
                      label: (
                        <FilterLabel text={cellText(filterColumn, filterKey, option.value, t)} count={option.count} />
                      )
                    }))
                  ]}
                />
              ) : null}
            </>
          ) : null
        }
        actions={
          <>
            <RefreshButton busy={busy} updatedAt={data ? loadedAt.get(key) : undefined} onClick={() => void mutate()} />
            {resource === 'suppliers' && supplier?.access?.create ? (
              <Button
                data-ui="platform.supplier.create"
                size="sm"
                disabled={supplier.busy || (supplier.recovering && !supplier.resumable)}
                title={
                  supplier.recovering && !supplier.resumable ? t('platform.supplierForm.recoveryRequired') : undefined
                }
                onClick={() => supplier.open('create')}>
                {t(supplier.recovering ? 'platform.supplierForm.resume' : 'platform.supplierForm.createTitle')}
              </Button>
            ) : null}
          </>
        }
        summary={
          !error ? (
            <>
              {data ? (
                <span role="status" className="text-[12px] text-[#8a8a8a]">
                  {t('platform.total', { total })}
                </span>
              ) : null}
            </>
          ) : null
        }
      />
      {/* Rows here always belong to the current query, so a same-query refresh keeps them readable instead of dimming. */}
      <div data-ui="platform.list" aria-busy={busy || undefined}>
        {content}
      </div>
      {data && !error ? (
        <CollectionPagination
          page={data.page}
          pageSize={list.pageSize}
          total={total}
          busy={busy}
          onPageChange={(page) => update({ page, openId: null })}
          onPageSizeChange={(pageSize) => changeView({ pageSize })}
        />
      ) : null}
    </PlatformFrame>
  )
}

type RowAction = { key: string; label: string; accessibleName: string; danger?: boolean; onSelect: () => void }

/** The 24px row action holding secondary record actions; destructive entries come last. */
function RowActionMenu({ label, disabled, actions }: { label: string; disabled: boolean; actions: RowAction[] }) {
  const trigger = useRef<HTMLButtonElement>(null)
  const chosen = useRef<RowAction | null>(null)
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          ref={trigger}
          type="button"
          aria-label={label}
          title={label}
          disabled={disabled}
          className="flex size-6 shrink-0 items-center justify-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
          <MoreHorizontal className="size-3.5 text-current" strokeWidth={1.7} aria-hidden />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        sideOffset={6}
        aria-label={label}
        onCloseAutoFocus={(event) => {
          const action = chosen.current
          chosen.current = null
          if (!action) return
          // Run once the menu has released focus, so a dialog opened here owns focus and returns it to the trigger.
          event.preventDefault()
          trigger.current?.focus()
          action.onSelect()
        }}
        className="min-w-[128px] rounded-[12px] border border-[#ececec] bg-white p-1 text-[#3f3f3f] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
        {actions.map((action) => (
          <DropdownMenuItem
            key={action.key}
            aria-label={action.accessibleName}
            onSelect={() => {
              chosen.current = action
            }}
            className={cn(
              'h-8 rounded-[8px] px-2.5 text-[13px]',
              action.danger ? 'text-[#b42318] focus:text-[#b42318]' : 'text-[#3f3f3f]'
            )}>
            {action.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

/** Owns the draft text, so typing re-renders only the field; the list sees committed queries. */
function CatalogSearch({
  resource,
  committed,
  onCommit
}: {
  resource: PlatformCatalogResource
  committed: string
  onCommit: (value: string) => void
}) {
  const { t } = useTranslation()
  const searchRef = useRef<HTMLInputElement>(null)
  const [search, setSearch] = useState(committed)
  const [composing, setComposing] = useState(false)
  const commitRef = useRef(onCommit)
  useLayoutEffect(() => {
    commitRef.current = onCommit
  })
  useSlashToSearch(searchRef)
  useEffect(() => {
    if (composing || search === committed) return
    const timer = setTimeout(() => commitRef.current(search), 300)
    return () => clearTimeout(timer)
  }, [composing, search, committed])
  const commit = (value: string) => {
    setSearch(value)
    onCommit(value)
  }
  return (
    <SearchInput
      ref={searchRef}
      size="sm"
      value={search}
      maxLength={128}
      placeholder={t(
        resource === 'suppliers' ? 'platform.suppliers.searchPlaceholder' : 'platform.models.searchPlaceholder'
      )}
      aria-label={t('common.search')}
      aria-description={t(
        resource === 'suppliers' ? 'platform.suppliers.searchDescription' : 'platform.models.searchDescription'
      )}
      aria-keyshortcuts="/"
      title={t('platform.searchShortcut')}
      containerClassName={searchFieldClassName}
      onChange={(event) => setSearch(event.target.value)}
      onCompositionStart={() => setComposing(true)}
      onCompositionEnd={(event) => {
        setSearch(event.currentTarget.value)
        setComposing(false)
      }}
      onClear={() => commit('')}
      clearLabel={t('common.clear')}
      onKeyDown={(event) => {
        if (composing || event.nativeEvent.isComposing) return
        if (event.key === 'Enter') commit(search)
        if (event.key === 'Escape' && search) {
          event.preventDefault()
          commit('')
        }
      }}
    />
  )
}

const CATALOG_DETAILS: Partial<Record<PlatformMenuId, (item: PlatformItem) => ReactNode>> = {
  customers: (item) => <CustomerPeople customerId={item.id} />,
  suppliers: (item) => <SupplierModels supplierId={item.id} />,
  models: (item) => <ModelRoutes modelId={item.id} />
}

function catalogDetail(menu: PlatformMenuId) {
  return CATALOG_DETAILS[menu]
}

const LIVE_REFRESH_MS = 10_000

function LogsPage() {
  const { t } = useTranslation()
  const [logWindow, setLogWindow] = usePlatformFacet('logs.window', LOG_WINDOWS, '1h')
  const [level, setLevel] = useState<LogLevel>('all')
  const [live, setLive] = useState(false)
  const view = viewFor('logs')
  const refreshInterval = live ? LIVE_REFRESH_MS : 0
  const wider = LOG_WINDOWS[LOG_WINDOWS.indexOf(logWindow) + 1]
  return (
    <RecordPage
      titleKey="platform.nav.logs"
      descriptionKey="platform.logs.description"
      emptyKey="platform.logs.empty"
      problemsFirst={false}
      emptyAction={
        wider ? (
          <button type="button" className={textButtonClassName} onClick={() => setLogWindow(wider)}>
            {t('platform.logs.widen', { window: t(`platform.logs.window.${wider}`) })}
          </button>
        ) : undefined
      }
      marker="platform.logs"
      swrKey={`studio/platform/logs/${logWindow}/${level}`}
      listScope="studio/platform/logs"
      refreshInterval={refreshInterval}
      load={() => listPlatformLogs(logWindow, level)}
      below={<LogLevels logWindow={logWindow} level={level} onLevel={setLevel} refreshInterval={refreshInterval} />}
      body={view.body}
      nameLabelKey={view.nameLabelKey}
      columns={view.columns}
      segments={
        <>
          <SegmentedControl
            size="sm"
            className="h-[30px]"
            aria-label={t('platform.logs.window')}
            value={logWindow}
            onValueChange={setLogWindow}
            options={LOG_WINDOWS.map((value) => ({ value, label: t(`platform.logs.window.${value}`) }))}
          />
          <button
            type="button"
            aria-pressed={live}
            title={t('platform.logs.liveHint')}
            className={cn(
              'flex h-[30px] shrink-0 items-center gap-1.5 rounded-[8px] px-2.5 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10',
              live
                ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                : 'text-[#8a8a8a] hover:bg-[#f5f5f5] hover:text-[#171717]'
            )}
            onClick={() => setLive((current) => !current)}>
            <span
              aria-hidden
              className={cn('size-1.5 rounded-full', live ? 'animate-pulse bg-[#171717]' : 'bg-[#c4c4c4]')}
            />
            {t('platform.logs.live')}
          </button>
        </>
      }
    />
  )
}

/** Counts cover the whole window, so the strip stays honest when the list holds only the newest page. */
function LogLevels({
  logWindow,
  level,
  onLevel,
  refreshInterval
}: {
  logWindow: LogWindow
  level: LogLevel
  onLevel: (level: LogLevel) => void
  refreshInterval: number
}) {
  const { t } = useTranslation()
  const { data, isLoading } = useSWR(
    `studio/platform/logs/levels/${logWindow}`,
    () => listPlatformLogLevels(logWindow),
    {
      keepPreviousData: true,
      refreshInterval
    }
  )
  const counts = new Map((data ?? []).map((row) => [row.id, Number(fieldValue(row, 'count'))]))
  const countOf = (value: LogLevel) => {
    const count = counts.get(value)
    return count !== undefined && Number.isFinite(count) ? count : undefined
  }
  const shown = LOG_LEVELS.filter((value) => value === 'all' || value === level || (countOf(value) ?? 0) > 0)
  const selected = countOf(level)
  if (shown.length < 2) return null
  return (
    <div
      data-ui="platform.logs.levels"
      aria-busy={isLoading || undefined}
      className={cn('mb-3 grid gap-2 transition-opacity', isLoading && 'opacity-50')}>
      <SegmentedControl
        size="sm"
        className="h-[30px] w-fit"
        aria-label={t('platform.col.level')}
        value={level}
        onValueChange={(value) => onLevel(value)}
        options={shown.map((value) => {
          const text = value === 'all' ? t('common.all') : statusText(value, t)
          const count = countOf(value)
          return { value, label: count === undefined ? text : <FilterLabel text={text} count={count} /> }
        })}
      />
      {selected !== undefined && selected > LOG_PAGE_LIMIT ? (
        <p className="m-0 text-[12px] text-[#8a8a8a]">
          {t('platform.logs.truncated', { shown: LOG_PAGE_LIMIT, total: selected })}
        </p>
      ) : null}
    </div>
  )
}

function usageKey(days: UsageWindow) {
  return `studio/platform/usage/${days}`
}

function UsagePage() {
  const { t } = useTranslation()
  const [days, setDays] = usePlatformFacet('usage.days', USAGE_WINDOWS, '30')
  const [facet, setFacet] = usePlatformFacet('usage', USAGE_FACETS)
  const view = viewFor('usage', facet)
  return (
    <RecordPage
      titleKey="platform.nav.usage"
      descriptionKey="platform.usage.description"
      marker={`platform.usage.${facet}`}
      swrKey={usageKey(days)}
      listScope={`studio/platform/usage/${facet}`}
      load={() => listPlatformUsage(days)}
      rowFilter={(item) => fieldValue(item, 'section') === facet}
      problemsFirst={false}
      body={view.body}
      nameLabelKey={view.nameLabelKey}
      columns={view.columns}
      emptyKey="platform.usage.empty"
      segments={
        <>
          <SegmentedControl
            size="sm"
            className="h-[30px]"
            aria-label={t('platform.usage.facet')}
            value={facet}
            onValueChange={setFacet}
            options={USAGE_FACETS.map((id) => ({ value: id, label: t(`platform.usage.${id}`) }))}
          />
          <SegmentedControl
            size="sm"
            className="h-[30px]"
            aria-label={t('platform.usage.window')}
            value={days}
            onValueChange={setDays}
            options={USAGE_WINDOWS.map((value) => ({ value, label: t(`platform.usage.window.${value}`) }))}
          />
        </>
      }
      below={<UsageSummary days={days} />}
    />
  )
}

function usageTotals(data: PlatformItem[]) {
  const series = data.filter((item) => fieldValue(item, 'section') === 'days')
  const customers = data.filter((item) => fieldValue(item, 'section') === 'customers')
  const models = data.filter((item) => fieldValue(item, 'section') === 'models')
  const sum = (key: string) => series.reduce((total, item) => total + (Number(fieldValue(item, key)) || 0), 0)
  const requests = sum('requests')
  const failed = sum('failed')
  return {
    series,
    customers,
    models,
    tokens: sum('tokens'),
    requests,
    failed,
    success: requests > 0 ? `${(((requests - failed) / requests) * 100).toFixed(1)}%` : '—',
    active: customers.filter((item) => Number(fieldValue(item, 'requests')) > 0).length,
    partial: customers.filter((item) => fieldValue(item, 'cloud') === 'unavailable').length
  }
}

function UsageSummary({ days }: { days: UsageWindow }) {
  const { t } = useTranslation()
  const { data, isLoading } = useSWR(usageKey(days), () => listPlatformUsage(days), { keepPreviousData: true })
  if (!data) return null
  const { series, models, tokens, requests, failed, success, active, partial } = usageTotals(data)
  const stats = [
    {
      key: 'tokens',
      label: t('platform.usage.stat.tokens'),
      value: formatTokens(String(tokens)),
      hint: t('platform.usage.stat.models', { value: models.length })
    },
    {
      key: 'requests',
      label: t('platform.usage.stat.requests'),
      value: requests.toLocaleString(),
      hint: failed > 0 ? t('platform.usage.stat.failed', { value: failed.toLocaleString() }) : ''
    },
    {
      key: 'success',
      label: t('platform.usage.stat.success'),
      value: success,
      hint: ''
    },
    {
      key: 'customers',
      label: t('platform.usage.stat.customers'),
      value: String(active),
      hint: ''
    }
  ]
  return (
    <div
      data-ui="platform.usage.summary"
      aria-busy={isLoading || undefined}
      className={cn('mb-4 grid gap-3 transition-opacity', isLoading && 'opacity-50')}>
      <dl className="m-0 grid grid-cols-2 gap-3 lg:grid-cols-4">
        {stats.map((stat) => (
          <div key={stat.key} className="min-w-0 rounded-[12px] border border-[#efefef] bg-white px-3.5 py-3">
            <dt className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{stat.label}</dt>
            <dd className="m-0 mt-1.5 flex h-6 items-center text-[18px] leading-6 font-semibold text-[#1a1a1a]">
              {stat.value}
            </dd>
            <dd className="m-0 mt-1 truncate text-[12px] leading-4 text-[#8a8a8a]" title={stat.hint}>
              {stat.hint || '\u00a0'}
            </dd>
          </div>
        ))}
      </dl>
      {partial > 0 ? (
        <p data-ui="platform.usage.partial" className="m-0 text-[13px] text-[#8a8a8a]">
          {t('platform.usage.partial', { value: partial })}
        </p>
      ) : null}
      {tokens > 0 ? <UsageBars series={series} /> : null}
    </div>
  )
}

function UsageBars({ series }: { series: PlatformItem[] }) {
  const { t } = useTranslation()
  const points = series.map((item) => ({ day: item.name, tokens: Number(fieldValue(item, 'tokens')) || 0 }))
  const peak = points.reduce((max, point) => Math.max(max, point.tokens), 0)
  const labelEvery = points.length <= 8 ? 1 : points.length <= 31 ? 5 : 14
  return (
    <section data-ui="platform.usage.chart" className="rounded-[12px] border border-[#efefef] bg-white px-3.5 py-3">
      <p className={detailLabelClassName}>{t('platform.usage.chart')}</p>
      <div className="mt-2 flex h-32 items-end gap-px" role="img" aria-label={t('platform.usage.chart')}>
        {points.map((point) => (
          <div
            key={point.day}
            className="flex h-full min-w-0 flex-1 items-end"
            title={`${point.day} · ${formatTokens(String(point.tokens))}`}>
            <div
              className={cn('w-full rounded-t-[2px]', point.tokens > 0 ? 'bg-[#3f3f3f]' : 'bg-[#efefef]')}
              style={{ height: point.tokens > 0 ? `${Math.max(2, Math.round((point.tokens / peak) * 100))}%` : '1px' }}
            />
          </div>
        ))}
      </div>
      <div className="mt-1 flex gap-px text-[11px] text-[#8a8a8a]">
        {points.map((point, index) => (
          <span key={point.day} className="min-w-0 flex-1 truncate text-center">
            {index % labelEvery === 0 ? point.day.slice(5) : ''}
          </span>
        ))}
      </div>
    </section>
  )
}

function GatewayPage() {
  const { t } = useTranslation()
  const [facet, setFacet] = usePlatformFacet('gateway', GATEWAY_FACETS)
  const view = viewFor('gateway', facet)
  return (
    <RecordPage
      titleKey="platform.nav.gateway"
      descriptionKey="platform.gateway.description"
      emptyKey="platform.gateway.empty"
      marker={`platform.gateway.${facet}`}
      swrKey={facet === 'routes' ? 'studio/platform/gateway' : `studio/platform/gateway/${facet}`}
      load={() => listPlatformItems('gateway', facet === 'routes' ? undefined : facet)}
      body={view.body}
      nameLabelKey={view.nameLabelKey}
      columns={view.columns}
      segments={
        <SegmentedControl
          size="sm"
          className="h-[30px]"
          aria-label={t('platform.nav.gateway')}
          value={facet}
          onValueChange={setFacet}
          options={GATEWAY_FACETS.map((id) => ({ value: id, label: t(`platform.gateway.${id}`) }))}
        />
      }
      below={<GatewayStats />}
      detailExtra={facet === 'routes' ? (item) => <GatewayChain route={item} /> : undefined}
    />
  )
}

function ChainHop({
  label,
  title,
  detail,
  children
}: {
  label: string
  title: string
  detail?: string
  children?: ReactNode
}) {
  return (
    <div className="max-w-[240px] min-w-[148px] rounded-[8px] bg-[#f7f7f7] px-3 py-2">
      <p className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{label}</p>
      <p className="m-0 mt-0.5 truncate text-[13px] font-medium text-[#1a1a1a]" title={title}>
        {title || '—'}
      </p>
      {detail ? (
        <p className={cn(digestClassName, 'm-0 mt-0.5')} title={detail}>
          {detail}
        </p>
      ) : null}
      {children}
    </div>
  )
}

function GatewayChain({ route }: { route: PlatformItem }) {
  const { t } = useTranslation()
  const members = fieldValue(route, 'members')
  const replicas = fieldValue(route, 'replicas')
  const health = fieldValue(route, 'health')
  const hops = [
    <ChainHop
      key="entry"
      label={t('platform.gateway.chain.entry')}
      title={route.name}
      detail={fieldValue(route, 'path')}
    />,
    <ChainHop
      key="node"
      label={t('platform.gateway.chain.node')}
      title={fieldValue(route, 'gateway_node')}
      detail={members ? t('platform.gateway.chain.members', { value: members }) : undefined}
    />,
    <ChainHop
      key="workload"
      label={t('platform.gateway.chain.workload')}
      title={fieldValue(route, 'workload')}
      detail={replicas ? t('platform.gateway.chain.replicas', { value: replicas }) : undefined}>
      {health ? (
        <span className="mt-1 inline-flex">
          <StatusChip value={health} />
        </span>
      ) : null}
    </ChainHop>,
    <ChainHop
      key="upstream"
      label={t('platform.gateway.chain.upstream')}
      title={fieldValue(route, 'upstream')}
      detail={fieldValue(route, 'port')}
    />
  ]
  return (
    <section data-ui="platform.gateway.chain" className="min-w-0">
      <p className={detailLabelClassName}>{t('platform.gateway.chain.title')}</p>
      <ol className="m-0 mt-1 flex list-none flex-wrap items-center gap-1.5 p-0">
        {hops.map((hop, index) => (
          <li key={hop.key} className="flex items-center gap-1.5">
            {index > 0 ? (
              <ChevronRight className="size-3.5 shrink-0 text-[#b0b0b0]" strokeWidth={1.7} aria-hidden />
            ) : null}
            {hop}
          </li>
        ))}
      </ol>
    </section>
  )
}

function formatUptime(value: string) {
  const seconds = Number(value)
  if (!value || !Number.isFinite(seconds) || seconds < 0) return '—'
  const days = Math.floor(seconds / 86_400)
  const hours = Math.floor((seconds % 86_400) / 3_600)
  const minutes = Math.floor((seconds % 3_600) / 60)
  if (days > 0) return `${days}d ${hours}h`
  if (hours > 0) return `${hours}h ${minutes}m`
  if (minutes > 0) return `${minutes}m`
  return `${seconds}s`
}

function GatewayStats() {
  const { t } = useTranslation()
  const { data } = useSWR('studio/platform/gateway/live', () => listPlatformItems('gateway', 'live'), {
    refreshInterval: LIVE_REFRESH_MS
  })
  const live = data?.[0]
  if (!live) return null
  const errors = fieldValue(live, 'errors')
  const connections = fieldValue(live, 'connections')
  const stats: { key: string; label: string; value: ReactNode; hint: string }[] = [
    {
      key: 'state',
      label: t('platform.gateway.stat.state'),
      value: <StatusChip value={fieldValue(live, 'state')} />,
      hint: fieldValue(live, 'notes') || live.name
    },
    {
      key: 'uptime',
      label: t('platform.gateway.stat.uptime'),
      value: formatUptime(fieldValue(live, 'uptime')),
      hint: connections ? t('platform.gateway.stat.connections', { value: formatCount(connections) }) : ''
    },
    {
      key: 'mounted',
      label: t('platform.gateway.stat.mounted'),
      value: fieldValue(live, 'mounted') || '—',
      hint: t('platform.gateway.stat.mountedHint')
    },
    {
      key: 'traffic',
      label: t('platform.gateway.stat.traffic'),
      value: formatCount(fieldValue(live, 'requests')) || '—',
      hint: errors ? t('platform.gateway.stat.errors', { value: formatCount(errors) }) : ''
    }
  ]
  return (
    <dl data-ui="platform.gateway.live" className="m-0 mb-4 grid grid-cols-2 gap-3 lg:grid-cols-4">
      {stats.map((stat) => (
        <div key={stat.key} className="min-w-0 rounded-[12px] border border-[#efefef] bg-white px-3.5 py-3">
          <dt className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{stat.label}</dt>
          <dd className="m-0 mt-1.5 flex h-6 items-center text-[18px] leading-6 font-semibold text-[#1a1a1a]">
            {stat.value}
          </dd>
          <dd className="m-0 mt-1 truncate text-[12px] leading-4 text-[#8a8a8a]" title={stat.hint}>
            {stat.hint || '\u00a0'}
          </dd>
        </div>
      ))}
    </dl>
  )
}

function FacetPage({
  menu,
  facets,
  facetPrefix
}: {
  menu: PlatformMenuId
  facets: readonly string[]
  facetPrefix: string
}) {
  const { t } = useTranslation()
  const [facet, setFacet] = usePlatformFacet(menu, facets)
  const [smsView, setSmsView] = usePlatformFacet('signIn.sms', SMS_VIEWS)
  const [pool, setPool] = useState('')
  const [lookup, setLookup] = useState('')
  const resource = menu === 'signIn' ? 'sign-in' : menu
  const kind = menu === 'signIn' && facet === 'sms' ? `sms-${smsView}` : facet
  const view = viewFor(menu, facet, smsView)
  const nodeList = menu === 'fleet' && facet === 'fleet'
  const runtimes = menu === 'workloads' && facet === 'runtimes'
  return (
    <RecordPage
      choice={nodeList ? pool : undefined}
      onChoiceChange={nodeList ? setPool : undefined}
      titleKey={`platform.nav.${menu}`}
      descriptionKey={`${facetPrefix}.description`}
      marker={`platform.${menu}.${facet}`}
      swrKey={runtimes ? `studio/platform/agent-runtimes/${lookup}` : `studio/platform/${resource}/${kind}`}
      load={
        runtimes
          ? () => (lookup ? findAgentRuntimes(lookup) : Promise.resolve([]))
          : () => listPlatformItems(resource, kind)
      }
      searchable={!runtimes}
      emptyKey={runtimes ? (lookup ? 'platform.runtimes.none' : 'platform.runtimes.prompt') : `platform.${menu}.empty`}
      detailExtra={workloadDetail(menu, facet)}
      body={view.body}
      nameLabelKey={view.nameLabelKey}
      columns={view.columns}
      cardTitleKey={view.cardTitleKey}
      segments={
        <SegmentedControl
          size="sm"
          className="h-[30px]"
          aria-label={t(`platform.nav.${menu}`)}
          value={facet}
          onValueChange={setFacet}
          options={facets.map((id) => ({ value: id, label: t(`${facetPrefix}.${id}`) }))}
        />
      }
      below={
        menu === 'signIn' && facet === 'sms' ? (
          <div className="mb-4">
            <SegmentedControl
              size="sm"
              className="h-[30px]"
              aria-label={t('platform.signIn.sms')}
              value={smsView}
              onValueChange={(value) => setSmsView(value)}
              options={SMS_VIEWS.map((id) => ({ value: id, label: t(`platform.signIn.sms.${id}`) }))}
            />
          </div>
        ) : nodeList ? (
          <PoolCards active={pool} onPick={setPool} />
        ) : runtimes ? (
          <RuntimeLookup initial={lookup} onLookup={setLookup} />
        ) : undefined
      }
    />
  )
}

function workloadDetail(menu: PlatformMenuId, facet: string) {
  if (menu !== 'workloads') return undefined
  if (facet === 'functions') return (item: PlatformItem) => <FunctionReleases assetId={item.id} />
  if (facet === 'cells') return (item: PlatformItem) => <CellRevisions cellId={item.id} />
  if (facet === 'runtimes') return (item: PlatformItem) => <RuntimeTrajectory executionId={item.id} />
  return undefined
}

const UUID_TEXT = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

function RuntimeLookup({ initial, onLookup }: { initial: string; onLookup: (id: string) => void }) {
  const { t } = useTranslation()
  const [text, setText] = useState(initial)
  const [invalid, setInvalid] = useState(false)
  const submit = () => {
    const id = text.trim()
    if (id && !UUID_TEXT.test(id)) {
      setInvalid(true)
      return
    }
    setInvalid(false)
    onLookup(id.toLowerCase())
  }
  return (
    <form
      data-ui="platform.runtimes.lookup"
      className="mb-4 flex flex-wrap items-center gap-2"
      onSubmit={(event) => {
        event.preventDefault()
        submit()
      }}>
      <SearchInput
        size="sm"
        value={text}
        placeholder={t('platform.runtimes.placeholder')}
        aria-label={t('platform.runtimes.placeholder')}
        aria-invalid={invalid || undefined}
        containerClassName="w-[360px] max-w-full"
        onChange={(event) => {
          setText(event.target.value)
          setInvalid(false)
        }}
        onClear={() => {
          setText('')
          setInvalid(false)
          onLookup('')
        }}
        clearLabel={t('common.clear')}
      />
      <button type="submit" className={quietButtonClassName()}>
        {t('platform.runtimes.lookup')}
      </button>
      {invalid ? (
        <span role="alert" className="text-[12px] text-[#8a8a8a]">
          {t('platform.runtimes.invalid')}
        </span>
      ) : null}
    </form>
  )
}

function PoolCards({ active, onPick }: { active: string; onPick: (pool: string) => void }) {
  const { t } = useTranslation()
  const { data } = useSWR('studio/platform/fleet/topology', () => listPlatformItems('fleet', 'topology'))
  if (!data?.length) return null
  return (
    <section data-ui="platform.fleet.pools" aria-label={t('platform.fleet.pools')} className="mb-4">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {data.map((pool) => {
          const selected = active === pool.name
          const maintenance = fieldValue(pool, 'maintenance')
          return (
            <button
              key={pool.id}
              type="button"
              aria-pressed={selected}
              className={cn(
                'min-w-0 rounded-[12px] border bg-white px-3.5 py-3 text-left outline-none focus-visible:ring-2 focus-visible:ring-black/10',
                selected ? 'border-[#171717]' : 'border-[#efefef] hover:border-[#d9d9d9]'
              )}
              onClick={() => onPick(selected ? '' : pool.name)}>
              <span className="flex items-center justify-between gap-2">
                <span className="truncate text-[13px] font-medium text-[#1a1a1a]">{pool.name}</span>
                {maintenance ? <StatusChip value={maintenance} /> : null}
              </span>
              <span className="mt-1.5 block text-[18px] leading-6 font-semibold text-[#1a1a1a]">
                {fieldValue(pool, 'online') || '—'}
              </span>
              <span className="mt-1 block truncate text-[12px] leading-4 text-[#8a8a8a]">
                {t('platform.fleet.poolOnline')}
              </span>
            </button>
          )
        })}
      </div>
    </section>
  )
}

function nameMark(name: string) {
  return Array.from(name.trim())[0] || '—'
}

function NameMark({ name }: { name: string }) {
  return (
    <span className="flex size-8 shrink-0 items-center justify-center rounded-[8px] bg-[#f2f3f5] text-[13px] font-medium text-[#3f3f3f]">
      {nameMark(name)}
    </span>
  )
}

function fieldValue(item: PlatformItem, key: string) {
  return item.fields?.find((field) => field.key === key)?.value ?? ''
}

const CAPTCHA_GROUPS = ['align', 'attempts', 'lifetime'] as const

function captchaGroup(id: string) {
  if (id === 'tolerance_px' || id === 'min_duration_ms') return 'align'
  if (id === 'max_attempts') return 'attempts'
  return 'lifetime'
}

const PROVIDER_SECTIONS = ['credentials', 'callback', 'experience', 'accounts'] as const

function providerFieldRows(item: PlatformItem) {
  const indexes = new Set<string>()
  for (const field of item.fields) {
    const match = /^row\.(\d+)\./.exec(field.key)
    if (match?.[1]) indexes.add(match[1])
  }
  return [...indexes]
    .sort((left, right) => Number(left) - Number(right))
    .map((index) => ({
      section: fieldValue(item, `row.${index}.section`),
      label: fieldValue(item, `row.${index}.label`),
      description: fieldValue(item, `row.${index}.description`),
      value: fieldValue(item, `row.${index}.value`)
    }))
}

function displayProviderValue(value: string, t: (key: string) => string) {
  if (!value) return '—'
  const key = `platform.signIn.value.${value}`
  const label = t(key)
  return label === key ? value : label
}

function providerSteps(item: PlatformItem) {
  return [
    { key: 'app', done: fieldValue(item, 'ready.credentials') === 'yes' },
    { key: 'domain', done: fieldValue(item, 'ready.domain') === 'yes' },
    { key: 'enable', done: fieldValue(item, 'status') === 'live' }
  ]
}

function ProviderList({ rows, marker }: { rows: PlatformItem[]; marker: string }) {
  const { t } = useTranslation()
  const [openId, setOpenId] = useState<string | null>(null)
  const open = rows.find((item) => item.id === openId)
  if (open) {
    const steps = providerSteps(open)
    const done = steps.filter((step) => step.done).length
    const grouped = providerFieldRows(open)
    return (
      <div data-ui={marker} className="grid gap-4">
        <button
          type="button"
          className="h-9 self-start rounded-[8px] px-2.5 text-[13px] text-[#3f3f3f] outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10"
          onClick={() => setOpenId(null)}>
          {t('platform.signIn.back')}
        </button>
        <section className={featureCardClassName} aria-label={open.name}>
          <div className="flex items-center gap-3">
            <NameMark name={open.name} />
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <h3 className="m-0 truncate text-[18px] leading-6 font-semibold text-[#1a1a1a]">{open.name}</h3>
                <StatusChip value={fieldValue(open, 'status')} />
              </div>
              <p className="m-0 mt-0.5 text-[12px] text-[#8a8a8a]">{fieldValue(open, 'detail') || open.meta}</p>
            </div>
          </div>
        </section>
        <section className={featureCardClassName} aria-labelledby="sign-in-steps">
          <header className="flex items-center justify-between gap-3">
            <h3 id="sign-in-steps" className="m-0 text-[18px] leading-6 font-semibold text-[#1a1a1a]">
              {t('platform.signIn.steps')}
            </h3>
            <span className="text-[12px] text-[#8a8a8a]">
              {done}/{steps.length}
            </span>
          </header>
          <ol className="m-0 mt-3 grid gap-2 p-0 md:grid-cols-3">
            {steps.map((step, index) => (
              <li key={step.key} className="flex items-start gap-2 text-[13px] leading-5">
                <span
                  className={cn(
                    'mt-0.5 inline-flex size-4 shrink-0 items-center justify-center rounded-[4px] text-[11px]',
                    step.done ? 'bg-[#171717] text-white' : 'bg-[#f2f3f5] text-[#8a8a8a]'
                  )}>
                  {step.done ? '✓' : index + 1}
                </span>
                <span className={step.done ? 'text-[#8a8a8a] line-through' : 'text-[#1a1a1a]'}>
                  {t(`platform.signIn.step.${step.key}`)}
                </span>
              </li>
            ))}
          </ol>
        </section>
        {PROVIDER_SECTIONS.map((section) => {
          const entries = grouped.filter((row) => row.section === section)
          if (entries.length === 0) return null
          return (
            <section key={section} className={featureCardClassName}>
              <h3 className="m-0 text-[18px] leading-6 font-semibold text-[#1a1a1a]">
                {t(`platform.signIn.section.${section}`)}
              </h3>
              <dl className="m-0 mt-3 divide-y divide-[#efefef]">
                {entries.map((row) => (
                  <div key={row.label} className="flex items-start justify-between gap-3 py-3 first:pt-0 last:pb-0">
                    <dt className="m-0 min-w-0">
                      <span className="block text-[13px] text-[#3f3f3f]">{row.label}</span>
                      {row.description ? (
                        <span className="mt-0.5 block text-[12px] text-[#8a8a8a]">{row.description}</span>
                      ) : null}
                    </dt>
                    <dd className="m-0 max-w-[240px] truncate text-[13px] font-medium text-[#1a1a1a]">
                      {displayProviderValue(row.value, t)}
                    </dd>
                  </div>
                ))}
              </dl>
            </section>
          )
        })}
      </div>
    )
  }
  const live = rows.filter((item) => fieldValue(item, 'status') === 'live').length
  return (
    <div data-ui={marker} className="grid gap-3">
      <p className="m-0 text-[12px] text-[#8a8a8a]">
        {t('platform.signIn.liveCount', { value: `${live}/${rows.length}` })}
      </p>
      <ul className="m-0 divide-y divide-[#efefef] overflow-hidden rounded-[16px] border border-[#efefef] p-0">
        {rows.map((item) => {
          const status = fieldValue(item, 'status')
          const detail = fieldValue(item, 'detail') || item.meta
          const steps = providerSteps(item)
          const done = steps.filter((step) => step.done).length
          return (
            <li key={item.id}>
              <button
                type="button"
                className="flex w-full items-center gap-3 px-4 py-3 text-left outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset"
                onClick={() => setOpenId(item.id)}>
                <NameMark name={item.name} />
                <span className="min-w-0 flex-1">
                  <span className="flex items-center gap-2">
                    <span className="truncate text-[14px] font-medium text-[#1a1a1a]">{item.name}</span>
                    {status ? <StatusChip value={status} /> : null}
                  </span>
                  {detail ? <span className="mt-0.5 block truncate text-[12px] text-[#8a8a8a]">{detail}</span> : null}
                </span>
                <span className="shrink-0 text-[12px] text-[#8a8a8a]" title={t('platform.signIn.steps')}>
                  {done}/{steps.length}
                </span>
                <span className="shrink-0 text-[13px] text-[#8a8a8a]">{t('platform.signIn.configure')}</span>
              </button>
            </li>
          )
        })}
      </ul>
    </div>
  )
}

const SETTING_SOURCES = ['env', 'default', 'attention'] as const

function needsAttention(item: PlatformItem) {
  return fieldValue(item, 'required') === 'yes' && fieldValue(item, 'source') === 'default'
}

function matchesSource(item: PlatformItem, source: string) {
  if (!source) return true
  if (source === 'attention') return needsAttention(item)
  return fieldValue(item, 'source') === source
}

function SettingTag({ text, strong }: { text: string; strong?: boolean }) {
  return (
    <span
      className={cn(
        'inline-flex h-5 shrink-0 items-center rounded-[4px] px-1.5 text-[12px]',
        strong ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'text-[#8a8a8a]'
      )}>
      {text}
    </span>
  )
}

function GroupedSettings({ rows, marker }: { rows: PlatformItem[]; marker: string }) {
  const { t } = useTranslation()
  const [source, setSource] = useState('')
  const sourced = rows.some((item) => fieldValue(item, 'source'))
  const visible = rows.filter((item) => matchesSource(item, source))
  const groups: { name: string; items: PlatformItem[] }[] = []
  for (const item of visible) {
    const name = fieldValue(item, 'group')
    const current = groups.find((group) => group.name === name)
    if (current) current.items.push(item)
    else groups.push({ name, items: [item] })
  }
  return (
    <div data-ui={marker} className="grid gap-4">
      {sourced ? (
        <SegmentedControl
          size="sm"
          className="h-[30px] justify-self-start"
          aria-label={t('platform.col.source')}
          value={source}
          onValueChange={setSource}
          options={[
            { value: '', label: <FilterLabel text={t('common.all')} count={rows.length} /> },
            ...SETTING_SOURCES.map((value) => ({
              value,
              label: (
                <FilterLabel
                  text={t(`platform.settings.source.${value}`)}
                  count={rows.filter((item) => matchesSource(item, value)).length}
                />
              )
            }))
          ]}
        />
      ) : null}
      {groups.length === 0 ? <NoticeLine marker={`${marker}.none`} text={t('platform.nomatch')} /> : null}
      {groups.map((group) => (
        <section key={group.name || 'settings'} className={featureCardClassName}>
          {group.name ? <h3 className="m-0 text-[18px] leading-6 font-semibold text-[#1a1a1a]">{group.name}</h3> : null}
          <dl className="m-0 mt-3 divide-y divide-[#efefef]">
            {group.items.map((item) => {
              const itemSource = fieldValue(item, 'source')
              const secret = fieldValue(item, 'secret') === 'yes'
              const value = fieldValue(item, 'value')
              return (
                <div key={item.id} className="grid gap-1 py-3 first:pt-0 last:pb-0">
                  <dt className="m-0 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
                    <span className="text-[13px] font-medium text-[#1a1a1a]">{item.name}</span>
                    {sourced ? (
                      <span className="truncate font-[family-name:var(--code-font-family)] text-[12px] text-[#8a8a8a]">
                        {item.id}
                      </span>
                    ) : null}
                    {itemSource ? (
                      <SettingTag
                        text={t(`platform.settings.source.${itemSource === 'env' ? 'env' : 'default'}`)}
                        strong={itemSource === 'env'}
                      />
                    ) : null}
                    {fieldValue(item, 'required') === 'yes' ? (
                      <SettingTag
                        text={t(needsAttention(item) ? 'platform.settings.missing' : 'platform.settings.required')}
                        strong={needsAttention(item)}
                      />
                    ) : null}
                    {secret ? <SettingTag text={t('platform.settings.secret')} /> : null}
                  </dt>
                  {sourced ? (
                    <dd className="m-0 truncate font-[family-name:var(--code-font-family)] text-[12px] text-[#3f3f3f]">
                      {secret ? t('platform.settings.hidden') : value || '—'}
                    </dd>
                  ) : null}
                  <dd className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{fieldValue(item, 'detail') || '—'}</dd>
                </div>
              )
            })}
          </dl>
        </section>
      ))}
    </div>
  )
}

function CaptchaRules({ rows, marker }: { rows: PlatformItem[]; marker: string }) {
  const { t } = useTranslation()
  return (
    <div data-ui={marker} className="grid gap-4">
      {CAPTCHA_GROUPS.map((group) => {
        const items = rows.filter((item) => (fieldValue(item, 'group') || captchaGroup(item.id)) === group)
        if (items.length === 0) return null
        return (
          <section key={group} className={featureCardClassName}>
            <h3 className="m-0 text-[18px] leading-6 font-semibold text-[#1a1a1a]">
              {t(`platform.captcha.group.${group}`)}
            </h3>
            <dl className="m-0 mt-3 divide-y divide-[#efefef]">
              {items.map((item) => {
                const labelKey = `platform.captcha.${item.id}`
                const label = t(labelKey)
                const unit = fieldValue(item, 'unit')
                return (
                  <div key={item.id} className="flex items-center justify-between gap-3 py-3 first:pt-0 last:pb-0">
                    <dt className="m-0 text-[13px] text-[#3f3f3f]">{label === labelKey ? item.name : label}</dt>
                    <dd className="m-0 text-[13px] font-medium text-[#1a1a1a]">
                      {fieldValue(item, 'value') || item.meta || '—'}
                      {unit ? <span className="ml-1 font-normal text-[#8a8a8a]">{unit}</span> : null}
                    </dd>
                  </div>
                )
              })}
            </dl>
          </section>
        )
      })}
    </div>
  )
}

const FILLED_STATUSES = new Set([
  'enabled',
  'active',
  'live',
  'published',
  'healthy',
  'ready',
  'online',
  'running',
  'mounted'
])

function statusText(value: string, t: (key: string) => string) {
  if (!value) return '—'
  const key = `platform.status.${value}`
  const label = t(key)
  return label === key ? value : label
}

/** States an operator has to act on. They lead in ink with a dot; the palette keeps red for delete. */
const ATTENTION_STATUSES = new Set([
  'degraded',
  'unhealthy',
  'unreachable',
  'unavailable',
  'offline',
  'failed',
  'error',
  'warn',
  'frozen',
  'missing',
  'past_due',
  'suspended',
  'expired',
  'revoked',
  'rejected'
])

function StatusChip({ value }: { value: string }) {
  const { t } = useTranslation()
  const attention = ATTENTION_STATUSES.has(value)
  return (
    <span
      data-attention={attention || undefined}
      className={cn(
        'inline-flex h-5 items-center gap-1 rounded-[4px] px-1.5 text-[11px]',
        FILLED_STATUSES.has(value) && 'bg-[#f2f3f5] font-medium text-[#171717]',
        attention && 'font-medium text-[#171717]',
        !attention && !FILLED_STATUSES.has(value) && 'text-[#8a8a8a]'
      )}>
      {attention ? <span aria-hidden className="size-1.5 shrink-0 rounded-full bg-[#171717]" /> : null}
      {statusText(value, t)}
    </span>
  )
}

const STATUS_KEYS = new Set([
  'state',
  'status',
  'subscription_status',
  'health',
  'availability',
  'level',
  'release_state'
])
const KEY_FORMAT: Record<string, Column['format']> = {
  context: 'tokens',
  output: 'tokens',
  latency: 'duration',
  time: 'instant',
  created: 'time',
  published: 'time',
  yanked: 'time',
  updated: 'time',
  observed: 'time',
  size: 'bytes',
  memory: 'bytes',
  timeout: 'duration'
}

function pad(value: number) {
  return String(value).padStart(2, '0')
}

const DATE_ONLY = /^\d{4}-\d{2}-\d{2}$/

function formatTime(value: string) {
  if (DATE_ONLY.test(value)) return value
  const at = new Date(value)
  if (Number.isNaN(at.getTime())) return value
  return `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())} ${pad(at.getHours())}:${pad(at.getMinutes())}`
}

function formatInstant(value: string) {
  if (!value.includes('T')) return value
  const at = new Date(value)
  if (Number.isNaN(at.getTime())) return value
  return `${formatTime(value)}:${pad(at.getSeconds())}`
}

const MINUTE_MS = 60_000

function formatAgo(value: string, t: (key: string, options?: Record<string, unknown>) => string) {
  const at = new Date(value).getTime()
  if (Number.isNaN(at)) return value
  const minutes = Math.floor((Date.now() - at) / MINUTE_MS)
  if (minutes < 1) return t('platform.ago.now')
  if (minutes < 60) return t('platform.ago.minutes', { count: minutes })
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return t('platform.ago.hours', { count: hours })
  return t('platform.ago.days', { count: Math.floor(hours / 24) })
}

function formatBytes(value: string) {
  const bytes = Number(value)
  if (!Number.isFinite(bytes) || bytes < 0) return value
  const units = ['B', 'KB', 'MB', 'GB']
  let size = bytes
  let unit = 0
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024
    unit += 1
  }
  return `${unit === 0 ? size : size.toFixed(1)} ${units[unit]}`
}

function formatDuration(value: string) {
  const millis = Number(value)
  if (!Number.isFinite(millis) || millis < 0) return value
  return millis < 1000 ? `${millis} ms` : `${Number((millis / 1000).toFixed(1))} s`
}

const UNSERVED = new Set(['not_published', 'no_routable_supplier'])

function formatTokens(value: string) {
  const tokens = Number(value)
  if (!Number.isFinite(tokens) || tokens < 0) return value
  if (tokens >= 1_000_000) return `${Number((tokens / 1_000_000).toFixed(1))}M`
  if (tokens >= 1_000) return `${Number((tokens / 1_000).toFixed(1))}K`
  return String(tokens)
}

function cellText(column: Column | undefined, key: string, value: string, t: (key: string) => string) {
  if (!value) return '—'
  if (column?.chip || (!column && STATUS_KEYS.has(key))) return statusText(value, t)
  const format = column?.format ?? KEY_FORMAT[key]
  if (format === 'time') return formatTime(value)
  if (format === 'instant') return formatInstant(value)
  if (format === 'ago') return formatAgo(value, t)
  if (format === 'bytes') return formatBytes(value)
  if (format === 'duration') return formatDuration(value)
  if (format === 'tokens') return formatTokens(value)
  if (key === 'platform') return labeledValue('platform.apps.platform', value, t)
  if (key === 'kind') return labeledValue('platform.kind', value, t)
  if (key === 'grade') return labeledValue('platform.grade', value, t)
  if (key === 'mode') return labeledValue('platform.mode', value, t)
  if (key === 'plan') return labeledValue('platform.plan', value, t)
  if (key === 'plane') return labeledValue('platform.gateway.plane', value, t)
  if (key === 'credential') return labeledValue('platform.credential', value, t)
  if (key === 'served') return labeledValue(UNSERVED.has(value) ? 'platform.served' : 'platform.kind', value, t)
  if (key === 'capabilities') {
    return value
      .split(', ')
      .map((capability) => labeledValue('platform.capability', capability, t))
      .join(', ')
  }
  if (key === 'retains') return labeledValue('platform.signIn.value', value, t)
  if (key === 'category') return labeledValue('platform.category', value, t)
  if (key === 'deployment') return labeledValue('platform.deployment', value, t)
  if (key === 'transport') return labeledValue('platform.transport', value, t)
  if (key === 'featured') return labeledValue('platform.flag', value, t)
  if (key === 'desired') return statusText(value, t)
  if (key === 'bound') return labeledValue('platform.bound', value, t)
  if (key === 'channel') return labeledValue('platform.apps', value, t)
  if (key === 'failover') return labeledValue('platform.failover', value, t)
  if (key === 'routing') return labeledValue('platform.routing', value, t)
  if (key === 'multiplier') return `×${value}`
  if (key === 'share' || key === 'success') return `${value}%`
  if (key === 'cloud') return labeledValue('platform.usage.cloud', value, t)
  if (key === 'efforts') {
    return value
      .split(', ')
      .map((effort) => labeledValue('platform.effort', effort, t))
      .join(', ')
  }
  if (NUMERIC_KEYS.has(key)) return formatCount(value)
  return value
}

/** Whole counts get thousands separators; anything else (`1/2`, `0.5`) reads as sent. */
function formatCount(value: string) {
  return /^\d+$/.test(value) ? Number(value).toLocaleString() : value
}

/** Values in a plain column that block the row from working, flagged like an attention status. */
const ATTENTION_VALUES: Record<string, ReadonlySet<string>> = {
  credential: new Set(['missing']),
  served: new Set(['no_routable_supplier'])
}

function needsOperator(item: PlatformItem, columns: Column[]) {
  return columns.some((column) => {
    const value = fieldValue(item, column.key)
    return column.chip ? ATTENTION_STATUSES.has(value) : (ATTENTION_VALUES[column.key]?.has(value) ?? false)
  })
}

const PAGE_SIZE = 20
const NO_ROWS: readonly PlatformItem[] = []
const itemId = (item: PlatformItem) => item.id
const ALL_CHOICE = '__all__'
const choiceFieldClassName =
  'h-[30px] w-auto min-w-[168px] rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 text-[13px] text-[#3f3f3f] shadow-none'

type SortState = { key: string; direction: 'ascending' | 'descending' } | null

type ListState = {
  scope: string
  query: string
  choice: string
  filter: string
  page: number
  pageSize: number
  openId: string | null
  sort: SortState
}

function freshList(scope: string): ListState {
  return { scope, query: '', choice: '', filter: '', page: 1, pageSize: PAGE_SIZE, openId: null, sort: null }
}

/** Each list's search, filters, sort and page live as long as its data cache, so a menu resumes where it was left. */
const listMemories = new WeakMap<object, Map<string, ListState>>()

function useListMemory() {
  const { cache } = useSWRConfig()
  let memory = listMemories.get(cache)
  if (!memory) {
    memory = new Map()
    listMemories.set(cache, memory)
  }
  return memory
}

function sortValue(item: PlatformItem, key: string) {
  return key === 'name' ? item.name : fieldValue(item, key)
}

function countValues(rows: PlatformItem[], key: string) {
  const counts = new Map<string, number>()
  for (const item of rows) {
    const value = fieldValue(item, key)
    if (value) counts.set(value, (counts.get(value) ?? 0) + 1)
  }
  return [...counts].sort((left, right) => right[1] - left[1]).map(([value, count]) => ({ value, count }))
}

function pickFilter(columns: Column[], rows: PlatformItem[]) {
  const candidates = [
    ...columns.filter((column) => column.filter),
    ...columns.filter((column) => column.chip && !column.filter && !column.select && !column.noFilter)
  ]
  for (const column of candidates) {
    const options = countValues(rows, column.key)
    if (options.length > 1) return { filterColumn: column, filterOptions: options }
  }
  const filterColumn = candidates[0]
  return { filterColumn, filterOptions: filterColumn ? countValues(rows, filterColumn.key) : [] }
}

function NoticeLine({
  marker,
  icon: Icon,
  title,
  text,
  detail,
  action
}: {
  marker: string
  icon?: typeof Inbox
  title?: string
  text: string
  detail?: string
  action?: ReactNode
}) {
  const { t } = useTranslation()
  return (
    <div className="flex flex-col items-center gap-3 py-16" data-ui={marker}>
      {Icon ? (
        <span className="flex size-10 items-center justify-center rounded-full bg-[#f5f5f5] text-[#8a8a8a]">
          <Icon aria-hidden className="size-[18px]" strokeWidth={1.75} />
        </span>
      ) : null}
      <div className="flex max-w-md flex-col items-center gap-1 text-center">
        {title ? <p className="m-0 text-[14px] font-medium text-[#1a1a1a]">{title}</p> : null}
        <p className="m-0 text-[13px] leading-5 text-[#8a8a8a]">{text}</p>
        {detail ? (
          <details className="mt-1 text-[12px] text-[#8a8a8a]">
            <summary className="cursor-pointer rounded-[4px] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10">
              {t('platform.error.detail')}
            </summary>
            <p className="m-0 mt-1 font-[family-name:var(--code-font-family)] text-[11px] break-all select-text">
              {detail}
            </p>
          </details>
        ) : null}
      </div>
      {action}
    </div>
  )
}

const SKELETON_ROWS = 6

/** Row-shaped placeholders, so a revisit that skips the entry mark still reads as loading. */
function ListSkeleton() {
  return (
    <div data-ui="platform.skeleton" aria-hidden className="grid">
      <div className="h-9 border-b border-[#efefef]" />
      {Array.from({ length: SKELETON_ROWS }, (_, index) => (
        <div key={index} className="flex h-12 items-center gap-2.5 px-2.5">
          <span className="size-7 shrink-0 animate-pulse rounded-[8px] bg-[#f2f3f5]" />
          <span className="h-3 w-[22%] animate-pulse rounded-full bg-[#f2f3f5]" />
          <span className="ml-auto h-3 w-[12%] animate-pulse rounded-full bg-[#f2f3f5]" />
          <span className="h-3 w-[16%] animate-pulse rounded-full bg-[#f2f3f5]" />
        </div>
      ))}
    </div>
  )
}

/** A ledger this installation does not keep explains itself; anything else offers a retry. */
function FailureNotice({ marker, error, onRetry }: { marker: string; error: unknown; onRetry: () => void }) {
  const { t } = useTranslation()
  const absent = isPlatformAbsent(error)
  return (
    <NoticeLine
      marker={marker}
      icon={absent ? Archive : CloudOff}
      title={t(absent ? 'platform.absent.title' : 'platform.error.title')}
      text={platformErrorText(error, t)}
      detail={platformErrorDetail(error)}
      action={
        absent ? undefined : (
          <button type="button" className={quietButtonClassName()} onClick={onRetry}>
            {t('common.retry')}
          </button>
        )
      }
    />
  )
}

const textButtonClassName =
  'rounded-[6px] px-1 text-[13px] text-[#8a8a8a] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10'

function RecordPage({
  titleKey,
  descriptionKey,
  marker,
  swrKey,
  load,
  segments,
  below,
  body = 'table',
  nameLabelKey = 'platform.col.name',
  columns = [],
  cardTitleKey,
  onOpen,
  rowFilter,
  listScope,
  refreshInterval,
  choice,
  onChoiceChange,
  detailExtra,
  searchable = true,
  problemsFirst = true,
  emptyKey = 'platform.empty',
  emptyAction
}: {
  titleKey: string
  descriptionKey: string
  marker: string
  swrKey: string
  searchable?: boolean
  /** Unsorted tables lead with rows an operator has to act on; off where order is the point (time, ranking). */
  problemsFirst?: boolean
  emptyKey?: string
  emptyAction?: ReactNode
  listScope?: string
  refreshInterval?: number
  choice?: string
  onChoiceChange?: (value: string) => void
  detailExtra?: (item: PlatformItem) => ReactNode
  load: () => Promise<PlatformItem[]>
  segments?: ReactNode
  below?: ReactNode
  body?: BodyKind
  nameLabelKey?: string
  columns?: Column[]
  cardTitleKey?: string
  onOpen?: (item: PlatformItem) => void
  rowFilter?: (item: PlatformItem) => boolean
}) {
  const { t } = useTranslation()
  const scope = listScope ?? swrKey
  const memory = useListMemory()
  const [state, setState] = useState(() => memory.get(scope) ?? freshList(scope))
  const list = state.scope === scope ? state : (memory.get(scope) ?? freshList(scope))
  const update = (patch: Partial<ListState>) => {
    const next = { ...list, ...patch }
    memory.set(scope, next)
    setState(next)
  }
  const pickChoice = (value: string, patch: Partial<ListState> = {}) => {
    onChoiceChange?.(value)
    update({ ...patch, choice: value, filter: '', page: 1, openId: null })
  }
  const { data, error, isLoading, isValidating, mutate } = useSWR(swrKey, load, {
    keepPreviousData: listScope !== undefined,
    refreshInterval,
    onSuccess: () => markLoaded(swrKey)
  })
  const stale = isLoading && Boolean(data)
  const searchRef = useRef<HTMLInputElement>(null)
  useSlashToSearch(searchRef)

  const needle = list.query.trim().toLowerCase()
  const baseRows = rowFilter ? (data ?? []).filter(rowFilter) : (data ?? [])
  const searched = baseRows.filter((item) => {
    const haystack = [item.name, item.meta, item.id, ...item.fields.map((field) => field.value)].join(' ').toLowerCase()
    return haystack.includes(needle)
  })
  const choiceColumn = body === 'table' ? columns.find((column) => column.select) : undefined
  const choiceCounts = choiceColumn
    ? new Map(countValues(searched, choiceColumn.key).map((option) => [option.value, option.count]))
    : new Map<string, number>()
  const choiceOptions = choiceColumn
    ? countValues(baseRows, choiceColumn.key).map((option) => ({
        ...option,
        count: choiceCounts.get(option.value) ?? 0
      }))
    : []
  const wantedChoice = choice ?? list.choice
  const activeChoice = choiceColumn ? wantedChoice : ''
  if (activeChoice && !choiceOptions.some((option) => option.value === activeChoice))
    choiceOptions.push({ value: activeChoice, count: 0 })
  const chosen =
    choiceColumn && activeChoice
      ? searched.filter((item) => fieldValue(item, choiceColumn.key) === activeChoice)
      : searched
  const { filterColumn, filterOptions } = pickFilter(body === 'table' ? columns : [], baseRows)
  if (filterColumn) {
    const counts = new Map(countValues(chosen, filterColumn.key).map((option) => [option.value, option.count]))
    for (const option of filterOptions) option.count = counts.get(option.value) ?? 0
    if (list.filter && !filterOptions.some((option) => option.value === list.filter))
      filterOptions.push({ value: list.filter, count: 0 })
  }
  const activeFilter = filterColumn ? list.filter : ''
  const rows =
    filterColumn && activeFilter ? chosen.filter((item) => fieldValue(item, filterColumn.key) === activeFilter) : chosen
  const narrowed = needle !== '' || activeFilter !== '' || activeChoice !== ''
  const raised =
    body === 'table' && problemsFirst && !list.sort ? rows.filter((item) => needsOperator(item, columns)).length : 0
  const pageCount = Math.max(1, Math.ceil(rows.length / list.pageSize))
  const page = Math.min(list.page, pageCount)
  // Persist the clamp: otherwise removed rows can silently return the operator to an old page on refresh.
  useEffect(() => {
    if (error || !data || stale || list.page <= pageCount) return
    const next = { ...list, page: pageCount, openId: null }
    memory.set(scope, next)
    setState(next)
  }, [data, error, stale, list, pageCount, memory, scope])

  const sort = list.sort
  const sorted = sort
    ? rows.toSorted((left, right) => {
        const order = sortValue(left, sort.key).localeCompare(sortValue(right, sort.key), undefined, {
          numeric: true,
          sensitivity: 'base'
        })
        return sort.direction === 'ascending' ? order : -order
      })
    : raised > 0
      ? rows.toSorted((left, right) => Number(needsOperator(right, columns)) - Number(needsOperator(left, columns)))
      : rows
  const visibleRows = sorted.slice((page - 1) * list.pageSize, page * list.pageSize)
  const selection = useCollectionSelection(
    body === 'table' && !error && !stale ? visibleRows : [],
    JSON.stringify([
      readCenterMemberId(),
      readCenterId(),
      scope,
      list.query,
      activeChoice,
      activeFilter,
      list.sort,
      page,
      list.pageSize
    ])
  )
  const selectionOpen = selection.ids.length > 0
  const selectionHold = useRef<readonly string[]>([])
  const selectionPresence = useRestingDialog(selectionOpen)
  if (selectionOpen) selectionHold.current = selection.ids
  const shownSelectionIds = selectionOpen ? selection.ids : selectionHold.current

  let content: ReactNode
  if ((!data && !error) || (stale && rows.length === 0)) {
    content = (
      <div className="relative min-h-40" aria-busy="true">
        <PageEntry id={marker} ready={false} />
        <ListSkeleton />
      </div>
    )
  } else if (error) {
    content = <FailureNotice marker={marker} error={error} onRetry={() => void mutate()} />
  } else if (rows.length === 0) {
    content = (
      <NoticeLine
        marker={marker}
        icon={narrowed ? SearchX : Inbox}
        text={
          needle
            ? t('platform.nomatchQuery', { query: list.query.trim() })
            : t(narrowed ? 'platform.nomatch' : emptyKey)
        }
        action={
          narrowed ? (
            <button type="button" className={textButtonClassName} onClick={() => pickChoice('', { query: '' })}>
              {t('platform.clearFilters')}
            </button>
          ) : (
            emptyAction
          )
        }
      />
    )
  } else if (body === 'providers') {
    content = <ProviderList rows={rows} marker={marker} />
  } else if (body === 'rules') {
    content = <CaptchaRules rows={rows} marker={marker} />
  } else if (body === 'card') {
    content = (
      <article className={featureCardClassName} data-ui={marker}>
        <h3 className="m-0 text-[18px] leading-6 font-semibold text-[#1a1a1a]">{t(cardTitleKey ?? titleKey)}</h3>
        <dl className="m-0 mt-3 grid gap-2">
          {rows.map((item) => {
            const labelKey = item.id === 'status' ? 'platform.debug.revision' : `platform.captcha.${item.id}`
            const label = t(labelKey)
            return (
              <div key={item.id} className="flex items-center justify-between gap-3">
                <dt className="m-0 text-[13px] text-[#3f3f3f]">{label === labelKey ? item.name : label}</dt>
                <dd className="m-0 text-[13px] font-medium text-[#1a1a1a]">
                  {fieldValue(item, 'revision') || item.meta || '—'}
                </dd>
              </div>
            )
          })}
        </dl>
      </article>
    )
  } else if (body === 'settings' && rows.some((item) => fieldValue(item, 'group'))) {
    content = <GroupedSettings rows={rows} marker={marker} />
  } else if (body === 'settings') {
    content = (
      <table data-ui={marker} className="w-full table-fixed border-collapse text-left">
        <thead className="sticky top-0 z-[1] bg-white">
          <tr className="border-b border-[#efefef] text-[12px] text-[#8a8a8a]">
            <th scope="col" className="h-9 pl-2.5 font-normal">
              {t('platform.col.key')}
            </th>
            <th scope="col" className="h-9 font-normal">
              {t('platform.col.name')}
            </th>
            <th scope="col" className="h-9 pr-2.5 font-normal">
              {t('platform.col.detail')}
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((item) => (
            <tr key={item.id} className="hover:bg-[#f7f7f7]">
              <td className="h-12 overflow-hidden pl-2.5 text-[13px] text-[#8a8a8a]">
                <span className="block truncate">{item.id}</span>
              </td>
              <td className="h-12 overflow-hidden text-[13px] font-medium text-[#1a1a1a]">
                <span className="block truncate">{item.name}</span>
              </td>
              <td className="h-12 overflow-hidden pr-2.5 text-[13px] text-[#3f3f3f]">
                <span className="block truncate">{item.meta || '—'}</span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    )
  } else {
    content = (
      <>
        <div className="overflow-x-auto">
          <RecordTable
            marker={marker}
            label={t(titleKey)}
            rows={visibleRows}
            selection={selection}
            selectionDisabled={stale}
            nameLabelKey={nameLabelKey}
            columns={columns}
            sort={sort}
            onSort={(key) =>
              update({
                page: 1,
                sort:
                  sort?.key !== key
                    ? { key, direction: 'ascending' }
                    : sort.direction === 'ascending'
                      ? { key, direction: 'descending' }
                      : null
              })
            }
            openId={list.openId}
            onToggle={(id) => update({ openId: list.openId === id ? null : id })}
            onOpen={onOpen}
            detailExtra={detailExtra}
          />
        </div>
      </>
    )
  }

  const showSummary =
    body === 'table' &&
    Boolean(data) &&
    !error &&
    (rows.length > 0 || narrowed || filterOptions.length > 1 || choiceOptions.length > 1)

  return (
    <PlatformFrame
      title={t(titleKey)}
      description={t(descriptionKey)}
      overlay={
        selectionPresence.present ? (
          <CollectionSelectionBar
            ids={shownSelectionIds}
            state={selectionPresence.state}
            onClear={selection.clear}
            disabled={stale}
          />
        ) : undefined
      }>
      {below}
      <CollectionToolbar
        label={t('collection.controls', { name: t(titleKey) })}
        search={
          <>
            {isPlatformAbsent(error) || !searchable ? null : (
              <SearchInput
                ref={searchRef}
                size="sm"
                value={list.query}
                placeholder={t('common.search')}
                aria-label={t('common.search')}
                aria-keyshortcuts="/"
                title={t('platform.searchShortcut')}
                containerClassName={searchFieldClassName}
                onKeyDown={(event) => {
                  if (event.key === 'Escape' && list.query) update({ query: '', page: 1 })
                }}
                onChange={(event) => update({ query: event.target.value, page: 1, openId: null })}
                onClear={() => update({ query: '', page: 1 })}
                clearLabel={t('common.clear')}
              />
            )}
          </>
        }
        filters={
          showSummary ? (
            <>
              {choiceColumn && (choiceOptions.length > 1 || activeChoice) ? (
                <Combobox
                  size="sm"
                  className={choiceFieldClassName}
                  popoverAlign="start"
                  searchable={choiceOptions.length > 8}
                  searchPlaceholder={t('common.search')}
                  aria-label={t(choiceColumn.labelKey)}
                  value={activeChoice || ALL_CHOICE}
                  options={[
                    { value: ALL_CHOICE, label: t('common.all'), count: searched.length },
                    ...choiceOptions.map((option) => ({
                      value: option.value,
                      label: cellText(choiceColumn, choiceColumn.key, option.value, t),
                      count: option.count
                    }))
                  ]}
                  renderOption={(option) => (
                    <span className="flex min-w-0 flex-1 items-center justify-between gap-4">
                      <span className="truncate">{option.label}</span>
                      <span className="shrink-0 text-[12px] text-[#8a8a8a]">{option.count}</span>
                    </span>
                  )}
                  renderValue={(value) => {
                    const picked = Array.isArray(value) ? value[0] : value
                    const text =
                      !picked || picked === ALL_CHOICE
                        ? t('common.all')
                        : cellText(choiceColumn, choiceColumn.key, picked, t)
                    return t('platform.filter.value', { label: t(choiceColumn.labelKey), value: text })
                  }}
                  onChange={(next) => {
                    const picked = Array.isArray(next) ? next[0] : next
                    pickChoice(picked === ALL_CHOICE ? '' : (picked ?? ''))
                  }}
                />
              ) : null}
              {filterColumn && (filterOptions.length > 1 || activeFilter) ? (
                <SegmentedControl
                  size="sm"
                  className="h-[30px]"
                  aria-label={t(filterColumn.labelKey)}
                  value={activeFilter}
                  onValueChange={(value) => update({ filter: value, page: 1, openId: null })}
                  options={[
                    { value: '', label: <FilterLabel text={t('common.all')} count={chosen.length} /> },
                    ...filterOptions.map((option) => ({
                      value: option.value,
                      label: (
                        <FilterLabel
                          text={cellText(filterColumn, filterColumn.key, option.value, t)}
                          count={option.count}
                        />
                      )
                    }))
                  ]}
                />
              ) : null}
            </>
          ) : null
        }
        views={segments}
        actions={
          <RefreshButton
            busy={isValidating}
            updatedAt={data ? loadedAt.get(swrKey) : undefined}
            onClick={() => void mutate()}
          />
        }
        summary={
          showSummary ? (
            <span className="flex items-center gap-3 text-[12px] text-[#8a8a8a]">
              {raised > 0 ? (
                <span data-ui="platform.problems-first" className="flex items-center gap-1.5 text-[#171717]">
                  <span aria-hidden className="size-1.5 rounded-full bg-[#171717]" />
                  {t('platform.problemsFirst', { count: raised })}
                </span>
              ) : null}
              <span>{t('platform.total', { total: rows.length })}</span>
            </span>
          ) : null
        }
      />
      <div
        data-ui="platform.list"
        aria-busy={stale || undefined}
        className={cn('transition-opacity', stale && 'pointer-events-none opacity-50')}>
        {content}
      </div>
      {body === 'table' && data && !error ? (
        <CollectionPagination
          page={page}
          pageSize={list.pageSize}
          total={rows.length}
          busy={isValidating}
          onPageChange={(next) => update({ page: next, openId: null })}
          onPageSizeChange={(pageSize) => update({ pageSize, page: 1, openId: null })}
        />
      ) : null}
    </PlatformFrame>
  )
}

/** `/` jumps to the list search unless the operator is already typing somewhere. */
function useSlashToSearch(input: { current: HTMLInputElement | null }) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== '/' || event.metaKey || event.ctrlKey || event.altKey) return
      const target = event.target instanceof HTMLElement ? event.target : null
      if (target?.closest('input, textarea, select, [contenteditable="true"], [role="textbox"]')) return
      if (!input.current) return
      event.preventDefault()
      input.current.focus()
      input.current.select()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [input])
}

function FilterLabel({ text, count }: { text: string; count: number }) {
  return (
    <>
      <span>{text}</span>
      <span className="text-[12px] font-normal text-[#8a8a8a]">{count}</span>
    </>
  )
}

function SortHeader({
  label,
  sortKey,
  sort,
  onSort,
  className,
  enabled = true
}: {
  label: string
  sortKey: string
  sort: SortState
  onSort: (key: string) => void
  className?: string
  enabled?: boolean
}) {
  if (!enabled)
    return (
      <th scope="col" className={cn('h-9 text-[12px] font-normal text-[#8a8a8a]', className)}>
        {label}
      </th>
    )
  const direction = sort?.key === sortKey ? sort.direction : undefined
  const Icon = direction === 'descending' ? ChevronDown : ChevronUp
  return (
    <th scope="col" aria-sort={direction ?? 'none'} className={cn('h-9 font-normal', className)}>
      <button
        type="button"
        onClick={() => onSort(sortKey)}
        className="inline-flex items-center gap-1 rounded-[6px] text-[12px] text-[#8a8a8a] outline-none hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10">
        {label}
        <Icon className={cn('size-3 text-current', !direction && 'opacity-0')} strokeWidth={1.7} aria-hidden />
      </button>
    </th>
  )
}

// Gateway hops render in the call chain instead of the field grid.
const DETAIL_HIDDEN = new Set([
  'notes',
  'group',
  'fields',
  'gateway_node',
  'members',
  'workload',
  'port',
  'upstream',
  'section'
])

function RecordDetail({ item, columns, extra }: { item: PlatformItem; columns: Column[]; extra?: ReactNode }) {
  const { t } = useTranslation()
  const notes = fieldValue(item, 'notes')
  const structured = fieldValue(item, 'fields')
  const entries = item.fields.filter((field) => field.value && !DETAIL_HIDDEN.has(field.key))
  const labelFor = (key: string) => {
    const column = columns.find((entry) => entry.key === key)
    if (column) return t(column.labelKey)
    const labelKey = `platform.col.${key}`
    const label = t(labelKey)
    return label === labelKey ? key : label
  }
  return (
    <div className="grid gap-3 pt-1 pb-4 pl-[52px]">
      {notes ? <p className="m-0 text-[13px] leading-5 whitespace-pre-line text-[#3f3f3f]">{notes}</p> : null}
      <dl className="m-0 grid grid-cols-2 gap-x-6 gap-y-2 lg:grid-cols-3">
        {entries.map((field) => (
          <div key={field.key} className="min-w-0">
            <dt className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{labelFor(field.key)}</dt>
            <dd className="m-0 mt-0.5 truncate text-[13px] text-[#1a1a1a]" title={field.value}>
              {cellText(
                columns.find((entry) => entry.key === field.key),
                field.key,
                field.value,
                t
              )}
            </dd>
          </div>
        ))}
        <div className="min-w-0">
          <dt className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{t('platform.col.id')}</dt>
          <dd className="m-0 mt-0.5 flex min-w-0 items-center gap-1">
            <span
              className="truncate font-[family-name:var(--code-font-family)] text-[12px] text-[#3f3f3f]"
              title={item.id}>
              {item.id}
            </span>
            <CopyButton value={item.id} />
          </dd>
        </div>
      </dl>
      {structured ? (
        <div className="min-w-0">
          <p className="m-0 text-[12px] leading-4 text-[#8a8a8a]">{t('platform.col.fields')}</p>
          <pre className="m-0 mt-1 max-h-64 overflow-auto rounded-[8px] bg-[#f7f7f7] p-3 font-[family-name:var(--code-font-family)] text-[12px] leading-5 whitespace-pre-wrap text-[#3f3f3f]">
            {structured}
          </pre>
        </div>
      ) : null}
      {extra}
    </div>
  )
}

const COPIED_MS = 1_500

function CopyButton({ value }: { value: string }) {
  const { t } = useTranslation()
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle')
  useEffect(() => {
    if (state === 'idle') return
    const timer = window.setTimeout(() => setState('idle'), COPIED_MS)
    return () => window.clearTimeout(timer)
  }, [state])
  const label = t(state === 'copied' ? 'common.copied' : state === 'failed' ? 'common.copy_failed' : 'common.copy')
  const Icon = state === 'copied' ? Check : Copy
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className="flex size-5 shrink-0 items-center justify-center rounded-[4px] text-[#8a8a8a] outline-none hover:bg-[#efefef] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10"
      onClick={() => {
        void navigator.clipboard
          .writeText(value)
          .then(() => setState('copied'))
          .catch(() => setState('failed'))
      }}>
      <Icon className="size-3 text-current" strokeWidth={1.7} aria-hidden />
    </button>
  )
}

function CustomerPeople({ customerId }: { customerId: string }) {
  return (
    <DetailHistory
      marker="platform.customers.people"
      titleKey="platform.customers.people"
      loadingKey="platform.customers.peopleLoading"
      emptyKey="platform.customers.noPeople"
      swrKey={`studio/platform/customers/${customerId}/people`}
      load={() => listCustomerPeople(customerId)}
      render={(person) => (
        <>
          <NameMark name={person.name} />
          <span className="truncate text-[#1a1a1a]">{person.name}</span>
          <span className="truncate text-[#8a8a8a]">{fieldValue(person, 'contact')}</span>
          <span className="ml-auto shrink-0">
            <StatusChip value={fieldValue(person, 'status')} />
          </span>
        </>
      )}
    />
  )
}

const detailLabelClassName = 'm-0 text-[12px] leading-4 text-[#8a8a8a]'
const detailNoteClassName = 'm-0 mt-1 text-[13px] text-[#8a8a8a]'
const digestClassName = 'truncate font-[family-name:var(--code-font-family)] text-[12px] text-[#8a8a8a]'

function DetailHistory({
  marker,
  titleKey,
  loadingKey,
  emptyKey,
  swrKey,
  load,
  pick,
  render
}: {
  marker: string
  titleKey: string
  loadingKey: string
  emptyKey: string
  swrKey: string
  load: () => Promise<PlatformItem[]>
  pick?: (item: PlatformItem) => boolean
  render: (item: PlatformItem) => ReactNode
}) {
  const { t } = useTranslation()
  const { data: rows, error, mutate } = useSWR(swrKey, load)
  const data = rows && pick ? rows.filter(pick) : rows
  const pagination = useCollectionPage(data, swrKey)
  return (
    <section data-ui={marker} className="min-w-0">
      <p className={detailLabelClassName}>{t(titleKey)}</p>
      {error ? (
        <div role="alert" className="flex items-center gap-2">
          <p className={detailNoteClassName}>{platformErrorText(error, t)}</p>
          <button type="button" className={textButtonClassName} onClick={() => void mutate()}>
            {t('common.retry')}
          </button>
        </div>
      ) : !data ? (
        <p className={detailNoteClassName}>{t(loadingKey)}</p>
      ) : data.length === 0 ? (
        <p className={detailNoteClassName}>{t(emptyKey)}</p>
      ) : (
        <>
          <ul className="m-0 mt-1 grid list-none gap-1.5 p-0">
            {pagination.visible.map((item) => (
              <li key={item.id} className="flex min-w-0 items-center gap-3 text-[13px]">
                {render(item)}
              </li>
            ))}
          </ul>
          {data.length > 20 ? <CollectionPagination {...pagination} /> : null}
        </>
      )}
    </section>
  )
}

function SupplierModels({ supplierId }: { supplierId: string }) {
  const supplier = useSupplierActions()
  const scope = `${readCenterMemberId()}/${readCenterId()}`
  return (
    <SupplierRelations
      key={`${scope}/${supplierId}`}
      supplierId={supplierId}
      scope={scope}
      authorized={Boolean(supplier?.access?.read)}
    />
  )
}

function ModelRoutes({ modelId }: { modelId: string }) {
  const { t } = useTranslation()
  return (
    <DetailHistory
      marker="platform.models.routes"
      titleKey="platform.models.routes"
      loadingKey="platform.models.routesLoading"
      emptyKey="platform.models.noRoutes"
      swrKey="studio/platform/model-routes"
      load={() => listModelRoutes()}
      pick={(route) => fieldValue(route, 'offering') === modelId}
      render={(route) => (
        <>
          <span className="w-[36px] shrink-0 text-[12px] text-[#8a8a8a]">
            {t('platform.models.priority', { value: fieldValue(route, 'priority') })}
          </span>
          <span className="w-[160px] shrink-0 truncate font-medium text-[#1a1a1a]" title={route.name}>
            {route.name}
          </span>
          <StatusChip value={fieldValue(route, 'status')} />
          <span className={cn(digestClassName, 'min-w-0 flex-1')} title={fieldValue(route, 'upstream')}>
            {fieldValue(route, 'upstream')}
          </span>
          <span className="shrink-0 text-[12px] text-[#8a8a8a]">
            {t('platform.models.weight', { value: fieldValue(route, 'weight') })}
          </span>
        </>
      )}
    />
  )
}

function FunctionReleases({ assetId }: { assetId: string }) {
  const { t } = useTranslation()
  return (
    <DetailHistory
      marker="platform.functions.releases"
      titleKey="platform.functions.releases"
      loadingKey="platform.functions.releasesLoading"
      emptyKey="platform.functions.noReleases"
      swrKey={`studio/platform/functions/${assetId}/releases`}
      load={() => listFunctionReleases(assetId)}
      render={(release) => (
        <>
          <span className="w-[88px] shrink-0 truncate font-medium text-[#1a1a1a]">{release.name}</span>
          <StatusChip value={fieldValue(release, 'state')} />
          <span className="shrink-0 text-[#3f3f3f]">{cellText(undefined, 'mode', fieldValue(release, 'mode'), t)}</span>
          <span className={cn(digestClassName, 'min-w-0 flex-1')} title={fieldValue(release, 'artifact')}>
            {fieldValue(release, 'artifact') || '—'}
          </span>
          <span className="shrink-0 text-[12px] text-[#8a8a8a]">
            {cellText(undefined, 'published', fieldValue(release, 'published') || fieldValue(release, 'created'), t)}
          </span>
        </>
      )}
    />
  )
}

function CellRevisions({ cellId }: { cellId: string }) {
  const { t } = useTranslation()
  return (
    <DetailHistory
      marker="platform.cells.revisions"
      titleKey="platform.cells.revisions"
      loadingKey="platform.cells.revisionsLoading"
      emptyKey="platform.cells.noRevisions"
      swrKey={`studio/platform/cells/${cellId}/revisions`}
      load={() => listCellRevisions(cellId)}
      render={(revision) => (
        <>
          <span className="w-[56px] shrink-0 font-medium text-[#1a1a1a]">{revision.name}</span>
          <span className="shrink-0 text-[#3f3f3f]">{fieldValue(revision, 'schema') || '—'}</span>
          <span className={cn(digestClassName, 'min-w-0 flex-1')} title={fieldValue(revision, 'digest')}>
            {fieldValue(revision, 'digest') || '—'}
          </span>
          <span className="shrink-0 text-[12px] text-[#8a8a8a]">
            {cellText(undefined, 'created', fieldValue(revision, 'created'), t)}
          </span>
        </>
      )}
    />
  )
}

function RuntimeTrajectory({ executionId }: { executionId: string }) {
  const { t } = useTranslation()
  const { data, error } = useSWR(`studio/platform/agent-runtimes/${executionId}/logs`, () =>
    readAgentRuntimeLogs(executionId)
  )
  const [more, setMore] = useState<PlatformRowPage[]>([])
  const [loadingMore, setLoadingMore] = useState(false)
  const [moreError, setMoreError] = useState<unknown>(null)
  const pages = data ? [data, ...more] : []
  const seen = new Set<string>()
  const records = pages
    .flatMap((page) => page.rows)
    .filter((record) => {
      if (seen.has(record.id)) return false
      seen.add(record.id)
      return true
    })
  const cursor = pages.at(-1)?.nextCursor ?? null
  const loadMore = async () => {
    if (!cursor || loadingMore) return
    setLoadingMore(true)
    setMoreError(null)
    try {
      const page = await readAgentRuntimeLogs(executionId, cursor)
      setMore((current) => [...current, page])
    } catch (failure) {
      setMoreError(failure)
    } finally {
      setLoadingMore(false)
    }
  }
  const failure = error ?? moreError
  return (
    <section data-ui="platform.runtimes.trajectory" className="min-w-0">
      <p className={detailLabelClassName}>{t('platform.runtimes.trajectory')}</p>
      {!data && !error ? (
        <p className={detailNoteClassName}>{t('platform.runtimes.trajectoryLoading')}</p>
      ) : records.length === 0 && !failure ? (
        <p className={detailNoteClassName}>{t('platform.runtimes.noTrajectory')}</p>
      ) : (
        <ol className="m-0 mt-1 grid list-none gap-2 p-0">
          {records.map((record) => {
            const content = fieldValue(record, 'fields')
            return (
              <li key={record.id} className="min-w-0">
                <div className="flex min-w-0 items-center gap-3 text-[13px]">
                  <span className="w-[40px] shrink-0 text-[12px] text-[#8a8a8a]">#{record.id}</span>
                  <span className="truncate font-medium text-[#1a1a1a]">
                    {labeledValue('platform.logKind', record.name, t)}
                  </span>
                  <span className="ml-auto shrink-0 text-[12px] text-[#8a8a8a]">
                    {cellText(undefined, 'time', fieldValue(record, 'time'), t)}
                  </span>
                </div>
                {content ? (
                  <pre className="m-0 mt-1 ml-[52px] max-h-40 overflow-auto rounded-[8px] bg-[#f7f7f7] p-2.5 font-[family-name:var(--code-font-family)] text-[12px] leading-5 whitespace-pre-wrap text-[#3f3f3f]">
                    {content}
                  </pre>
                ) : null}
              </li>
            )
          })}
        </ol>
      )}
      {failure ? <p className={detailNoteClassName}>{platformErrorText(failure, t)}</p> : null}
      {cursor ? (
        <button
          type="button"
          className={cn(textButtonClassName, 'mt-2')}
          disabled={loadingMore}
          onClick={() => void loadMore()}>
          {t('platform.runtimes.more')}
        </button>
      ) : null}
    </section>
  )
}

function RecordTable({
  marker,
  label,
  rows,
  nameLabelKey,
  columns,
  sort,
  onSort,
  openId,
  onToggle,
  onOpen,
  detailExtra,
  sortableKeys,
  rowActions,
  motion,
  selection,
  selectionDisabled = false
}: {
  marker: string
  label: string
  rows: readonly PlatformItem[]
  nameLabelKey: string
  columns: Column[]
  sort: SortState
  onSort: (key: string) => void
  openId: string | null
  onToggle: (id: string) => void
  onOpen?: (item: PlatformItem) => void
  detailExtra?: (item: PlatformItem) => ReactNode
  sortableKeys?: ReadonlySet<string>
  rowActions?: (item: PlatformItem) => ReactNode
  /** Optimistic state per record ID, from `useOptimisticRows`. */
  motion?: (id: string) => RowMotion | undefined
  selection?: CollectionRowSelection
  selectionDisabled?: boolean
}) {
  const { t } = useTranslation()
  const span = columns.length + (onOpen ? 2 : 1) + (rowActions ? 1 : 0) + (selection ? 1 : 0)
  const selected = new Set(selection?.ids)
  // Rows are memoized, so they receive stable callbacks that always reach the latest handlers.
  const latest = useRef({ onToggle, onOpen, select: selection?.toggle })
  useLayoutEffect(() => {
    latest.current = { onToggle, onOpen, select: selection?.toggle }
  })
  const toggleRow = useCallback((id: string) => latest.current.onToggle(id), [])
  const openRow = useCallback((item: PlatformItem) => latest.current.onOpen?.(item), [])
  const selectRow = useCallback((id: string, checked: boolean) => latest.current.select?.(id, checked), [])
  return (
    <table
      data-ui={marker}
      aria-label={label}
      style={{
        minWidth: Math.max(
          520,
          240 + columns.length * 112 + (selection ? 40 : 0) + (onOpen ? 120 : 0) + (rowActions ? 120 : 0)
        )
      }}
      className="w-full table-fixed border-collapse text-left">
      <thead className="sticky top-0 z-[1] bg-white">
        <tr className="border-b border-[#efefef]">
          {selection ? (
            <th scope="col" className="h-9 w-10 px-2">
              <Checkbox
                className={rowCheckboxClassName}
                ref={selection.headerRef}
                aria-label={t('collection.selectPage')}
                checked={selection.all ? true : selection.ids.length ? 'indeterminate' : false}
                disabled={selectionDisabled || rows.length === 0}
                onCheckedChange={(checked) => selection.togglePage(checked === true)}
              />
            </th>
          ) : null}
          <SortHeader label={t(nameLabelKey)} sortKey="name" sort={sort} onSort={onSort} className="pl-2.5" />
          {columns.map((column, index) => (
            <SortHeader
              key={column.key}
              label={t(column.labelKey)}
              sortKey={column.key}
              sort={sort}
              onSort={onSort}
              enabled={!sortableKeys || sortableKeys.has(column.key)}
              className={cn(
                columnWidth(column),
                isNumeric(column) && 'pr-4 text-right',
                index === columns.length - 1 && !onOpen && 'pr-2.5'
              )}
            />
          ))}
          {onOpen ? <th scope="col" className="h-9 pr-2.5 font-normal" /> : null}
          {rowActions ? (
            <th scope="col" className="h-9 w-[120px] px-2.5 text-right text-[12px] font-normal">
              {t('platform.supplierForm.actions')}
            </th>
          ) : null}
        </tr>
      </thead>
      <tbody>
        {rows.map((item, index) => (
          <RecordRow
            key={item.id}
            marker={marker}
            item={item}
            index={Math.min(index, STAGGER_CAP)}
            motion={motion?.(item.id)}
            columns={columns}
            span={span}
            open={openId === item.id}
            selectable={Boolean(selection)}
            selected={selected.has(item.id)}
            selectionDisabled={selectionDisabled}
            onToggle={toggleRow}
            onSelect={selectRow}
            onOpen={onOpen ? openRow : undefined}
            detailExtra={detailExtra}
            rowActions={rowActions}
          />
        ))}
      </tbody>
    </table>
  )
}

// 16px box; the pseudo-element keeps the 24px pointer target that section 14.12 requires.
const rowCheckboxClassName =
  "relative align-middle after:absolute after:-inset-1 after:content-[''] focus-visible:ring-[#171717] focus-visible:ring-offset-2"

// Rows past the stagger window enter together, so their position never needs to re-render them.
const STAGGER_CAP = 13

const RecordRow = memo(function RecordRow({
  marker,
  item,
  index,
  motion,
  columns,
  span,
  open,
  selectable,
  selected,
  selectionDisabled,
  onToggle,
  onSelect,
  onOpen,
  detailExtra,
  rowActions
}: {
  marker: string
  item: PlatformItem
  index: number
  motion?: RowMotion
  columns: Column[]
  span: number
  open: boolean
  selectable: boolean
  selected: boolean
  selectionDisabled: boolean
  onToggle: (id: string) => void
  onSelect: (id: string, checked: boolean) => void
  onOpen?: (item: PlatformItem) => void
  detailExtra?: (item: PlatformItem) => ReactNode
  rowActions?: (item: PlatformItem) => ReactNode
}) {
  const { t } = useTranslation()
  const detailId = `${marker}-detail-${item.id}`
  const notes = fieldValue(item, 'notes')
  return (
    <>
      <tr
        data-state={selected ? 'selected' : undefined}
        data-motion={motion}
        aria-busy={motion === 'pending' || undefined}
        style={rowMotionStyle(index, motion)}
        className={cn(
          'cursor-pointer',
          selected ? 'bg-[#f2f3f5]' : cn('hover:bg-[#f7f7f7]', open && 'bg-[#f7f7f7]'),
          rowMotionClassName(motion)
        )}
        onClick={(event) => {
          if ((event.target as HTMLElement).closest('button, input, a, [role=checkbox], label, [data-selection-cell]'))
            return
          onToggle(item.id)
        }}>
        {selectable ? (
          <td
            data-selection-cell
            className="h-12 w-10 px-2"
            onClick={(event) => {
              if (selectionDisabled || (event.target as HTMLElement).closest('[role=checkbox]')) return
              onSelect(item.id, !selected)
            }}>
            <Checkbox
              className={rowCheckboxClassName}
              aria-label={t('collection.selectRow', { name: item.name, id: item.id })}
              checked={selected}
              disabled={selectionDisabled}
              onCheckedChange={(checked) => onSelect(item.id, checked === true)}
            />
          </td>
        ) : null}
        <td className={cn('overflow-hidden pl-2.5', open ? 'min-h-12 py-1.5' : 'h-12')}>
          <button
            type="button"
            aria-expanded={open}
            aria-controls={detailId}
            onClick={() => onToggle(item.id)}
            className="flex w-full min-w-0 items-center gap-2.5 rounded-[8px] text-left outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset">
            <NameMark name={item.name} />
            <span className="min-w-0 flex-1">
              <span
                className={cn(
                  'block text-[13px] font-medium text-[#1a1a1a]',
                  open ? 'py-1 break-words whitespace-normal' : 'truncate'
                )}>
                {item.name}
              </span>
              {notes ? <span className="block truncate text-[12px] font-normal text-[#8a8a8a]">{notes}</span> : null}
            </span>
          </button>
        </td>
        {columns.map((column, index) => {
          const value = fieldValue(item, column.key)
          const flagged = ATTENTION_VALUES[column.key]?.has(value) ?? false
          const last = index === columns.length - 1 && !onOpen
          return (
            <td
              key={column.key}
              className={cn(
                'h-12 overflow-hidden text-[13px]',
                isNumeric(column) && 'pr-4 text-right tabular-nums',
                last && 'pr-2.5',
                column.muted ? 'text-[#8a8a8a]' : 'text-[#3f3f3f]'
              )}>
              {column.chip ? (
                <StatusChip value={value} />
              ) : (
                <span
                  data-attention={flagged || undefined}
                  className={cn('block truncate', flagged && 'flex items-center gap-1 font-medium text-[#171717]')}
                  title={(column.format === 'ago' ? formatTime(value) : value) || undefined}>
                  {flagged ? <span aria-hidden className="size-1.5 shrink-0 rounded-full bg-[#171717]" /> : null}
                  <span className="min-w-0 truncate">{cellText(column, column.key, value, t)}</span>
                </span>
              )}
            </td>
          )
        })}
        {onOpen ? (
          <td className="h-12 pr-2.5 text-right">
            <button type="button" disabled={!item.orgId} onClick={() => onOpen(item)} className={textButtonClassName}>
              {t('platform.channels.open')}
            </button>
          </td>
        ) : null}
        {rowActions ? <td className="h-12 px-2.5 text-right">{rowActions(item)}</td> : null}
      </tr>
      {open ? (
        <tr
          id={detailId}
          className={cn('border-b border-[#efefef]', motion === 'leaving' && rowMotionClassName(motion))}>
          <td colSpan={span} className="px-2.5">
            <RecordDetail item={item} columns={columns} extra={detailExtra?.(item)} />
          </td>
        </tr>
      ) : null}
    </>
  )
})

function ChannelsPage() {
  const navigate = useNavigate()
  const open = (orgId: string | undefined) => {
    if (!orgId) return
    writeCenterId(orgId)
    void navigate({ to: '/app/org' })
  }
  const view = viewFor('channels')
  return (
    <RecordPage
      titleKey="platform.nav.channels"
      descriptionKey="platform.channels.description"
      emptyKey="platform.channels.empty"
      marker="platform.channels"
      swrKey="studio/platform-partners"
      load={() => listPlatformItems('partners')}
      body={view.body}
      nameLabelKey={view.nameLabelKey}
      columns={view.columns}
      onOpen={(item) => open(item.orgId)}
    />
  )
}

function labeledValue(key: string, value: string, t: (key: string) => string) {
  if (!value) return '—'
  const labelKey = `${key}.${value}`
  const label = t(labelKey)
  return label === labelKey ? value : label
}

function SystemPage() {
  const { t } = useTranslation()
  const [facet, setFacet] = usePlatformFacet('system', SYSTEM_FACETS)
  const [kind, setKind] = usePlatformFacet('system.reference', REFERENCE_FACETS)
  const [channel, setChannel] = useState('desktop')
  const view = viewFor('system', facet)
  const catalog = facet === 'reference' ? kind : facet
  return (
    <RecordPage
      titleKey="platform.nav.system"
      descriptionKey="platform.system.description"
      emptyKey="platform.system.empty"
      marker={facet === 'reference' ? `platform.reference.${kind}` : `platform.system.${facet}`}
      swrKey={`studio/platform/system/${catalog}`}
      load={() => listPlatformItems('system', catalog)}
      body={view.body}
      nameLabelKey={view.nameLabelKey}
      columns={view.columns}
      cardTitleKey={view.cardTitleKey}
      rowFilter={
        facet === 'apps'
          ? (item) => {
              const value = fieldValue(item, 'channel')
              return value === '' || value === channel
            }
          : undefined
      }
      segments={
        <SegmentedControl
          size="sm"
          className="h-[30px]"
          aria-label={t('platform.nav.system')}
          value={facet}
          onValueChange={setFacet}
          options={SYSTEM_FACETS.map((id) => ({ value: id, label: t(`platform.system.${id}`) }))}
        />
      }
      below={
        facet === 'reference' ? (
          <div className="mb-4">
            <SegmentedControl
              size="sm"
              className="h-[30px]"
              aria-label={t('platform.system.reference')}
              value={kind}
              onValueChange={setKind}
              options={REFERENCE_FACETS.map((id) => ({ value: id, label: t(`platform.reference.${id}`) }))}
            />
          </div>
        ) : facet === 'apps' ? (
          <div className="mb-4">
            <SegmentedControl
              size="sm"
              className="h-[30px]"
              aria-label={t('platform.col.channel')}
              value={channel}
              onValueChange={setChannel}
              options={[
                { value: 'desktop', label: t('platform.apps.desktop') },
                { value: 'mobile', label: t('platform.apps.mobile') }
              ]}
            />
          </div>
        ) : null
      }
    />
  )
}
