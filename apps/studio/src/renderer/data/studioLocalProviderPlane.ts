import i18n from '@renderer/i18n/resolver'
import type { DataRequest, DataResponse } from '@shared/data/api/types'
import {
  createUniqueModelId,
  ENDPOINT_TYPE,
  MODALITY,
  parseUniqueModelId,
  type Model,
  type UniqueModelId
} from '@shared/data/types/model'
import {
  DEFAULT_PROVIDER_SETTINGS,
  type ApiKeyEntry,
  type AuthConfig,
  type Provider
} from '@shared/data/types/provider'
import { readStudioRuntime, type StudioRuntimeDescriptor } from '@shared/studioRuntime'

/**
 * The Aster build keeps BYOK providers in its private native store. Managed
 * a3s credentials and catalogs stay on the separate organization transport.
 */

const MODEL_SETTINGS = [
  'group',
  'capabilities',
  'endpointTypes',
  'inputModalities',
  'outputModalities',
  'contextWindow',
  'maxInputTokens',
  'maxOutputTokens',
  'supportsStreaming',
  'pricing'
] as const satisfies readonly (keyof Model)[]

type ShellModel = { id: string; name: string; enabled?: boolean; chat?: boolean } & Partial<
  Pick<Model, (typeof MODEL_SETTINGS)[number]>
>

function modelSettings(row: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(MODEL_SETTINGS.filter((key) => row[key] !== undefined).map((key) => [key, row[key]]))
}

class ProviderStoreError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string
  ) {
    super(message)
  }
}

type ShellProvider = {
  id: string
  presetProviderId?: string
  name: string
  baseUrl: string
  apiKeyUrl?: string
  authOptional?: boolean
  enabled?: boolean
  hasApiKey?: boolean
  chatModels?: ShellModel[]
}

type ShellDetail = ShellProvider & {
  apiKey?: string
  models?: ShellModel[]
}

const KEY_SUFFIX = ':key'
const GATEWAY_PROVIDER_ID = 'a3s'

function omitsGatewayProvider(): boolean { return true }

function rejectGatewayProvider(parts: string[], body: Record<string, unknown>, query: Record<string, unknown>) {
  const addressed = [parts[1], body.providerId, body.presetProviderId, query.providerId]
  if (addressed.some((id) => id === GATEWAY_PROVIDER_ID)) throw new Error('provider route not found')
}

function ok(request: DataRequest, data: unknown): DataResponse {
  return { id: request.id, status: 200, data, metadata: { timestamp: Date.now() } }
}

function fail(request: DataRequest, error: unknown): DataResponse {
  const status = error instanceof ProviderStoreError ? error.status : 400
  const code = error instanceof ProviderStoreError ? error.code : 'VALIDATION_ERROR'
  const message =
    code === 'model_in_use'
      ? i18n.t('models.error.in_use')
      : error instanceof Error
        ? error.message
        : 'provider request failed'
  return {
    id: request.id,
    status,
    error: { code, message, status },
    metadata: { timestamp: Date.now() }
  }
}

function planePath(path: string): boolean {
  return path === '/providers' || path.startsWith('/providers/') || path === '/models' || path.startsWith('/models/')
}

function segments(path: string): string[] {
  return path
    .split('/')
    .filter(Boolean)
    .map((part) => {
      try {
        return decodeURIComponent(part)
      } catch {
        return part
      }
    })
}

function runtimeOrNull(): StudioRuntimeDescriptor | null {
  try {
    return readStudioRuntime() ?? null
  } catch {
    return null
  }
}

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === 'object' ? (value as Record<string, unknown>) : {}
}

function asStringList(value: unknown): string[] {
  if (Array.isArray(value)) return value.map((item) => String(item)).filter(Boolean)
  if (typeof value === 'string' && value) return [value]
  return []
}

async function shell(
  runtime: StudioRuntimeDescriptor,
  method: 'GET' | 'POST' | 'DELETE',
  path: string,
  body?: unknown
): Promise<unknown> {
  const response = await fetch(`${runtime.apiBaseUrl}${path}`, {
    method,
    headers: {
      authorization: `Bearer ${runtime.token}`,
      ...(body === undefined ? {} : { 'content-type': 'application/json' })
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) })
  })
  const payload = await response.json().catch(() => null)
  if (!response.ok) {
    const message = record(payload).error
    throw new ProviderStoreError(
      response.status,
      typeof message === 'string' ? message : 'provider_store_error',
      typeof message === 'string' && message ? message : `provider store returned HTTP ${response.status}`
    )
  }
  return payload
}

async function listShell(runtime: StudioRuntimeDescriptor): Promise<ShellProvider[]> {
  const body = record(await shell(runtime, 'GET', '/api/providers'))
  return Array.isArray(body.providers) ? (body.providers as ShellProvider[]) : []
}

async function detailShell(runtime: StudioRuntimeDescriptor, id: string): Promise<ShellDetail> {
  return record(await shell(runtime, 'GET', `/api/providers/${encodeURIComponent(id)}`)) as ShellDetail
}

async function saveShell(runtime: StudioRuntimeDescriptor, id: string, body: Record<string, unknown>) {
  await shell(runtime, 'POST', `/api/providers/${encodeURIComponent(id)}`, body)
}

function toProvider(item: ShellProvider): Provider {
  const baseUrl = typeof item.baseUrl === 'string' ? item.baseUrl : ''
  const storedKey = typeof (item as ShellDetail).apiKey === 'string' ? ((item as ShellDetail).apiKey ?? '') : ''
  const hasKey = item.hasApiKey === true || storedKey.length > 0
  // Native explicitly identifies registry instances; never infer custom ownership from an id shape.
  const presetProviderId = item.presetProviderId
  return {
    id: item.id,
    ...(presetProviderId ? { presetProviderId } : {}),
    name: item.name,
    ...(item.apiKeyUrl ? { websites: { apiKey: item.apiKeyUrl } } : {}),
    endpointConfigs: {
      [ENDPOINT_TYPE.OPENAI_CHAT_COMPLETIONS]: { adapterFamily: 'openai-compatible', baseUrl }
    },
    defaultChatEndpoint: ENDPOINT_TYPE.OPENAI_CHAT_COMPLETIONS,
    authMethods: ['api-key'],
    authOptional: item.authOptional === true,
    reportsActualCost: false,
    apiKeys: hasKey ? [{ id: `${item.id}${KEY_SUFFIX}`, isEnabled: true }] : [],
    authType: 'api-key',
    settings: DEFAULT_PROVIDER_SETTINGS,
    isEnabled: item.enabled === true
  }
}

function toModel(providerId: string, model: ShellModel): Model | null {
  if (!model.id) return null
  let id: UniqueModelId
  try {
    id = createUniqueModelId(providerId, model.id)
  } catch {
    return null
  }
  return {
    id,
    providerId,
    apiModelId: model.id,
    name: model.name || model.id,
    ...Object.fromEntries(Object.entries(modelSettings(model)).filter(([, value]) => value !== null)),
    capabilities: model.capabilities ?? [],
    endpointTypes: model.endpointTypes ?? [ENDPOINT_TYPE.OPENAI_CHAT_COMPLETIONS],
    inputModalities: model.inputModalities ?? [MODALITY.TEXT],
    outputModalities: model.outputModalities ?? [MODALITY.TEXT],
    supportsStreaming: model.supportsStreaming ?? true,
    isEnabled: model.enabled !== false,
    isHidden: false
  }
}

function detailModels(providerId: string, detail: ShellDetail): Model[] {
  const rows = Array.isArray(detail.models) ? detail.models : (detail.chatModels ?? [])
  return rows.flatMap((model) => {
    const next = toModel(providerId, model)
    return next ? [next] : []
  })
}

function baseUrlFromPatch(body: Record<string, unknown>): string | undefined {
  const endpoints = body.endpointConfigs
  if (!endpoints || typeof endpoints !== 'object') return undefined
  for (const value of Object.values(endpoints as Record<string, unknown>)) {
    const row = record(value)
    if (typeof row.baseUrl === 'string') return row.baseUrl.trim()
  }
  return undefined
}

export async function listStudioLocalModelsIpc(input: unknown) {
  const runtime = runtimeOrNull()
  const providerId =
    input && typeof input === 'object' && 'providerId' in input
      ? String((input as { providerId?: unknown }).providerId || '')
      : ''
  if (!runtime || !providerId || (omitsGatewayProvider() && providerId === GATEWAY_PROVIDER_ID)) {
    return { ok: false, error: { code: 'INTERNAL', message: 'provider model list is unavailable' } }
  }
  try {
    await shell(runtime, 'POST', `/api/providers/${encodeURIComponent(providerId)}/sync`)
    const detail = await detailShell(runtime, providerId)
    return { ok: true, data: { models: detailModels(providerId, detail) } }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'provider model sync failed'
    return { ok: false, error: { code: 'INTERNAL', message } }
  }
}

async function dispatch(runtime: StudioRuntimeDescriptor, request: DataRequest): Promise<unknown> {
  const method = request.method
  const parts = segments(request.path)
  const query = request.params ?? {}
  const body = request.body && typeof request.body === 'object' ? (request.body as Record<string, unknown>) : {}
  rejectGatewayProvider(parts, body, query)

  if (parts[0] === 'providers' && parts.length === 1) {
    if (method === 'GET') {
      let providers = (await listShell(runtime)).map(toProvider)
      if (omitsGatewayProvider()) {
        providers = providers.filter((provider) => provider.id !== GATEWAY_PROVIDER_ID)
      }
      if (typeof query.enabled === 'boolean') {
        providers = providers.filter((provider) => provider.isEnabled === query.enabled)
      }
      return providers
    }
    if (method === 'POST') {
      const id = String(body.providerId || body.presetProviderId || '')
      if (!id) throw new Error('provider id is required')
      const patch: Record<string, unknown> = { enabled: true }
      if (typeof body.name === 'string' && body.name.trim()) patch.name = body.name.trim()
      const baseUrl = baseUrlFromPatch(body)
      if (baseUrl !== undefined) patch.baseUrl = baseUrl
      const keys = Array.isArray(body.apiKeys) ? body.apiKeys : []
      const first = record(keys[0])
      if (typeof first.key === 'string' && first.key) patch.apiKey = first.key
      await saveShell(runtime, id, patch)
      return toProvider(await detailShell(runtime, id))
    }
    throw new Error('provider route not found')
  }

  if (parts[0] === 'providers' && (parts[1] === 'order:batch' || parts[2] === 'order') && method === 'PATCH') {
    return undefined
  }

  if (parts[0] === 'providers' && parts.length === 2) {
    const id = parts[1]
    if (method === 'GET') return toProvider(await detailShell(runtime, id))
    if (method === 'PATCH') {
      const patch: Record<string, unknown> = {}
      if (typeof body.isEnabled === 'boolean') patch.enabled = body.isEnabled
      if (typeof body.name === 'string' && body.name.trim()) patch.name = body.name.trim()
      const baseUrl = baseUrlFromPatch(body)
      if (baseUrl !== undefined) patch.baseUrl = baseUrl
      if (Object.keys(patch).length > 0) await saveShell(runtime, id, patch)
      return toProvider(await detailShell(runtime, id))
    }
    if (method === 'DELETE') {
      await saveShell(runtime, id, { deleted: true })
      return undefined
    }
    throw new Error('provider route not found')
  }

  if (parts[0] === 'providers' && parts[2] === 'api-keys' && parts.length === 3) {
    const id = parts[1]
    if (method === 'GET') {
      const detail = await detailShell(runtime, id)
      const key = typeof detail.apiKey === 'string' ? detail.apiKey : ''
      const keys: ApiKeyEntry[] = key ? [{ id: `${id}${KEY_SUFFIX}`, key, isEnabled: true }] : []
      return { keys: query.enabled === true ? keys.filter((item) => item.isEnabled) : keys }
    }
    if (method === 'POST') {
      const key = typeof body.key === 'string' ? body.key.trim() : ''
      if (!key) throw new Error('api key is required')
      await saveShell(runtime, id, { apiKey: key })
      return { id: `${id}${KEY_SUFFIX}`, key, isEnabled: true }
    }
    if (method === 'PUT') {
      const keys = Array.isArray(body.keys) ? body.keys : []
      const first = record(keys.find((item) => record(item).isEnabled !== false) ?? keys[0])
      const key = typeof first.key === 'string' ? first.key : ''
      await saveShell(runtime, id, { apiKey: key })
      return undefined
    }
    throw new Error('provider route not found')
  }

  if (parts[0] === 'providers' && parts[2] === 'api-keys' && parts.length === 4) {
    const id = parts[1]
    if (method === 'DELETE') {
      await saveShell(runtime, id, { apiKey: '' })
      return undefined
    }
    if (method === 'PATCH') {
      if (typeof body.key === 'string') await saveShell(runtime, id, { apiKey: body.key })
      else if (body.isEnabled === false) await saveShell(runtime, id, { apiKey: '' })
      const detail = await detailShell(runtime, id)
      const key = typeof detail.apiKey === 'string' ? detail.apiKey : ''
      return key ? { id: `${id}${KEY_SUFFIX}`, key, isEnabled: true } : undefined
    }
    throw new Error('provider route not found')
  }

  if (parts[0] === 'providers' && parts[2] === 'auth-config' && method === 'GET') {
    const detail = await detailShell(runtime, parts[1])
    const config: AuthConfig = { type: 'api-key', required: detail.authOptional !== true }
    return config
  }

  if (parts[0] === 'providers' && parts[2] === 'preset' && method === 'GET') {
    const detail = await detailShell(runtime, parts[1])
    const provider = toProvider(detail)
    const fields = asStringList(query.fields)
    const preset: { endpointConfigs?: Provider['endpointConfigs'] | null; models?: Model[] } = {}
    if (fields.includes('endpointConfigs')) preset.endpointConfigs = provider.endpointConfigs ?? null
    if (fields.includes('models')) preset.models = detailModels(parts[1], detail)
    return preset
  }

  if (parts[0] === 'providers' && parts[2] === 'models:resolve' && method === 'GET') {
    const providerId = parts[1]
    return asStringList(query.ids).flatMap((modelId) => {
      const model = toModel(providerId, { id: modelId, name: modelId, enabled: true })
      return model ? [model] : []
    })
  }

  if (parts[0] === 'providers' && parts[2] === 'models:reconcile' && method === 'POST') {
    const providerId = parts[1]
    for (const dto of Array.isArray(body.toAdd) ? body.toAdd : []) {
      const row = record(dto)
      const modelId = String(row.modelId || row.apiModelId || '')
      if (!modelId) continue
      await shell(runtime, 'POST', `/api/providers/${encodeURIComponent(providerId)}/models`, {
        ...modelSettings(row),
        id: modelId,
        name: typeof row.name === 'string' && row.name.trim() ? row.name.trim() : modelId,
        enabled: true
      })
    }
    for (const id of asStringList(body.toRemove)) {
      let modelId = id
      try {
        modelId = parseUniqueModelId(id as UniqueModelId).modelId
      } catch {
        // The shell stores the upstream id.
      }
      await shell(
        runtime,
        'DELETE',
        `/api/providers/${encodeURIComponent(providerId)}/models/${encodeURIComponent(modelId)}`
      )
    }
    return detailModels(providerId, await detailShell(runtime, providerId))
  }

  if (parts[0] === 'models' && parts.length === 1) {
    if (method === 'GET') {
      const providerId = typeof query.providerId === 'string' ? query.providerId : ''
      if (providerId) return detailModels(providerId, await detailShell(runtime, providerId))
      const providers = await listShell(runtime)
      const models: Model[] = []
      for (const provider of providers) {
        if (provider.id === GATEWAY_PROVIDER_ID || provider.enabled !== true) continue
        const detail = await detailShell(runtime, provider.id)
        models.push(...detailModels(provider.id, detail))
      }
      return models
    }
    if (method === 'POST') {
      const dtos = Array.isArray(request.body) ? request.body : []
      const created: Model[] = []
      for (const dto of dtos) {
        const row = record(dto)
        const providerId = String(row.providerId || '')
        const modelId = String(row.modelId || '')
        if (!providerId || !modelId) continue
        const name = typeof row.name === 'string' && row.name.trim() ? row.name.trim() : modelId
        const detail = await shell(runtime, 'POST', `/api/providers/${encodeURIComponent(providerId)}/models`, {
          ...modelSettings(row),
          id: modelId,
          name,
          enabled: true
        })
        const model = detailModels(providerId, detail as ShellDetail).find((model) => model.apiModelId === modelId)
        if (model) created.push(model)
      }
      return created
    }
    if (method === 'PATCH') {
      const items = Array.isArray(request.body) ? request.body : []
      const updated: Model[] = []
      for (const item of items) {
        const row = record(item)
        const uniqueModelId = typeof row.uniqueModelId === 'string' ? row.uniqueModelId : ''
        if (!uniqueModelId) continue
        updated.push(await patchModel(runtime, uniqueModelId, record(row.patch)))
      }
      return updated
    }
    if (method === 'DELETE') {
      for (const id of asStringList(query.ids)) await dropModel(runtime, id)
      return undefined
    }
  }

  if (parts[0] === 'models' && parts.length > 1) {
    const uniqueModelId = parts.slice(1).join('/')
    if (method === 'GET') {
      const { providerId, modelId } = parseUniqueModelId(uniqueModelId as UniqueModelId)
      const model = detailModels(providerId, await detailShell(runtime, providerId)).find(
        (item) => item.apiModelId === modelId || item.id === uniqueModelId
      )
      if (!model) throw new Error('model not found')
      return model
    }
    if (method === 'PATCH') return patchModel(runtime, uniqueModelId, body)
    if (method === 'DELETE') {
      await dropModel(runtime, uniqueModelId)
      return undefined
    }
  }

  throw new Error('provider route not found')
}

async function dropModel(runtime: StudioRuntimeDescriptor, uniqueModelId: string) {
  let providerId = ''
  let modelId = uniqueModelId
  try {
    const parsed = parseUniqueModelId(uniqueModelId as UniqueModelId)
    providerId = parsed.providerId
    modelId = parsed.modelId
  } catch {
    return
  }
  await shell(
    runtime,
    'DELETE',
    `/api/providers/${encodeURIComponent(providerId)}/models/${encodeURIComponent(modelId)}`
  )
}

async function patchModel(
  runtime: StudioRuntimeDescriptor,
  uniqueModelId: string,
  patch: Record<string, unknown>
): Promise<Model> {
  const parsed = parseUniqueModelId(uniqueModelId as UniqueModelId)
  const current = detailModels(parsed.providerId, await detailShell(runtime, parsed.providerId)).find(
    (item) => item.id === uniqueModelId
  )
  if (!current) throw new Error('model not found')
  const name = typeof patch.name === 'string' && patch.name.trim() ? patch.name.trim() : current.name
  const enabled = typeof patch.isEnabled === 'boolean' ? patch.isEnabled : current.isEnabled
  const detail = await shell(runtime, 'POST', `/api/providers/${encodeURIComponent(parsed.providerId)}/models`, {
    ...modelSettings(patch),
    id: parsed.modelId,
    name,
    enabled
  })
  const next = detailModels(parsed.providerId, detail as ShellDetail).find((item) => item.id === uniqueModelId)
  if (!next) throw new Error('model not found')
  return next
}

export type ShellModelChoice = { providerId: string; modelId: string }

/** The host and renderer defaults are committed together for each explicit local choice. */
export async function writeShellModelDefault(choice: ShellModelChoice): Promise<void> {
  const runtime = runtimeOrNull()
  if (!runtime) throw new Error('studio runtime is unavailable')
  const providerId = choice.providerId.trim()
  const modelId = choice.modelId.trim()
  if (!providerId || !modelId || providerId === GATEWAY_PROVIDER_ID) throw new Error('model is unavailable')
  const next = { providerId, modelId }
  await shell(runtime, 'POST', '/api/providers/defaults', { defaultModel: next, quickModel: next, translateModel: next })
}

export async function answerStudioLocalProviderRequest(request: DataRequest): Promise<DataResponse | null> {
  const runtime = runtimeOrNull()
  if (!runtime || !planePath(String(request?.path || ''))) return null
  try {
    return ok(request, await dispatch(runtime, request))
  } catch (error) {
    return fail(request, error)
  }
}
