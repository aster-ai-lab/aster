import { MODEL_CAPABILITY, type EndpointType, type ModelCapability } from '@shared/data/types/model'
import { readStudioRuntime } from '@shared/studioRuntime'

import type { ModelEntry } from './modelProviders'
import { protocolsToEndpointTypes } from './osGateway'

/** A model the organization publishes to members (`/v1/api/org/models`). */
export type OrgModel = {
  id: string
  name: string
  brand?: string
  description?: string
  /** Vendor icon key. It does not choose the gateway route. */
  catalogKey?: string
  /** Native gateway protocols this alias can be called with, when the catalog sends them. */
  protocols?: string[]
  capabilities: string[]
  contextWindow: number | null
  outputLimit: number | null
  inputPrice: number | null
  /** Prompt-cache hit. Null when the catalog does not publish one. */
  cacheReadPrice: number | null
  /** Prompt-cache write. Null when the catalog does not publish one. */
  cacheWritePrice: number | null
  outputPrice: number | null
  /** Billing coefficient. Null when the catalog omits it. 1 charges the list price. */
  priceMultiplier: number | null
  /**
   * Published intensity levels. Null means the catalog omitted the field and the
   * picker may infer. An empty list means the card has no intensity slider.
   */
  reasoningEfforts: string[] | null
  /** `self_hosted` / `vendor` / `relay` when available. */
  source?: string
  available: boolean
}

export type OrgModelCatalog = {
  items: OrgModel[]
  defaultModel?: string
  autoModel?: string
}

type Raw = Record<string, unknown>

const text = (value: unknown) => (typeof value === 'string' && value ? value : undefined)
const stringList = (value: unknown) => {
  if (!Array.isArray(value)) return undefined
  const items = value.filter((item): item is string => typeof item === 'string' && item.length > 0)
  return items.length > 0 ? items : undefined
}

/** Brand slug used to group the picker. Missing brands share one platform group. */
export function brandGroupId(brand: string | undefined): string {
  const slug = (brand ?? 'a3s').trim().toLowerCase().replace(/\s+/g, '-')
  return slug || 'a3s'
}

/** Icon id. A catalog key supplies the vendor mark; the brand stands in when it is absent. */
export function orgModelLogoId(model: Pick<OrgModel, 'brand' | 'catalogKey'>): string {
  const slash = model.catalogKey?.indexOf('/') ?? -1
  if (model.catalogKey && slash > 0) return model.catalogKey.slice(0, slash)
  return brandGroupId(model.brand)
}

/** A missing or non-array capabilities list grants nothing. */
function listedCapabilities(model: { capabilities?: readonly unknown[] | null }): readonly string[] {
  return Array.isArray(model.capabilities)
    ? model.capabilities.filter((item): item is string => typeof item === 'string')
    : []
}

export function orgModelCapabilities(model: { capabilities?: readonly unknown[] | null }): ModelCapability[] {
  const capabilities = listedCapabilities(model)
  const mapped: ModelCapability[] = []
  if (capabilities.includes('tool_call')) mapped.push(MODEL_CAPABILITY.FUNCTION_CALL)
  if (capabilities.includes('reasoning')) mapped.push(MODEL_CAPABILITY.REASONING)
  if (capabilities.includes('vision')) mapped.push(MODEL_CAPABILITY.IMAGE_RECOGNITION)
  if (capabilities.includes('embedding')) mapped.push(MODEL_CAPABILITY.EMBEDDING)
  if (capabilities.includes('rerank')) mapped.push(MODEL_CAPABILITY.RERANK)
  if (capabilities.includes('image_generation')) mapped.push(MODEL_CAPABILITY.IMAGE_GENERATION)
  return mapped
}

export function orgModelEndpointTypes(model: Pick<OrgModel, 'protocols'>): EndpointType[] {
  return protocolsToEndpointTypes(model.protocols)
}

const count = (value: unknown) => (typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : null)
const price = (value: unknown) => (typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null)
const multiplier = (value: unknown) =>
  typeof value === 'number' && Number.isFinite(value) && value >= 0.01 && value <= 1000 ? value : null
const record = (value: unknown): Raw => (value && typeof value === 'object' ? (value as Raw) : {})

export function toOrgModel(raw: Raw): OrgModel | null {
  const id = text(raw.id)
  if (!id) return null
  return {
    id,
    name: text(raw.name) ?? id,
    brand: text(raw.brand),
    description: text(raw.description),
    catalogKey: text(raw.catalogKey),
    protocols: stringList(raw.protocols),
    capabilities: Array.isArray(raw.capabilities)
      ? raw.capabilities.filter((item): item is string => typeof item === 'string')
      : [],
    contextWindow: count(raw.contextWindow),
    outputLimit: count(raw.outputLimit),
    inputPrice: price(raw.inputPrice),
    cacheReadPrice: price(raw.cacheReadPrice),
    cacheWritePrice: price(raw.cacheWritePrice),
    outputPrice: price(raw.outputPrice),
    priceMultiplier: multiplier(raw.priceMultiplier),
    reasoningEfforts: Array.isArray(raw.reasoningEfforts)
      ? raw.reasoningEfforts.filter((item): item is string => typeof item === 'string' && item.length > 0)
      : null,
    source: text(raw.source),
    available: raw.available === true
  }
}

/**
 * Picker row for an org model. The chat model id is the offering alias. The brand
 * is only the group, and the catalog key is only the vendor icon.
 */
export function orgModelAsCatalogEntry(model: OrgModel): ModelEntry {
  const providerId = brandGroupId(model.brand)
  const endpointTypes = orgModelEndpointTypes(model)
  const capabilities = listedCapabilities(model)
  return {
    key: `org:${model.id}`,
    providerId,
    providerName: model.brand?.trim() || providerId,
    logoId: orgModelLogoId(model),
    id: model.id,
    name: model.name || model.id,
    description: model.description,
    reasoning: capabilities.includes('reasoning'),
    toolCall: capabilities.includes('tool_call'),
    attachment: false,
    contextWindow: model.contextWindow,
    outputLimit: model.outputLimit,
    inputModalities: capabilities.includes('vision') ? ['text', 'image'] : ['text'],
    outputModalities: capabilities.includes('image_generation') ? ['image'] : ['text'],
    inputCost: model.inputPrice,
    outputCost: model.outputPrice,
    cacheReadCost: model.cacheReadPrice,
    cacheWriteCost: model.cacheWritePrice,
    ...(model.priceMultiplier != null ? { priceMultiplier: model.priceMultiplier } : {}),
    ...(endpointTypes.length > 0 ? { endpointTypes } : {}),
    ...(model.reasoningEfforts != null ? { reasoningEfforts: model.reasoningEfforts } : {})
  }
}

/** Reads the published catalog through the shell, which adds the member session. */
export async function fetchOrgModels(signal?: AbortSignal): Promise<OrgModelCatalog> {
  const runtime = readStudioRuntime()
  if (!runtime) throw new Error('studio runtime is unavailable')
  const response = await fetch(`${runtime.apiBaseUrl}/v1/api/org/models`, {
    headers: { authorization: `Bearer ${runtime.token}` },
    signal
  })
  const envelope = record(await response.json().catch(() => null))
  if (!response.ok || envelope.success === false) {
    throw new Error(text(envelope.message) ?? `org models returned HTTP ${response.status}`)
  }
  // The member API wraps the catalog in `{ success, data }`. A bare catalog still lists.
  const data = envelope.data === undefined ? envelope : record(envelope.data)
  return {
    items: (Array.isArray(data.items) ? data.items : [])
      .map((item) => toOrgModel(record(item)))
      .filter((item): item is OrgModel => item !== null),
    defaultModel: text(data.defaultModel),
    autoModel: text(data.autoModel)
  }
}
