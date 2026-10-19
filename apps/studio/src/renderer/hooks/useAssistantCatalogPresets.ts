import { useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { memberRequest } from '@renderer/data/memberOrgs'
import type { AixAgentDeclaration } from '@renderer/services/agents/aixAgentRuntime'
import { officialAgentAvatar } from '@renderer/services/agents/officialAgentAvatars'
import { listCapabilities, type CapabilityConfigField } from '@renderer/services/capabilities/capabilityCatalogue'
import { capabilityIconDataUrl } from '@renderer/services/capabilityIcon'
import type { CreateAssistantDto } from '@shared/data/api/schemas/assistants'
import { createUniqueModelId } from '@shared/data/types/model'
import { readStudioRuntime } from '@shared/studioRuntime'
import { PLUGIN_INSTALL_EVENT } from '@shared/types/pluginInstall'

import { useBundledCatalog } from './useBundledCatalog'

export const ASSISTANT_CATALOG_MY_TAB = '__mine__'

interface AssistantCatalogModel {
  id?: string
  provider?: string
  name?: string
  group?: string
}

export interface AssistantCatalogPreset {
  id: string
  name: string
  prompt?: string
  description?: string
  emoji?: string
  avatar?: string
  group?: string[]
  defaultModel?: AssistantCatalogModel
  /** Plugin slug. Installing this agent installs that `.aix` package. */
  packageSlug?: string
  installed?: boolean
  /** Shortcuts shown in the agent dialog. */
  actions?: string[]
  /** Parameters the member fills on this device. */
  config?: CapabilityConfigField[]
  /** Verified call schema. Present only when the installed package declared one. */
  parametersSchema?: string
  /** Run mode from the installed package. */
  agentRuntime?: AixAgentDeclaration
  /** Cloud asset binding from the capabilities DTO (runtime=cloud agents). */
  cloudAssetId?: string
  cloudAssetReleaseId?: string
}

function agentActions(prompts: string[] | undefined): string[] {
  return (prompts ?? []).map((prompt) => prompt.trim()).filter(Boolean)
}

function agentConfig(config: CapabilityConfigField[] | undefined): CapabilityConfigField[] {
  return config && config.length > 0 ? config : []
}

export interface AssistantCatalogTab {
  id: string
  label: string
  count: number
}

const ORDERED_GROUP_ALIASES = [
  ['精选', 'Featured'],
  ['职业', 'Career'],
  ['商业', 'Business'],
  ['工具', 'Tools'],
  ['语言', 'Language'],
  ['办公', 'Office'],
  ['通用', 'General'],
  ['写作', 'Writing'],
  ['编程', 'Programming'],
  ['情感', 'Emotional'],
  ['教育', 'Education'],
  ['创意', 'Creative'],
  ['学术', 'Academic'],
  ['设计', 'Design'],
  ['艺术', 'Art'],
  ['娱乐', 'Entertainment'],
  ['生活', 'Life']
]

const orderedGroupRank = new Map<string, number>()
ORDERED_GROUP_ALIASES.forEach((aliases, index) => {
  aliases.forEach((alias) => orderedGroupRank.set(alias, index))
})

function normalizePresets(value: unknown): AssistantCatalogPreset[] {
  if (!Array.isArray(value)) return []

  return value.filter((preset): preset is AssistantCatalogPreset => {
    return Boolean(
      preset &&
      typeof preset === 'object' &&
      typeof (preset as AssistantCatalogPreset).id === 'string' &&
      typeof (preset as AssistantCatalogPreset).name === 'string'
    )
  })
}

function getPresetGroups(preset: AssistantCatalogPreset): string[] {
  return Array.isArray(preset.group) ? preset.group.filter(Boolean) : []
}

function sortGroups(a: string, b: string) {
  const rankA = orderedGroupRank.get(a) ?? Number.MAX_SAFE_INTEGER
  const rankB = orderedGroupRank.get(b) ?? Number.MAX_SAFE_INTEGER
  if (rankA !== rankB) return rankA - rankB
  return a.localeCompare(b, 'zh')
}

export function sortAssistantCatalogGroups(groups: readonly string[]): string[] {
  return [...groups].sort(sortGroups)
}

export function buildAssistantCatalogTabs(
  presets: AssistantCatalogPreset[],
  mineCount: number,
  mineLabel: string
): AssistantCatalogTab[] {
  const counts = new Map<string, number>()

  presets.forEach((preset) => {
    getPresetGroups(preset).forEach((group) => counts.set(group, (counts.get(group) ?? 0) + 1))
  })

  const systemTabs = Array.from(counts.entries())
    .sort(([a], [b]) => sortGroups(a, b))
    .map(([id, count]) => ({
      id,
      label: id,
      count
    }))

  return [{ id: ASSISTANT_CATALOG_MY_TAB, label: mineLabel, count: mineCount }, ...systemTabs]
}

export function filterAssistantCatalogPresets(
  presets: AssistantCatalogPreset[],
  activeTab: string,
  search: string
): AssistantCatalogPreset[] {
  if (activeTab === ASSISTANT_CATALOG_MY_TAB) return []

  const keyword = search.trim().toLowerCase()
  return presets.filter((preset) => {
    if (!getPresetGroups(preset).includes(activeTab)) return false
    if (!keyword) return true

    return [preset.name, preset.description, preset.prompt]
      .filter(Boolean)
      .some((text) => text?.toLowerCase().includes(keyword))
  })
}

export function getAssistantPresetCatalogKey(preset: Pick<AssistantCatalogPreset, 'id'>) {
  return preset.id
}

export function toCreateAssistantDtoFromCatalogPreset(preset: AssistantCatalogPreset): CreateAssistantDto {
  const dto: CreateAssistantDto = {
    name: preset.name.trim(),
    prompt: preset.prompt?.trim() || ''
  }

  const description = preset.description?.trim()
  if (description) dto.description = description

  const emoji = preset.emoji?.trim()
  if (emoji) dto.emoji = emoji

  if (preset.defaultModel?.provider && preset.defaultModel.id) {
    dto.modelId = createUniqueModelId(preset.defaultModel.provider, preset.defaultModel.id)
  }

  return dto
}

async function loadCatalogPresets(resourcesPath: string, language: string) {
  const fileName = language === 'zh-CN' ? 'agents-zh.json' : 'agents-en.json'
  const content = await window.api.fs.read(`${resourcesPath}/data/${fileName}`, 'utf-8')
  return normalizePresets(JSON.parse(content))
}

const CATEGORY_GROUPS: Record<string, readonly [string, string]> = {
  office: ['办公', 'Office'],
  creative: ['创意', 'Creative'],
  design: ['设计', 'Design'],
  code: ['编程', 'Programming'],
  business: ['商业', 'Business'],
  productivity: ['工具', 'Tools'],
  science: ['学术', 'Academic'],
  education: ['教育', 'Education'],
  search: ['工具', 'Tools'],
  data: ['工具', 'Tools'],
  other: ['通用', 'General']
}

function publishedPreset(row: Record<string, unknown>, language: string): AssistantCatalogPreset | null {
  if (typeof row.slug !== 'string' || typeof row.name !== 'string' || !row.slug || !row.name.trim()) return null
  const chinese = language.toLowerCase().startsWith('zh')
  const category = typeof row.category === 'string' ? row.category : 'other'
  const pair = CATEGORY_GROUPS[category] ?? CATEGORY_GROUPS.other
  const groups = [chinese ? pair[0] : pair[1]]
  if (row.featured === true) groups.unshift(chinese ? '精选' : 'Featured')
  const supplied = typeof row.prompt === 'string' ? row.prompt.trim() : ''
  return {
    id: row.slug,
    name: row.name,
    prompt: supplied,
    description: typeof row.description === 'string' ? row.description : '',
    emoji: typeof row.emoji === 'string' ? row.emoji : '',
    avatar: officialAgentAvatar(row.slug),
    group: groups,
    actions: [],
    config: []
  }
}

/** Enabled Console catalog. A missing shell session leaves the bundled presets in place. */
export async function loadPublishedAssistantPresets(language: string): Promise<AssistantCatalogPreset[]> {
  if (!readStudioRuntime()) return []
  const data = await memberRequest('/api/catalog-agents?pageSize=100', { method: 'GET' }, fetch)
  if (!data || typeof data !== 'object' || !('items' in data)) return []
  const items = (data as { items?: unknown }).items
  if (!Array.isArray(items)) return []
  const catalog = items.flatMap((row) => {
    if (!row || typeof row !== 'object') return []
    const preset = publishedPreset(row as Record<string, unknown>, language)
    return preset ? [preset] : []
  })
  const plugins = await listCapabilities({ kind: 'plugin', page: 1, pageSize: 100 }).catch(() => null)
  // A package with a view is a plugin; its agent serves that view and is not listed here.
  const agentItems = (plugins?.items ?? []).filter(
    (item) => item.surfaces.includes('agent') && !item.surfaces.includes('ui') && item.packageType !== 'native'
  )
  const icons = await Promise.all(
    agentItems.map((item) => (item.iconUrl ? capabilityIconDataUrl(item.iconUrl) : Promise.resolve(null)))
  )
  const agentPackages = agentItems.flatMap((item, index): AssistantCatalogPreset[] => {
    const icon = icons[index]
    return [
      {
        id: item.slug,
        name: item.name,
        description: item.description,
        ...(icon ? { avatar: icon, emoji: icon } : {}),
        packageSlug: item.slug,
        installed: item.installed,
        actions: agentActions(item.prompts),
        config: agentConfig(item.config),
        ...(item.parametersSchema ? { parametersSchema: item.parametersSchema } : {}),
        ...(item.agentRuntime ? { agentRuntime: item.agentRuntime } : {}),
        ...(item.cloudAssetId && item.cloudAssetReleaseId
          ? {
              cloudBinding: {
                packageId: item.packageSlug ?? '',
                digest: '',
                agentAssetId: item.cloudAssetId,
                agentAssetReleaseId: item.cloudAssetReleaseId
              }
            }
          : {}),
        group: item.featured ? ['精选'] : []
      }
    ]
  })
  const packagedIds = new Set(agentPackages.map((preset) => preset.id))
  const packagedNames = new Set(agentPackages.map((preset) => preset.name.trim()).filter(Boolean))
  return [
    ...agentPackages,
    ...catalog.filter((preset) => !packagedIds.has(preset.id) && !packagedNames.has(preset.name.trim()))
  ]
}

function mergePresets(
  published: AssistantCatalogPreset[],
  bundled: AssistantCatalogPreset[]
): AssistantCatalogPreset[] {
  const publishedNames = new Set(published.map((preset) => preset.name.trim()).filter(Boolean))
  return [...published, ...bundled.filter((preset) => !publishedNames.has(preset.name.trim()))]
}

export const CATALOG_AGENT_ID_PREFIX = 'catalog:'

export function catalogAgentEntityId(slug: string) {
  return `${CATALOG_AGENT_ID_PREFIX}${slug}`
}

export function isCatalogAgentEntityId(id: string) {
  return id.startsWith(CATALOG_AGENT_ID_PREFIX)
}

/** Catalog rows that are not already present as a local agent with the same name. */
export function unpublishedCatalogAgents(
  localNames: ReadonlySet<string>,
  published: readonly AssistantCatalogPreset[]
): AssistantCatalogPreset[] {
  return published.filter((preset) => {
    const name = preset.name.trim()
    return name.length > 0 && !localNames.has(name)
  })
}

/** Enabled agents from the A3S OS member catalog. Bundled preset files stay out of this list. */
export function usePublishedCatalogAgents(enabled = true) {
  const { i18n } = useTranslation()
  const language = i18n?.resolvedLanguage ?? i18n?.language ?? 'en-US'
  const [agents, setAgents] = useState<AssistantCatalogPreset[]>([])
  const [isLoading, setIsLoading] = useState(enabled)
  const [error, setError] = useState<Error | null>(null)
  const [reloadKey, setReloadKey] = useState(0)
  const reload = useCallback(() => setReloadKey((key) => key + 1), [])

  useEffect(() => {
    if (!enabled) {
      setAgents([])
      setIsLoading(false)
      setError(null)
      return
    }
    let cancelled = false
    setIsLoading(true)
    void loadPublishedAssistantPresets(language)
      .then((rows) => {
        if (cancelled) return
        setAgents(rows)
        setError(null)
      })
      .catch((err: unknown) => {
        if (cancelled) return
        setAgents([])
        setError(err instanceof Error ? err : new Error(String(err)))
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [enabled, language, reloadKey])

  return { agents, error, isLoading, reload }
}

export function useAssistantCatalogPresets({ enabled = true }: { enabled?: boolean } = {}) {
  const { i18n } = useTranslation()
  const language = i18n?.resolvedLanguage ?? i18n?.language ?? 'en-US'
  const { isLoading, items: bundled } = useBundledCatalog({
    catalog: 'assistant presets',
    enabled,
    load: loadCatalogPresets
  })
  const [published, setPublished] = useState<AssistantCatalogPreset[]>([])

  useEffect(() => {
    if (!enabled) {
      setPublished([])
      return
    }
    let generation = 0
    let cancelled = false
    const load = () => {
      const current = ++generation
      void loadPublishedAssistantPresets(language)
        .then((rows) => {
          if (!cancelled && current === generation) setPublished(rows)
        })
        .catch(() => {
          if (!cancelled && current === generation) setPublished([])
        })
    }
    load()
    window.addEventListener(PLUGIN_INSTALL_EVENT, load)
    window.addEventListener('focus', load)
    return () => {
      cancelled = true
      window.removeEventListener(PLUGIN_INSTALL_EVENT, load)
      window.removeEventListener('focus', load)
    }
  }, [enabled, language])

  const presets = useMemo(() => mergePresets(published, bundled), [bundled, published])

  return {
    isLoading,
    presets
  }
}
