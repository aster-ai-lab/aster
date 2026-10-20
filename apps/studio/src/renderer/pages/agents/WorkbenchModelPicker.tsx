import { Popover, PopoverContent, PopoverTrigger, Slider } from '@a3s-lab/ui'
import { Brain, Check, ChevronLeft, ChevronRight, Loader2, RefreshCw, Search, Sparkles, X } from 'lucide-react'
import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { inferReasoningControls, REASONING_EFFORT_ORDER, type ReasoningEffort } from '@cherrystudio/provider-registry'
import { usePreference } from '@data/hooks/usePreference'
import { useMemberProfile } from '@renderer/components/account/memberProfile'
import { type ModelEntry, OFFICIAL_PROVIDERS, providerLogoId, studioProviderId } from '@renderer/data/modelProviders'
import { orgModelAsCatalogEntry, type OrgModel } from '@renderer/data/orgModels'
import { OS_GATEWAY_AUTO_MODEL_ID, OS_GATEWAY_PROVIDER_ID } from '@renderer/data/osGateway'
import { useProviders } from '@renderer/hooks/useProvider'
import { useModelMutations, useModels } from '@renderer/hooks/useModel'
import { ProviderAvatar } from '@renderer/pages/settings/ProviderSettings/components/ProviderAvatar'
import { toast } from '@renderer/services/toast'
import type { ThinkingOption } from '@renderer/types/reasoning'
import { formatErrorMessageWithPrefix } from '@renderer/utils/error'
import { cn } from '@renderer/utils/style'
import { MODEL_CAPABILITY, type ModelCapability } from '@shared/data/types/model'
import { createUniqueModelId, type Model } from '@shared/data/types/model'
import type { Provider } from '@shared/data/types/provider'
import { isEmbeddingModel, isGenerateImageModel, isRerankModel } from '@shared/utils/model'

import { useOrgModels } from './useOrgModels'
import { writePendingReasoningEffort } from './workbenchReasoningEffort'

const ALL_PROVIDERS = '__all__'
const PREVIEW_DISMISS_MS = 120

export type PickerOption = ModelEntry & {
  /** Registry provider that would serve this model, when Studio knows it. */
  studioProviderId?: string
  configured: boolean
}

export type PickerGroup = {
  key: string
  providerName: string
  /** Provider id the avatar resolves its logo from. */
  logoId: string
  configured: boolean
  options: PickerOption[]
}

/**
 * Brand groups, official vendors first among the rest. An organization model
 * (`org:` key) is served by the signed-in gateway and can be picked when the
 * catalog marks it available.
 */
export function buildPickerGroups(
  items: readonly ModelEntry[],
  knownStudioIds: ReadonlySet<string>,
  enabledStudioIds: ReadonlySet<string>,
  orgAvailable: ReadonlySet<string>
): PickerGroup[] {
  const groups = new Map<string, PickerGroup>()
  for (const item of items) {
    const managed = item.key.startsWith('org:')
    const studioId = managed ? OS_GATEWAY_PROVIDER_ID : studioProviderId(item.providerId, knownStudioIds)
    const configured = managed
      ? orgAvailable.has(item.key)
      : Boolean(studioId && enabledStudioIds.has(studioId)) && orgAvailable.has(item.key)
    const group = groups.get(item.providerId) ?? {
      key: item.providerId,
      providerName: item.providerName,
      logoId: item.logoId ?? studioId ?? providerLogoId(item.providerId),
      configured: false,
      options: []
    }
    group.configured ||= configured
    group.options.push({ ...item, studioProviderId: studioId, configured })
    groups.set(item.providerId, group)
  }
  const byName = (left: string, right: string) => left.localeCompare(right, 'en-US', { numeric: true })
  const officialRank = (key: string) => {
    const index = OFFICIAL_PROVIDERS.indexOf(key)
    return index === -1 ? OFFICIAL_PROVIDERS.length : index
  }
  return [...groups.values()]
    .map((group) => ({ ...group, options: group.options.toSorted((a, b) => byName(a.name, b.name)) }))
    .toSorted(
      (a, b) =>
        Number(b.configured) - Number(a.configured) ||
        officialRank(a.key) - officialRank(b.key) ||
        byName(a.providerName, b.providerName)
    )
}

/** A catalog row can be chosen only when the organization marks it available. */
export function catalogOptionSelectable(option: Pick<PickerOption, 'configured' | 'studioProviderId'>): boolean {
  return option.configured && Boolean(option.studioProviderId)
}

/**
 * Signed-in task picker rows. The published member catalog is the only source.
 * `query` narrows that list by name, alias, and brand.
 */
export function catalogPickerGroups(models: readonly OrgModel[], query: string): PickerGroup[] {
  const entries = models.map(orgModelAsCatalogEntry)
  const available = new Set<string>()
  models.forEach((model, index) => {
    if (model.available) available.add(entries[index].key)
  })
  return buildPickerGroups(filterOrgEntries(entries, query), new Set(), new Set(), available)
}

/** Local rows are selectable only from the enabled native provider/model records. */
export function localPickerGroups(models: readonly Model[], providers: readonly Provider[], query: string): PickerGroup[] {
  const groups: PickerGroup[] = []
  for (const provider of providers) {
    if (!provider.isEnabled || provider.id === OS_GATEWAY_PROVIDER_ID) continue
    const entries: PickerOption[] = models.filter((model) => model.providerId === provider.id && model.isEnabled && !model.isHidden &&
      !isEmbeddingModel(model) && !isRerankModel(model) && !isGenerateImageModel(model)).map((model) => ({
      key: `local:${model.id}`, providerId: provider.id, providerName: provider.name, logoId: provider.id,
      id: model.apiModelId, name: model.name || model.apiModelId,
      description: model.description, reasoning: model.capabilities?.includes(MODEL_CAPABILITY.REASONING) ?? false,
      toolCall: model.capabilities?.includes(MODEL_CAPABILITY.FUNCTION_CALL) ?? false,
      attachment: model.inputModalities?.includes('image') ?? false,
      contextWindow: model.contextWindow ?? null, outputLimit: model.maxOutputTokens ?? null,
      inputModalities: model.inputModalities ?? ['text'], outputModalities: model.outputModalities ?? ['text'],
      inputCost: null, outputCost: null, endpointTypes: model.endpointTypes,
      studioProviderId: provider.id, configured: true
    }))
    const filtered = filterOrgEntries(entries, query) as PickerOption[]
    if (filtered.length > 0) groups.push({ key: `local:${provider.id}`, providerName: provider.name, logoId: provider.id, configured: true, options: filtered })
  }
  return groups
}

/** Case-insensitive match on name, upstream id, and provider. */
export function filterOrgEntries(entries: readonly ModelEntry[], query: string): ModelEntry[] {
  const needle = query.trim().toLowerCase()
  if (!needle) return [...entries]
  return entries.filter((entry) =>
    [entry.name, entry.id, entry.providerName].some(
      (field) => typeof field === 'string' && field.toLowerCase().includes(needle)
    )
  )
}

export function compactTokenCount(value: number | null): string | null {
  if (!value) return null
  const scaled = (amount: number, unit: string) => `${Number.isInteger(amount) ? amount : amount.toFixed(1)}${unit}`
  if (value >= 1_000_000) return scaled(value / 1_000_000, 'M')
  if (value >= 1_000) return scaled(value / 1_000, 'K')
  return String(value)
}

/** USD per million tokens, without trailing zeros. */
export function formatUnitPrice(value: number): string {
  return value.toFixed(4).replace(/\.?0+$/, '')
}

/** Coefficient without trailing zeros. */
export function formatPriceMultiplier(value: number): string {
  return value.toFixed(4).replace(/\.?0+$/, '') || '0'
}

/** Member-facing rate. Cache parts appear only when the catalog publishes them. */
export function formatPublishedRate(
  t: (key: string, values: { value: string }) => string,
  rate: { input: number; output: number; cacheRead?: number | null; cacheWrite?: number | null }
): string {
  const part = (key: string, value: number) => t(key, { value: formatUnitPrice(value) })
  const parts = [part('workbench.model_picker.price_input', rate.input)]
  if (rate.cacheRead != null) parts.push(part('workbench.model_picker.price_cache_read', rate.cacheRead))
  if (rate.cacheWrite != null) parts.push(part('workbench.model_picker.price_cache_write', rate.cacheWrite))
  parts.push(part('workbench.model_picker.price_output', rate.output))
  return parts.join(' · ')
}

const EFFORT_LABEL_KEYS: Record<ThinkingOption, string> = {
  default: 'assistants.settings.reasoning_effort.default',
  none: 'assistants.settings.reasoning_effort.off',
  minimal: 'assistants.settings.reasoning_effort.minimal',
  low: 'assistants.settings.reasoning_effort.low',
  medium: 'assistants.settings.reasoning_effort.medium',
  high: 'assistants.settings.reasoning_effort.high',
  xhigh: 'assistants.settings.reasoning_effort.xhigh',
  max: 'assistants.settings.reasoning_effort.max',
  ultra: 'assistants.settings.reasoning_effort.ultra',
  auto: 'assistants.settings.reasoning_effort.auto'
}

/** Effort the picker can represent. Models without a published or inferred level show nothing. */
export function visibleReasoningEffort(
  modelId: string,
  effort: ThinkingOption,
  published?: readonly string[] | null
): ThinkingOption | null {
  const ladder = reasoningEffortLadder(modelId, published)
  if (ladder.length < 1) return null
  return effort === 'default' || ladder.includes(effort as ReasoningEffort) ? effort : 'default'
}

const RISING_EFFORTS = new Set<ThinkingOption>(['minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra'])

/**
 * Stops on the intensity slider, weak to strong.
 * 关闭 sits below 默认. Explicit intensities sit above it. 自动 is a mode, not a higher intensity.
 */
export function reasoningEffortScale(ladder: readonly ReasoningEffort[]): ThinkingOption[] {
  if (ladder.length < 1) return []
  const rising = REASONING_EFFORT_ORDER.filter((item) => ladder.includes(item) && RISING_EFFORTS.has(item))
  return [
    ...(ladder.includes('none') ? (['none'] as const) : []),
    'default',
    ...rising,
    ...(ladder.includes('auto') ? (['auto'] as const) : [])
  ]
}

/** Composer label for an intensity above 默认. 默认, 关闭, and 自动 stay off the chip. */
export function composerReasoningEffort(
  modelId: string,
  effort: ThinkingOption,
  published?: readonly string[] | null
): ThinkingOption | null {
  const shown = visibleReasoningEffort(modelId, effort, published)
  if (!shown || !RISING_EFFORTS.has(shown)) return null
  return shown
}

const EFFORT_TEXT_COLOR: Record<ThinkingOption, string> = {
  default: '#8a8a8a',
  none: '#b0b0b0',
  minimal: '#7aa2ff',
  low: '#2f6fe0',
  medium: '#c48a12',
  high: '#e06a12',
  xhigh: '#e14a2a',
  max: '#d2363c',
  ultra: '#a11d45',
  auto: '#5c6b80'
}

export function reasoningEffortColor(effort: ThinkingOption): string {
  return EFFORT_TEXT_COLOR[effort]
}

export function reasoningEffortLabelKey(effort: ThinkingOption): string {
  return EFFORT_LABEL_KEYS[effort]
}

/**
 * Intensity ladder for an org alias.
 * A published list is the authority, including a single level or an empty list.
 * When the catalog omits the field, the family table still fills in today's ladder.
 */
export function reasoningEffortLadder(modelId: string, published?: readonly string[] | null): ReasoningEffort[] {
  if (published) {
    const allowed = new Set(published)
    return REASONING_EFFORT_ORDER.filter((item) => allowed.has(item))
  }
  const controls = inferReasoningControls(modelId) ?? []
  const effort = controls.find((control) => control.kind === 'effort')
  const toggle = controls.some((control) => control.kind === 'toggle')
  const budget = controls.some((control) => control.kind === 'budget')
  let values: ReasoningEffort[] = []
  if (effort?.kind === 'effort') {
    values = [...effort.values]
    if (toggle && !values.includes('none')) values = [...values, 'none']
  } else if (budget) {
    values = [...(toggle ? (['none'] as const) : []), 'low', 'medium', 'high']
  } else if (toggle) {
    values = ['none', 'auto']
  }
  if (values.length < 2) return []
  const allowed = new Set(values)
  return REASONING_EFFORT_ORDER.filter((item) => allowed.has(item))
}

function capabilitiesOf(option: ModelEntry): ModelCapability[] {
  const capabilities: ModelCapability[] = []
  if (option.toolCall) capabilities.push(MODEL_CAPABILITY.FUNCTION_CALL)
  if (option.reasoning) capabilities.push(MODEL_CAPABILITY.REASONING)
  if (option.inputModalities?.includes('image')) capabilities.push(MODEL_CAPABILITY.IMAGE_RECOGNITION)
  return capabilities
}

function isSelected(option: PickerOption, model: Model | undefined): boolean {
  return Boolean(model && option.studioProviderId === model.providerId && option.id === model.apiModelId)
}

type Props = {
  model?: Model
  trigger: ReactNode
  /** Applies the picked model; without it the pick becomes the default model. */
  onSelect?: (model: Model) => void | Promise<void>
  /** Current turn effort. Models with a ladder can change it from the preview. */
  reasoningEffort?: ThinkingOption
  onReasoningEffortChange?: (effort: ThinkingOption) => void
}

type ProviderTab = { key: string; providerName: string; logoId?: string; count: number }

/** Horizontally scrolling tab strip: wheel scrolls sideways, and edge arrows appear when tabs overflow. */
function ProviderTabs({
  label,
  tabs,
  active,
  onChange
}: {
  label: string
  tabs: ProviderTab[]
  active: string
  onChange: (key: string) => void
}) {
  const { t } = useTranslation()
  const stripRef = useRef<HTMLDivElement>(null)
  const [edges, setEdges] = useState({ left: false, right: false })

  const measure = useCallback(() => {
    const strip = stripRef.current
    if (!strip) return
    const left = strip.scrollLeft > 1
    const right = strip.scrollLeft + strip.clientWidth < strip.scrollWidth - 1
    setEdges((current) => (current.left === left && current.right === right ? current : { left, right }))
  }, [])

  useEffect(() => {
    const strip = stripRef.current
    if (!strip) return
    // React registers wheel listeners as passive, so preventDefault needs a native listener.
    const onWheel = (event: WheelEvent) => {
      if (Math.abs(event.deltaY) <= Math.abs(event.deltaX) || strip.scrollWidth <= strip.clientWidth) return
      event.preventDefault()
      strip.scrollLeft += event.deltaY
    }
    strip.addEventListener('wheel', onWheel, { passive: false })
    const observer = new ResizeObserver(measure)
    observer.observe(strip)
    measure()
    return () => {
      strip.removeEventListener('wheel', onWheel)
      observer.disconnect()
    }
  }, [measure])

  useEffect(measure, [measure, tabs.length])

  useEffect(() => {
    stripRef.current
      ?.querySelector<HTMLElement>(`[data-provider-tab="${CSS.escape(active)}"]`)
      ?.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }, [active])

  const scrollBy = (direction: 1 | -1) => {
    const strip = stripRef.current
    strip?.scrollBy({ left: direction * strip.clientWidth * 0.7, behavior: 'smooth' })
  }

  const arrowClass =
    'absolute top-0 z-10 flex h-7 w-7 items-center justify-center rounded-full bg-white text-[#5c5c5c] shadow-[0_0_8px_6px_#fff] hover:text-[#171717]'

  return (
    <div className="relative px-3 pb-2">
      {edges.left ? (
        <button
          type="button"
          aria-label={t('workbench.model_picker.scroll_left')}
          onClick={() => scrollBy(-1)}
          className={cn(arrowClass, 'left-2')}>
          <ChevronLeft size={14} strokeWidth={1.8} />
        </button>
      ) : null}
      <div
        ref={stripRef}
        role="tablist"
        aria-label={label}
        onScroll={measure}
        className="flex [scrollbar-width:none] gap-1 overflow-x-auto overscroll-x-contain [&::-webkit-scrollbar]:hidden">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            type="button"
            role="tab"
            data-provider-tab={tab.key}
            aria-selected={tab.key === active}
            onClick={() => onChange(tab.key)}
            className={cn(
              'flex h-7 shrink-0 items-center gap-1 rounded-full px-2.5 text-[12px] text-[#5c5c5c] hover:bg-[#f5f5f5]',
              tab.logoId && 'pl-1',
              tab.key === active && 'bg-[#171717] text-white hover:bg-[#171717]'
            )}>
            {tab.logoId ? (
              <span className="flex size-5 shrink-0 items-center justify-center rounded-full bg-white">
                <ProviderAvatar provider={{ id: tab.logoId, name: tab.providerName }} size={16} />
              </span>
            ) : null}
            <span className="max-w-28 truncate">{tab.providerName}</span>
            <small className="opacity-60">{tab.count}</small>
          </button>
        ))}
      </div>
      {edges.right ? (
        <button
          type="button"
          aria-label={t('workbench.model_picker.scroll_right')}
          onClick={() => scrollBy(1)}
          className={cn(arrowClass, 'right-2')}>
          <ChevronRight size={14} strokeWidth={1.8} />
        </button>
      ) : null}
    </div>
  )
}

export function WorkbenchModelPicker({
  model,
  trigger,
  onSelect,
  reasoningEffort = 'default',
  onReasoningEffortChange
}: Props) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [activeProvider, setActiveProvider] = useState(ALL_PROVIDERS)
  const [revision, setRevision] = useState(0)
  const [preview, setPreview] = useState<{ option: PickerOption; top: number } | null>(null)
  const [draftEffort, setDraftEffort] = useState<ThinkingOption | null>(null)
  const [saving, setSaving] = useState(false)
  const contentRef = useRef<HTMLDivElement>(null)
  const dismissTimer = useRef<number | undefined>(undefined)

  const { models, isLoading: localModelsLoading } = useModels(undefined, { fetchEnabled: open })
  const { providers } = useProviders(undefined, { enabled: open && import.meta.env.PUBLIC_STUDIO_TENANT === 'aster' })
  const member = useMemberProfile()
  const localBuild = import.meta.env.PUBLIC_STUDIO_TENANT === 'aster'
  const organizationEnabled = !localBuild || member !== null
  const { createModel } = useModelMutations()
  const org = useOrgModels({ enabled: open && organizationEnabled, revision })
  const phase = localBuild ? (localModelsLoading && models.length === 0 ? 'loading' : 'ready') : org.phase
  const catalogItems = organizationEnabled ? org.catalog.items : []
  const [defaultModelId, setDefaultModelId] = usePreference('chat.default_model_id')
  const autoSelected = !defaultModelId
  const groups = useMemo(() => [
    ...(localBuild ? localPickerGroups(models, providers, query) : []),
    ...catalogPickerGroups(catalogItems, query)
  ], [localBuild, models, providers, catalogItems, query])
  const visibleGroups =
    activeProvider === ALL_PROVIDERS ? groups : groups.filter((group) => group.key === activeProvider)
  const total = groups.reduce((sum, group) => sum + group.options.length, 0)

  const cancelDismiss = () => {
    window.clearTimeout(dismissTimer.current)
    dismissTimer.current = undefined
  }
  const scheduleDismiss = () => {
    cancelDismiss()
    dismissTimer.current = window.setTimeout(() => setPreview(null), PREVIEW_DISMISS_MS)
  }
  const showPreview = (option: PickerOption, row: HTMLElement) => {
    cancelDismiss()
    const content = contentRef.current
    if (!content) return
    const top = row.getBoundingClientRect().top - content.getBoundingClientRect().top
    setPreview({ option, top: Math.max(0, top) })
  }

  const handleOpenChange = (next: boolean) => {
    setOpen(next)
    if (!next) {
      cancelDismiss()
      setPreview(null)
      setDraftEffort(null)
      setQuery('')
      setActiveProvider(ALL_PROVIDERS)
    }
  }

  const shownEffort = draftEffort ?? reasoningEffort
  const setEffort = (effort: ThinkingOption) => {
    setDraftEffort(effort)
    writePendingReasoningEffort(effort)
    onReasoningEffortChange?.(effort)
  }

  const resolveModel = async (option: PickerOption & { studioProviderId: string }) => {
    const id = createUniqueModelId(option.studioProviderId, option.id)
    return (
      models.find((existing) => existing.id === id) ??
      (await createModel({
        providerId: option.studioProviderId,
        modelId: option.id,
        name: option.name,
        capabilities: capabilitiesOf(option),
        ...(option.contextWindow ? { contextWindow: option.contextWindow } : {}),
        ...(option.outputLimit ? { maxOutputTokens: option.outputLimit } : {}),
        ...(option.endpointTypes?.length ? { endpointTypes: option.endpointTypes } : {})
      }))
    )
  }

  const choose = async (option: PickerOption) => {
    if (!catalogOptionSelectable(option)) return
    const studioId = option.studioProviderId
    setSaving(true)
    try {
      const picked = await resolveModel({ ...option, studioProviderId: studioId })
      if (import.meta.env.PUBLIC_STUDIO_TENANT === 'aster' && picked.providerId !== OS_GATEWAY_PROVIDER_ID) {
        const { writeShellModelDefault } = await import('@renderer/data/studioLocalProviderPlane')
        await writeShellModelDefault({ providerId: picked.providerId, modelId: picked.apiModelId })
      }
      await setDefaultModelId(picked.id)
      if (onSelect) await onSelect(picked)
      handleOpenChange(false)
    } catch (error) {
      toast.error(formatErrorMessageWithPrefix(error, t('workbench.model_picker.select_failed')))
    } finally {
      setSaving(false)
    }
  }

  const chooseAuto = async () => {
    if (!organizationEnabled || !org.catalog.autoModel) {
      toast.error(t('workbench.model_picker.auto_unavailable'))
      return
    }
    setSaving(true)
    try {
      const id = createUniqueModelId(OS_GATEWAY_PROVIDER_ID, OS_GATEWAY_AUTO_MODEL_ID)
      const picked =
        models.find((existing) => existing.id === id) ??
        (await createModel({
          providerId: OS_GATEWAY_PROVIDER_ID,
          modelId: OS_GATEWAY_AUTO_MODEL_ID,
          name: OS_GATEWAY_AUTO_MODEL_ID
        }))
      await setDefaultModelId(null)
      if (onSelect) await onSelect(picked)
      handleOpenChange(false)
    } catch (error) {
      toast.error(formatErrorMessageWithPrefix(error, t('workbench.model_picker.select_failed')))
    } finally {
      setSaving(false)
    }
  }

  const autoQuery = query.trim().toLowerCase()
  const showAuto =
    phase === 'ready' &&
    organizationEnabled && Boolean(org.catalog.autoModel) &&
    activeProvider === ALL_PROVIDERS &&
    (autoQuery.length === 0 ||
      t('workbench.model_picker.auto').toLowerCase().includes(autoQuery) ||
      t('workbench.composer.auto_model').toLowerCase().includes(autoQuery) ||
      'auto'.startsWith(autoQuery))

  const renderOption = (option: PickerOption) => {
    const selected = !autoSelected && isSelected(option, model)
    const context = compactTokenCount(option.contextWindow)
    return (
      <button
        key={option.key}
        type="button"
        role="option"
        aria-selected={selected}
        data-model-option={option.key}
        data-configured={option.configured ? 'true' : 'false'}
        aria-disabled={saving || !option.configured}
        onClick={() => {
          if (!saving) void choose(option)
        }}
        onPointerEnter={(event) => showPreview(option, event.currentTarget)}
        onFocus={(event) => showPreview(option, event.currentTarget)}
        onPointerLeave={scheduleDismiss}
        className={cn(
          'flex h-9 w-full items-center gap-2 rounded-[8px] px-2 text-left text-[13px] outline-none',
          option.configured ? 'hover:bg-[#f5f5f5] focus-visible:bg-[#f5f5f5]' : 'cursor-default',
          saving && 'opacity-60',
          selected && 'bg-[#f0f0f3]'
        )}>
        <span className="flex w-5 shrink-0 justify-center">
          <ProviderAvatar
            provider={{
              id: option.logoId ?? option.studioProviderId ?? providerLogoId(option.providerId),
              name: option.providerName
            }}
            size={18}
          />
        </span>
        <span className="flex min-w-0 flex-1 items-center gap-1">
          <span className={cn('truncate', option.configured ? 'text-[#171717]' : 'text-[#737373]')} title={option.name}>
            {option.name}
          </span>
          {option.reasoning ? <Brain size={13} strokeWidth={1.7} className="shrink-0 text-[#a3a3a3]" /> : null}
        </span>
        {option.configured ? null : (
          <span className="shrink-0 rounded-[4px] bg-[#f5f5f5] px-1.5 py-px text-[11px] text-[#8a8a8a]">
            {t('workbench.model_picker.unavailable')}
          </span>
        )}
        {option.priceMultiplier != null ? (
          <span className="shrink-0 text-[11px] text-[#737373] tabular-nums">
            ×{formatPriceMultiplier(option.priceMultiplier)}
          </span>
        ) : null}
        <span className="w-10 shrink-0 text-right text-[11px] text-[#a3a3a3] tabular-nums">{context ?? ''}</span>
        <span className="flex w-4 shrink-0 justify-center">
          {selected ? <Check size={14} strokeWidth={2} className="text-[#171717]" /> : null}
        </span>
      </button>
    )
  }

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent
        ref={contentRef}
        side="top"
        align="end"
        sideOffset={8}
        aria-label={t('workbench.model_picker.title')}
        data-workbench-model-picker="true"
        className="relative w-[384px] overflow-visible rounded-[14px] border border-[#ebebeb] bg-white p-0 shadow-[0_12px_40px_rgba(0,0,0,0.12)]">
        <header className="flex items-center gap-2 px-3 pt-3 pb-2">
          <div className="min-w-0 flex-1">
            <div className="text-[14px] font-medium text-[#171717]">{t('workbench.model_picker.title')}</div>
            <div className="truncate text-[12px] text-[#8a8a8a]">
              {autoSelected
                ? t('workbench.composer.auto_model')
                : (model?.name ?? t('workbench.composer.choose_model'))}
            </div>
          </div>
          <button
            type="button"
            aria-label={t('workbench.model_picker.close')}
            onClick={() => handleOpenChange(false)}
            className="flex size-7 items-center justify-center rounded-[8px] text-[#8a8a8a] hover:bg-[#f5f5f5] hover:text-[#171717]">
            <X size={15} strokeWidth={1.8} />
          </button>
        </header>
        <div className="px-3 pb-2">
          <label className="flex h-8 items-center gap-2 rounded-[8px] bg-[#f5f5f5] px-2.5 text-[13px] focus-within:ring-2 focus-within:ring-[#171717]/10">
            <Search size={14} strokeWidth={1.8} className="shrink-0 text-[#a3a3a3]" />
            <input
              autoFocus
              value={query}
              onChange={(event) => {
                setQuery(event.target.value)
                setActiveProvider(ALL_PROVIDERS)
              }}
              placeholder={t('workbench.model_picker.search')}
              aria-label={t('workbench.model_picker.search')}
              className="min-w-0 flex-1 bg-transparent text-[#171717] outline-none placeholder:text-[#a3a3a3]"
            />
          </label>
        </div>
        {groups.length > 0 ? (
          <ProviderTabs
            label={t('workbench.model_picker.providers')}
            active={activeProvider}
            onChange={setActiveProvider}
            tabs={[
              { key: ALL_PROVIDERS, providerName: t('workbench.model_picker.all'), count: total },
              ...groups.map((group) => ({
                key: group.key,
                providerName: group.providerName,
                logoId: group.logoId,
                count: group.options.length
              }))
            ]}
          />
        ) : null}
        <div className="h-[280px] overflow-y-auto px-2 pb-2" onScroll={() => setPreview(null)}>
          {(phase === 'idle' || phase === 'loading') && catalogItems.length === 0 ? (
            <div role="status" className="flex h-full items-center justify-center gap-2 text-[13px] text-[#8a8a8a]">
              <Loader2 size={14} className="animate-spin" />
              {t('workbench.model_picker.loading')}
            </div>
          ) : phase === 'error' ? (
            <div className="flex h-full flex-col items-center justify-center gap-2 text-[13px] text-[#8a8a8a]">
              {t('workbench.model_picker.error')}
              <button
                type="button"
                onClick={() => setRevision((value) => value + 1)}
                className="flex h-7 items-center gap-1 rounded-[8px] px-2 text-[#171717] hover:bg-[#f5f5f5]">
                <RefreshCw size={13} strokeWidth={1.8} />
                {t('workbench.model_picker.retry')}
              </button>
            </div>
          ) : visibleGroups.length === 0 && phase === 'ready' && !showAuto ? (
            <div className="flex h-full items-center justify-center text-[13px] text-[#8a8a8a]">
              {t('workbench.model_picker.empty')}
            </div>
          ) : (
            <>
              {showAuto ? (
                <button
                  type="button"
                  role="option"
                  aria-selected={autoSelected}
                  data-model-option="auto"
                  aria-disabled={saving}
                  onClick={() => {
                    if (!saving) void chooseAuto()
                  }}
                  className={cn(
                    'mt-1 flex w-full items-center gap-2 rounded-[8px] px-2 py-2 text-left outline-none hover:bg-[#f5f5f5] focus-visible:bg-[#f5f5f5]',
                    saving && 'opacity-60',
                    autoSelected && 'bg-[#f0f0f3]'
                  )}>
                  <span className="flex w-5 shrink-0 justify-center text-[#171717]">
                    <Sparkles size={16} strokeWidth={1.8} />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13px] text-[#171717]">
                      {t('workbench.model_picker.auto')}
                    </span>
                    <span className="block truncate text-[11px] text-[#8a8a8a]">
                      {t('workbench.model_picker.auto_hint')}
                    </span>
                  </span>
                  <span className="flex w-4 shrink-0 justify-center">
                    {autoSelected ? <Check size={14} strokeWidth={2} className="text-[#171717]" /> : null}
                  </span>
                </button>
              ) : null}
              {visibleGroups.map((group) => (
                <section key={group.key} aria-label={group.providerName}>
                  {activeProvider === ALL_PROVIDERS ? (
                    <h3 className="px-2 pt-2 pb-1 text-[11px] font-medium text-[#a3a3a3]">{group.providerName}</h3>
                  ) : null}
                  <div role="listbox" aria-label={group.providerName}>
                    {group.options.map(renderOption)}
                  </div>
                </section>
              ))}
            </>
          )}
        </div>
        {preview ? (
          <ModelPreviewCard
            option={preview.option}
            top={preview.top}
            reasoningEffort={shownEffort}
            onReasoningEffortChange={setEffort}
            onPointerEnter={cancelDismiss}
            onPointerLeave={scheduleDismiss}
          />
        ) : null}
      </PopoverContent>
    </Popover>
  )
}

function ModelPreviewCard({
  option,
  top,
  reasoningEffort,
  onReasoningEffortChange,
  onPointerEnter,
  onPointerLeave
}: {
  option: PickerOption
  top: number
  reasoningEffort: ThinkingOption
  onReasoningEffortChange: (effort: ThinkingOption) => void
  onPointerEnter: () => void
  onPointerLeave: () => void
}) {
  const { t } = useTranslation()
  const capabilities = [
    option.reasoning && t('workbench.model_picker.capability.reasoning'),
    option.toolCall && t('workbench.model_picker.capability.tools'),
    option.inputModalities?.includes('image') && t('workbench.model_picker.capability.vision')
  ].filter(Boolean)
  const context = compactTokenCount(option.contextWindow)
  const output = compactTokenCount(option.outputLimit)
  const price =
    option.inputCost !== null && option.outputCost !== null
      ? formatPublishedRate(t, {
          input: option.inputCost,
          output: option.outputCost,
          cacheRead: option.cacheReadCost,
          cacheWrite: option.cacheWriteCost
        })
      : null
  const multiplier =
    option.priceMultiplier != null
      ? t('workbench.model_picker.multiplier_value', { value: formatPriceMultiplier(option.priceMultiplier) })
      : null
  const efforts = option.configured
    ? reasoningEffortScale(reasoningEffortLadder(option.id, option.reasoningEfforts))
    : []
  const selected = efforts.includes(reasoningEffort) ? reasoningEffort : 'default'
  const selectedIndex = Math.max(0, efforts.indexOf(selected))
  return (
    <aside
      data-model-preview="true"
      aria-label={option.name}
      onPointerEnter={onPointerEnter}
      onPointerLeave={onPointerLeave}
      style={{ top }}
      className="absolute right-full mr-2 w-[288px] rounded-[12px] border border-[#ebebeb] bg-white p-3 text-[12px] shadow-[0_12px_40px_rgba(0,0,0,0.12)]">
      <div className="text-[13px] font-medium text-[#171717]">{option.name}</div>
      <div className="mt-0.5 text-[#8a8a8a]">{[option.providerName, ...capabilities].join(' · ')}</div>
      {option.description ? <p className="mt-2 line-clamp-3 text-[#5c5c5c]">{option.description}</p> : null}
      {context || output || price || multiplier ? (
        <dl className="mt-2 space-y-1">
          {context ? <PreviewFact label={t('workbench.model_picker.context')} value={context} /> : null}
          {output ? <PreviewFact label={t('workbench.model_picker.output')} value={output} /> : null}
          {price ? <PreviewFact label={t('workbench.model_picker.price')} value={price} /> : null}
          {multiplier ? <PreviewFact label={t('workbench.model_picker.multiplier')} value={multiplier} /> : null}
        </dl>
      ) : null}
      {efforts.length > 1 ? (
        <ReasoningEffortSlider
          efforts={efforts}
          selected={selected}
          selectedIndex={selectedIndex}
          onChange={onReasoningEffortChange}
        />
      ) : null}
    </aside>
  )
}

function ReasoningEffortSlider({
  efforts,
  selected,
  selectedIndex,
  onChange
}: {
  efforts: ThinkingOption[]
  selected: ThinkingOption
  selectedIndex: number
  onChange: (effort: ThinkingOption) => void
}) {
  const { t } = useTranslation()
  const label = t('workbench.model_picker.effort')
  return (
    <div className="mt-3" data-reasoning-effort={selected}>
      <div className="flex items-center justify-between gap-2">
        <div className="text-[11px] text-[#8a8a8a]">{label}</div>
        <div className="text-[11px] leading-none font-medium" style={{ color: reasoningEffortColor(selected) }}>
          {t(EFFORT_LABEL_KEYS[selected])}
        </div>
      </div>
      <Slider
        size="sm"
        min={0}
        max={efforts.length - 1}
        step={1}
        value={[selectedIndex]}
        aria-label={label}
        getThumbAriaLabel={() => label}
        getThumbAriaValueText={(value) => t(EFFORT_LABEL_KEYS[efforts[value] ?? selected])}
        onValueChange={([next]) => {
          const effort = efforts[next]
          if (effort && effort !== selected) onChange(effort)
        }}
        onPointerDown={(event) => event.stopPropagation()}
        marks={efforts.map((effort, index) => ({
          value: index,
          label: (
            <button
              type="button"
              className={cn('leading-none', effort === selected && 'font-medium')}
              style={{ color: reasoningEffortColor(effort) }}
              onPointerDown={(event) => event.stopPropagation()}
              onClick={(event) => {
                event.preventDefault()
                event.stopPropagation()
                if (effort !== selected) onChange(effort)
              }}>
              {t(EFFORT_LABEL_KEYS[effort])}
            </button>
          )
        }))}
        className="mt-2"
      />
    </div>
  )
}

function PreviewFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3">
      <dt className="text-[#8a8a8a]">{label}</dt>
      <dd className="text-right text-[#171717] tabular-nums">{value}</dd>
    </div>
  )
}
