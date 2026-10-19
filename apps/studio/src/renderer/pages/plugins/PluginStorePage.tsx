import { EmptyState, PopConfirm, SearchInput, SegmentedControl, Tooltip } from '@a3s-lab/ui'
import {
  CodeXml,
  Download,
  GraduationCap,
  Home,
  LayoutGrid,
  PenTool,
  Pin,
  PinOff,
  Settings,
  Star,
  Trash2,
  type LucideIcon
} from 'lucide-react'
import { useEffect, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { useMemberProfile } from '@renderer/components/account/memberProfile'
import Scrollbar from '@renderer/components/Scrollbar'
import { readAixCatalogue, subscribeAixCatalogue } from '@renderer/services/plugins'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  HOME_PLUGIN_PIN_LIMIT,
  packageHasHomeCards,
  pinHomePlugin,
  reconcileHomePluginPins,
  unpinHomePlugin,
  useHomePluginPins
} from '../agents/homePins'
import { toastNotOpen } from '../agents/notOpenToast'
import {
  capabilityCategoryLabelKey,
  installCapability,
  listCapabilities,
  uninstallCapability,
  type CapabilityEntry
} from '../capabilities/capabilitiesData'
import { PLUGIN_INSTALL_EVENT } from '../capabilities/pluginInstallSync'
import { assertPluginMaterialized, pluginNeedsMaterialization } from './pluginCallable'
import { PluginCardArt } from './PluginCardArt'
import { PluginConfigDialog } from './PluginConfigDialog'
import { launchableUiPlugin, openInstalledUiPlugin } from './pluginLaunch'
import { pluginShipsOkf } from './pluginOkf'
import { PluginOkfConfig } from './PluginOkfConfig'
import { PLUGIN_ALL, PLUGIN_FEATURED, pluginCatalogEntry, pluginQuery, pluginTypeKey } from './pluginStoreData'

const SEARCH_DEBOUNCE_MS = 300

const VIEW_OPTIONS = [
  { value: 'market', labelKey: 'plugins.tab.market' },
  { value: 'installed', labelKey: 'plugins.tab.installed' }
] as const

type StoreView = (typeof VIEW_OPTIONS)[number]['value']

const CATEGORY_ICONS: Record<string, LucideIcon> = {
  all: Home,
  featured: Star,
  code: CodeXml,
  education: GraduationCap,
  productivity: LayoutGrid,
  design: PenTool
}

export default function PluginStorePage() {
  const { t } = useTranslation()
  const member = useMemberProfile()
  const namespace = member?.id ?? 'signed-out'
  const homePins = useHomePluginPins(member?.id ?? null)
  const catalogue = useSyncExternalStore(subscribeAixCatalogue, readAixCatalogue, readAixCatalogue)
  const homeFull = homePins.length >= HOME_PLUGIN_PIN_LIMIT
  useEffect(() => {
    reconcileHomePluginPins(member?.id ?? null, catalogue.packages)
  }, [member?.id, catalogue])
  const [view, setView] = useState<StoreView>('market')
  const [category, setCategory] = useState(PLUGIN_ALL)
  const [query, setQuery] = useState('')
  const [committedQuery, setCommittedQuery] = useState('')
  const [pendingSlug, setPendingSlug] = useState<string | null>(null)
  const [installedOverride, setInstalledOverride] = useState<Readonly<Record<string, boolean>>>({})
  const [applyFailures, setApplyFailures] = useState<Readonly<Record<string, boolean>>>({})
  const [okfPlugin, setOkfPlugin] = useState<CapabilityEntry | null>(null)
  const [configPlugin, setConfigPlugin] = useState<CapabilityEntry | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const timer = window.setTimeout(() => setCommittedQuery(query.trim()), SEARCH_DEBOUNCE_MS)
    return () => window.clearTimeout(timer)
  }, [query])

  const request = pluginQuery({ view, category, q: committedQuery })
  const { data, error, mutate } = useSWR(
    member?.id ? (['studio/plugins', namespace, view, category, committedQuery] as const) : null,
    () => listCapabilities(request)
  )
  useEffect(() => {
    const onInstall = () => {
      void mutate()
    }
    window.addEventListener(PLUGIN_INSTALL_EVENT, onInstall)
    return () => window.removeEventListener(PLUGIN_INSTALL_EVENT, onInstall)
  }, [mutate])

  const searching = committedQuery.length > 0
  const entries = (data?.items ?? []).filter(pluginCatalogEntry)
  // The catalog category list ignores the selected shelf, search, and which cards
  // this page draws. Rebuilding it from the current cards drops shelves whose
  // packages are hidden here, including 代码.
  const categories = data?.categories ?? []
  const filters = [
    { id: PLUGIN_ALL, label: t('capabilities.category.all') },
    { id: PLUGIN_FEATURED, label: t('capabilities.category.featured') },
    ...categories.map((row) => {
      const labelKey = capabilityCategoryLabelKey(row.id)
      return { id: row.id, label: labelKey ? t(labelKey) : row.id }
    })
  ]
  const heading = filters.find((row) => row.id === category)?.label ?? t('capabilities.category.all')
  const emptyFeatured = category === PLUGIN_FEATURED && !searching
  const emptyTitle = emptyFeatured ? 'capabilities.empty.featured' : 'capabilities.no_results'
  const emptyHint = emptyFeatured ? 'capabilities.empty.featured_hint' : 'capabilities.no_results_hint'

  useEffect(() => {
    if (!error) return
    toast.error(t('capabilities.load_failed'))
  }, [error, t])

  useEffect(() => {
    if (category === PLUGIN_ALL || category === PLUGIN_FEATURED || !data) return
    if (!categories.some((row) => row.id === category)) setCategory(PLUGIN_ALL)
  }, [categories, category, data])

  useEffect(() => {
    scrollRef.current?.scrollTo?.({ top: 0 })
  }, [category, view, committedQuery])

  const shownInstalled = (entry: CapabilityEntry) => {
    const key = `${namespace}:${entry.slug}`
    if (Object.prototype.hasOwnProperty.call(installedOverride, key)) return installedOverride[key]
    return entry.installed
  }

  const clearOverride = (slug: string) => {
    setInstalledOverride((current) => {
      if (!Object.prototype.hasOwnProperty.call(current, slug)) return current
      const next = { ...current }
      delete next[slug]
      return next
    })
  }

  const changeInstallation = async (entry: CapabilityEntry, installed: boolean) => {
    if (pendingSlug) return
    const key = `${namespace}:${entry.slug}`
    const removing = !installed
    const updating = installed && shownInstalled(entry)
    setPendingSlug(key)
    setInstalledOverride((current) => ({ ...current, [key]: !removing }))
    try {
      if (removing) {
        await uninstallCapability(entry.slug)
        unpinHomePlugin(member?.id ?? null, entry.slug)
      } else {
        await installCapability(entry.slug)
        if (entry.packageType === 'aix' || pluginNeedsMaterialization(entry.surfaces)) {
          await assertPluginMaterialized(
            entry.slug,
            entry.surfaces,
            entry.packageType === 'aix' ? entry.packageDigest : undefined
          )
        }
      }
      setApplyFailures((current) => ({ ...current, [key]: false }))
      await mutate()
      clearOverride(key)
    } catch (failure) {
      if (!removing && !updating) setApplyFailures((current) => ({ ...current, [key]: true }))
      await mutate().catch(() => undefined)
      clearOverride(key)
      toast.error(
        t(
          updating
            ? 'plugins.update_failed'
            : removing
              ? 'capabilities.uninstall_failed'
              : 'capabilities.install_failed',
          {
            name: entry.name
          }
        ) + (failure instanceof Error && failure.message ? ` · ${failure.message}` : '')
      )
    } finally {
      setPendingSlug(null)
    }
  }

  const openUi = async (entry: CapabilityEntry) => {
    const opened = await openInstalledUiPlugin(entry, shownInstalled(entry), catalogue)
    if (!opened) toastNotOpen(t)
  }

  if (okfPlugin) {
    return <PluginOkfConfig slug={okfPlugin.slug} name={okfPlugin.name} onBack={() => setOkfPlugin(null)} />
  }

  return (
    <div
      data-ui="plugins.page"
      className="@container/plugins flex h-full min-h-0 w-full min-w-0 flex-1 flex-col bg-white">
      <div className="shrink-0 bg-white">
        <div className="flex flex-col gap-4 px-6 pt-4 pb-4">
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-4">
              <h1 className="m-0 min-w-0 truncate text-[24px] leading-tight font-bold text-foreground">
                {t('capabilities.tab.plugins')}
              </h1>
              <div className="flex shrink-0 items-center gap-2">
                <SearchInput
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  onClear={() => setQuery('')}
                  clearLabel={t('common.clear')}
                  placeholder={t('capabilities.search_placeholder')}
                  size="sm"
                  containerClassName="h-[30px] w-[220px] border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0 dark:bg-[#2c2c2c]"
                />
                <SegmentedControl
                  aria-label={t('plugins.title')}
                  size="sm"
                  className="h-[30px]"
                  value={view}
                  options={VIEW_OPTIONS.map(({ value, labelKey }) => ({ value, label: t(labelKey) }))}
                  onValueChange={setView}
                />
              </div>
            </div>
            <p className="m-0 text-[15px] text-[#8a8a8a]">{t('capabilities.subtitle.plugins')}</p>
          </div>
        </div>
      </div>

      <div className="flex min-h-0 min-w-0 flex-1 flex-row">
        <nav
          aria-label={t('plugins.title')}
          className="flex w-[176px] min-w-[176px] shrink-0 flex-col gap-0.5 overflow-x-hidden overflow-y-auto border-r border-[#efefef] bg-white px-3 pt-2 pb-4">
          {filters.map((row) => {
            const Icon = CATEGORY_ICONS[row.id] ?? LayoutGrid
            const selected = row.id === category
            return (
              <button
                key={row.id}
                type="button"
                aria-current={selected ? 'page' : undefined}
                onClick={() => setCategory(row.id)}
                className={cn(
                  'flex h-9 shrink-0 items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] transition-colors outline-none',
                  'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
                  selected
                    ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                    : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
                )}>
                <Icon className="size-4 shrink-0 text-current" strokeWidth={1.7} />
                <span className="truncate">{row.label}</span>
              </button>
            )
          })}
        </nav>

        <Scrollbar ref={scrollRef} className="@container/catalog-grid min-h-0 min-w-0 flex-1">
          <div className="flex min-h-full w-full flex-col px-6 pt-4 pb-6">
            <section className="flex w-full flex-1 flex-col">
              <h2 className="m-0 text-[18px] font-semibold text-[#1a1a1a]">{heading}</h2>
              {data && entries.length === 0 ? (
                <EmptyState
                  illustration="inbox"
                  title={t(emptyTitle)}
                  description={t(emptyHint)}
                  actionLabel={
                    searching ? t('common.clear') : emptyFeatured ? t('capabilities.category.all') : undefined
                  }
                  onAction={
                    searching
                      ? () => {
                          setQuery('')
                          setCommittedQuery('')
                        }
                      : emptyFeatured
                        ? () => setCategory(PLUGIN_ALL)
                        : undefined
                  }
                  className="py-16"
                />
              ) : (
                <div className="mt-4 grid w-full grid-cols-1 gap-4 @[520px]/catalog-grid:grid-cols-2 @[860px]/catalog-grid:grid-cols-3 @[1180px]/catalog-grid:grid-cols-4">
                  {entries.map((entry) => {
                    const done = shownInstalled(entry)
                    const installedPackage = catalogue.packages.find((item) => item.slug === entry.slug)
                    const updateAvailable =
                      done &&
                      entry.packageType === 'aix' &&
                      entry.packageDigest &&
                      installedPackage?.packageDigest &&
                      installedPackage.packageDigest !== entry.packageDigest
                    const key = `${namespace}:${entry.slug}`
                    const applyFailed = applyFailures[key] === true
                    const typeKey = pluginTypeKey(entry.surfaces, entry.applet)
                    return (
                      <article
                        key={key}
                        className="relative flex min-w-0 flex-col overflow-hidden rounded-[14px] border border-[#ececec] bg-white shadow-[0_8px_22px_rgba(20,40,80,0.04)]">
                        <div className="relative h-[132px]">
                          <PluginCardArt entry={entry} />
                          {!applyFailed &&
                          done &&
                          (entry.config.length > 0 ||
                            catalogue.packages.some(
                              (item) =>
                                item.slug === entry.slug &&
                                item.containers.some((container) => container.location === 'activitybar')
                            )) ? (
                            <button
                              type="button"
                              data-ui="plugins.configure"
                              aria-label={t('plugins.okf.configure')}
                              onClick={() => setConfigPlugin(entry)}
                              className={cn(
                                'absolute top-2 z-20 flex h-7 items-center gap-1 rounded-[8px] bg-white/95 px-2 text-[12px] text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.12)] outline-none hover:bg-white focus-visible:ring-2 focus-visible:ring-black/10',
                                pluginShipsOkf({ installed: done, surfaces: entry.surfaces }) ? 'left-2' : 'right-2'
                              )}>
                              <Settings className="size-3.5" strokeWidth={1.7} aria-hidden />
                              {t('plugins.okf.configure')}
                            </button>
                          ) : null}
                          {!applyFailed && pluginShipsOkf({ installed: done, surfaces: entry.surfaces }) ? (
                            <button
                              type="button"
                              data-ui="plugins.okf.configure"
                              aria-label={t('plugins.okf.configure')}
                              onClick={() => setOkfPlugin(entry)}
                              className="absolute top-2 right-2 z-20 flex h-7 items-center gap-1 rounded-[8px] bg-white/95 px-2 text-[12px] text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.12)] outline-none hover:bg-white focus-visible:ring-2 focus-visible:ring-black/10">
                              <Settings className="size-3.5" strokeWidth={1.7} aria-hidden />
                              {t('plugins.okf.configure')}
                            </button>
                          ) : null}
                        </div>
                        <div className="flex flex-1 flex-col px-4 pt-3 pb-3.5">
                          <h3 className="m-0 truncate text-[14px] font-medium text-[#1a1a1a]">{entry.name}</h3>
                          <p className="m-0 mt-1.5 line-clamp-2 min-h-10 text-[12px] leading-5 text-[#8b8b8b]">
                            {entry.description}
                          </p>
                          {done && (entry.available === false || applyFailed) ? (
                            <p className="m-0 mt-2 text-[12px] text-[#8b8b8b]">
                              {t(entry.available === false ? 'plugins.unavailable' : 'plugins.device_apply_failed')}
                            </p>
                          ) : null}
                          <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
                            <span className="truncate text-[13px] text-[#8b8b8b]">
                              {typeKey ? t(typeKey) : ''}
                              {typeKey && entry.version ? ' · ' : ''}
                              {entry.version ?? ''}
                            </span>
                            <div className="flex min-w-0 flex-wrap items-center gap-2">
                              {updateAvailable ? (
                                <button
                                  type="button"
                                  disabled={pendingSlug === key}
                                  onClick={() => void changeInstallation(entry, true)}
                                  className="flex h-[30px] shrink-0 items-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-60">
                                  {t('plugins.update')}
                                </button>
                              ) : null}
                              {!applyFailed && launchableUiPlugin(entry, done, catalogue) ? (
                                <button
                                  type="button"
                                  data-ui="plugins.open"
                                  onClick={() => void openUi(entry)}
                                  className="flex h-[30px] shrink-0 items-center rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                                  {t('common.open')}
                                </button>
                              ) : null}
                              {done && packageHasHomeCards(installedPackage) ? (
                                <Tooltip
                                  content={
                                    homePins.includes(entry.slug)
                                      ? t('workbench.home.pin.remove')
                                      : homeFull
                                        ? t('workbench.home.pin.full')
                                        : t('workbench.home.pin.to_home')
                                  }
                                  delay={400}
                                  placement="top"
                                  sideOffset={6}>
                                  <button
                                    type="button"
                                    data-ui="plugins.pin"
                                    aria-label={
                                      homePins.includes(entry.slug)
                                        ? t('workbench.home.pin.remove')
                                        : t('workbench.home.pin.to_home')
                                    }
                                    disabled={pendingSlug === key || (!homePins.includes(entry.slug) && homeFull)}
                                    onClick={() =>
                                      homePins.includes(entry.slug)
                                        ? unpinHomePlugin(member?.id ?? null, entry.slug)
                                        : pinHomePlugin(member?.id ?? null, entry.slug)
                                    }
                                    className="grid size-[30px] shrink-0 place-items-center rounded-[8px] border border-[#e5e5e5] bg-white text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:pointer-events-none disabled:opacity-60">
                                    {homePins.includes(entry.slug) ? (
                                      <PinOff className="size-3.5" strokeWidth={1.7} aria-hidden />
                                    ) : (
                                      <Pin className="size-3.5" strokeWidth={1.7} aria-hidden />
                                    )}
                                  </button>
                                </Tooltip>
                              ) : null}
                              {done ? (
                                <PopConfirm
                                  tone="danger"
                                  title={t('capabilities.uninstall_confirm', { name: entry.name })}
                                  confirmLabel={t('capabilities.uninstall')}
                                  cancelLabel={t('common.cancel')}
                                  disabled={pendingSlug === key}
                                  onConfirm={() => void changeInstallation(entry, false)}>
                                  <button
                                    type="button"
                                    aria-pressed
                                    disabled={pendingSlug === key}
                                    className="flex h-[30px] shrink-0 items-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none hover:bg-[#262626] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-60">
                                    <Trash2 className="size-3.5 text-white" strokeWidth={1.7} aria-hidden />
                                    {t('capabilities.uninstall')}
                                  </button>
                                </PopConfirm>
                              ) : !done ? (
                                <button
                                  type="button"
                                  aria-pressed={false}
                                  disabled={pendingSlug === key}
                                  onClick={() => void changeInstallation(entry, true)}
                                  className="flex h-[30px] shrink-0 items-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-60">
                                  <Download className="size-3.5" strokeWidth={1.7} aria-hidden />
                                  {t('plugins.install')}
                                </button>
                              ) : null}
                            </div>
                          </div>
                        </div>
                        {entry.featured ? <FeaturedRibbon label={t('capabilities.category.featured')} /> : null}
                      </article>
                    )
                  })}
                </div>
              )}
            </section>
          </div>
        </Scrollbar>
      </div>
      <PluginConfigDialog
        entry={configPlugin}
        hasActivityBar={catalogue.packages.some(
          (item) =>
            item.slug === configPlugin?.slug &&
            item.containers.some((container) => container.location === 'activitybar')
        )}
        onOpenChange={(open) => !open && setConfigPlugin(null)}
      />
    </div>
  )
}

function FeaturedRibbon({ label }: { label: string }) {
  return (
    <div
      data-ui="plugins.ribbon"
      className="pointer-events-none absolute top-1.5 -left-[26px] z-10 w-[72px] -rotate-45 bg-[#3dba6c] py-px text-center text-[9px] leading-[14px] font-medium text-white shadow-[0_1px_2px_rgba(0,0,0,0.12)]">
      {label}
    </div>
  )
}
