import { studioModelServiceName } from '@renderer/tenantBrand'
import type { DataRequest, DataResponse } from '@shared/data/api/types'
import {
  createUniqueModelId,
  MODALITY,
  MODEL_CAPABILITY,
  parseUniqueModelId,
  type Model,
  type UniqueModelId
} from '@shared/data/types/model'
import type { ApiKeyEntry, AuthConfig, Provider } from '@shared/data/types/provider'
import { readStudioRuntime, type StudioRuntimeDescriptor } from '@shared/studioRuntime'

import { fetchOrgModels, orgModelCapabilities, orgModelEndpointTypes, type OrgModel } from './orgModels'
import { OS_GATEWAY_AUTO_MODEL_ID, OS_GATEWAY_PROVIDER_ID, osGatewayOrigin, osGatewayProvider } from './osGateway'

/**
 * Signed-in Studio has one model provider: the a3s OS gateway. The member
 * session is read from the shell for each key lookup and kept in memory.
 * It is not written to the provider store.
 */

type GatewayCredential = { origin: string; token: string }

type PlaneState = {
  credential: GatewayCredential | null
  models: Map<string, Model[]>
  loadedModels: Set<string>
  tombstones: Set<string>
}

const MANAGED = 'the signed-in gateway does not accept provider configuration'

let state: PlaneState = emptyState()
let ipcInstalled = false

function emptyState(): PlaneState {
  return {
    credential: null,
    models: new Map(),
    loadedModels: new Set(),
    tombstones: new Set()
  }
}

export function resetStudioProviderPlaneForTests() {
  state = emptyState()
  ipcInstalled = false
}

function ok(request: DataRequest, data: unknown): DataResponse {
  return { id: request.id, status: 200, data, metadata: { timestamp: Date.now() } }
}

function fail(request: DataRequest, message: string): DataResponse {
  return {
    id: request.id,
    status: 400,
    error: { code: 'VALIDATION_ERROR', message, status: 400 },
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

async function readCredential(runtime: StudioRuntimeDescriptor): Promise<GatewayCredential> {
  if (state.credential) return state.credential
  const response = await fetch(`${runtime.apiBaseUrl}/api/os-gateway/credential`, {
    headers: { authorization: `Bearer ${runtime.token}` }
  })
  const body = record(await response.json().catch(() => null))
  const origin = typeof body.origin === 'string' ? body.origin.trim().replace(/\/+$/, '') : ''
  const token = typeof body.token === 'string' ? body.token : ''
  if (!response.ok || !origin.startsWith('http') || !token) {
    throw new Error('signed-in gateway is unavailable')
  }
  state.credential = { origin, token }
  return state.credential
}

async function gatewayProvider(runtime: StudioRuntimeDescriptor): Promise<Provider> {
  let origin = osGatewayOrigin()
  try {
    origin = (await readCredential(runtime)).origin
  } catch {
    // The picker can still list the gateway. Calls that need the session fail closed.
  }
  return osGatewayProvider(studioModelServiceName(), origin)
}

function assertGateway(id: string) {
  if (id !== OS_GATEWAY_PROVIDER_ID) throw new Error('provider not found')
}

/** Gateway alias `auto`. The catalog names the upstream model; the id stays `a3s::auto`. */
function gatewayAutoAlias(): Model {
  return {
    id: createUniqueModelId(OS_GATEWAY_PROVIDER_ID, OS_GATEWAY_AUTO_MODEL_ID),
    providerId: OS_GATEWAY_PROVIDER_ID,
    apiModelId: OS_GATEWAY_AUTO_MODEL_ID,
    name: OS_GATEWAY_AUTO_MODEL_ID,
    capabilities: [],
    supportsStreaming: true,
    isEnabled: true,
    isHidden: false
  }
}

function orgToModel(model: OrgModel): Model {
  const endpointTypes = orgModelEndpointTypes(model)
  const capabilities = orgModelCapabilities(model)
  return {
    id: createUniqueModelId(OS_GATEWAY_PROVIDER_ID, model.id),
    providerId: OS_GATEWAY_PROVIDER_ID,
    apiModelId: model.id,
    name: model.name,
    ...(model.description ? { description: model.description } : {}),
    ...(model.brand ? { group: model.brand } : {}),
    capabilities,
    ...(endpointTypes.length > 0 ? { endpointTypes } : {}),
    ...(model.contextWindow ? { contextWindow: model.contextWindow } : {}),
    ...(model.outputLimit ? { maxOutputTokens: model.outputLimit } : {}),
    inputModalities: capabilities.includes(MODEL_CAPABILITY.IMAGE_RECOGNITION)
      ? [MODALITY.TEXT, MODALITY.IMAGE]
      : [MODALITY.TEXT],
    outputModalities: capabilities.includes(MODEL_CAPABILITY.IMAGE_GENERATION) ? [MODALITY.IMAGE] : [MODALITY.TEXT],
    supportsStreaming: true,
    isEnabled: model.available,
    isHidden: false
  }
}

function rememberModels(providerId: string, models: Model[]) {
  state.loadedModels.add(providerId)
  state.models.set(
    providerId,
    models.filter((model) => !state.tombstones.has(model.id))
  )
}

async function ensureModels(runtime: StudioRuntimeDescriptor, providerId: string) {
  assertGateway(providerId)
  if (state.loadedModels.has(providerId)) return
  const catalog = await fetchOrgModels()
  void runtime
  rememberModels(providerId, catalog.items.map(orgToModel))
}

function modelFromCreate(dto: Record<string, unknown>): Model {
  const providerId = String(dto.providerId || '')
  assertGateway(providerId)
  const modelId = String(dto.modelId || '')
  if (!modelId) throw new Error('model id is required')
  return {
    id: createUniqueModelId(providerId, modelId),
    providerId,
    apiModelId: modelId,
    name: typeof dto.name === 'string' && dto.name.trim() ? dto.name.trim() : modelId,
    ...(typeof dto.group === 'string' ? { group: dto.group } : {}),
    ...(typeof dto.description === 'string' ? { description: dto.description } : {}),
    capabilities: Array.isArray(dto.capabilities) ? (dto.capabilities as Model['capabilities']) : [],
    ...(Array.isArray(dto.endpointTypes) ? { endpointTypes: dto.endpointTypes as Model['endpointTypes'] } : {}),
    supportsStreaming: dto.supportsStreaming !== false,
    isEnabled: true,
    isHidden: false
  }
}

function upsertMemoryModel(model: Model) {
  const current = state.models.get(model.providerId) ?? []
  rememberModels(model.providerId, [...current.filter((item) => item.id !== model.id), model])
}

async function sessionKey(runtime: StudioRuntimeDescriptor): Promise<ApiKeyEntry> {
  const credential = await readCredential(runtime)
  return { id: `${OS_GATEWAY_PROVIDER_ID}:session`, key: credential.token, isEnabled: true }
}

function asStringList(value: unknown): string[] {
  if (Array.isArray(value)) return value.map((item) => String(item)).filter(Boolean)
  if (typeof value === 'string' && value) return [value]
  return []
}

export function installStudioProviderIpc() {
  if (ipcInstalled || !runtimeOrNull()) return
  const bridge = globalThis.api?.ipcApi
  if (!bridge?.request) return
  ipcInstalled = true
  const original = bridge.request.bind(bridge)
  bridge.request = (route: string, input?: unknown) => {
    if (route === 'ovms.is_supported') return Promise.resolve({ ok: true, data: false })
    if (route === 'ai.provider.model.list') return listModelsIpc(input)
    if (route === 'ai.provider.model.check') {
      return Promise.resolve({
        ok: false,
        error: { code: 'INTERNAL', message: 'provider connection check is unavailable' }
      })
    }
    return original(route, input)
  }
}

async function listModelsIpc(input: unknown) {
  const runtime = runtimeOrNull()
  const providerId =
    input && typeof input === 'object' && 'providerId' in input
      ? String((input as { providerId?: unknown }).providerId || '')
      : ''
  if (!runtime || providerId !== OS_GATEWAY_PROVIDER_ID) {
    return { ok: false, error: { code: 'INTERNAL', message: 'provider model list is unavailable' } }
  }
  try {
    state.loadedModels.delete(providerId)
    await ensureModels(runtime, providerId)
    return { ok: true, data: { models: state.models.get(providerId) ?? [] } }
  } catch {
    return { ok: false, error: { code: 'INTERNAL', message: 'provider model sync failed' } }
  }
}

async function dispatch(runtime: StudioRuntimeDescriptor, request: DataRequest): Promise<unknown> {
  const method = request.method
  const parts = segments(request.path)
  const query = request.params ?? {}
  const body = request.body && typeof request.body === 'object' ? (request.body as Record<string, unknown>) : {}

  if (parts[0] === 'providers' && parts.length === 1) {
    if (method === 'GET') {
      const provider = await gatewayProvider(runtime)
      const enabled = typeof query.enabled === 'boolean' ? query.enabled : undefined
      if (enabled !== undefined && provider.isEnabled !== enabled) return []
      return [provider]
    }
    throw new Error(MANAGED)
  }

  if (parts[0] === 'providers' && (parts[1] === 'order:batch' || parts[2] === 'order') && method === 'PATCH') {
    return undefined
  }

  if (parts[0] === 'providers' && parts.length === 2) {
    const id = parts[1]
    assertGateway(id)
    if (method === 'GET') return gatewayProvider(runtime)
    throw new Error(MANAGED)
  }

  if (parts[0] === 'providers' && parts[2] === 'api-keys' && parts.length === 3) {
    const id = parts[1]
    assertGateway(id)
    if (method === 'GET') {
      const key = await sessionKey(runtime)
      return { keys: query.enabled === true && !key.isEnabled ? [] : [key] }
    }
    throw new Error(MANAGED)
  }

  if (parts[0] === 'providers' && parts[2] === 'api-keys' && parts.length === 4) {
    assertGateway(parts[1])
    throw new Error(MANAGED)
  }

  if (parts[0] === 'providers' && parts[2] === 'auth-config' && method === 'GET') {
    assertGateway(parts[1])
    const config: AuthConfig = { type: 'api-key', required: true }
    return config
  }

  if (parts[0] === 'providers' && parts[2] === 'preset' && method === 'GET') {
    assertGateway(parts[1])
    const provider = await gatewayProvider(runtime)
    const fields = asStringList(query.fields)
    const preset: { endpointConfigs?: Provider['endpointConfigs'] | null; models?: Model[] } = {}
    if (fields.includes('endpointConfigs')) preset.endpointConfigs = provider.endpointConfigs ?? null
    if (fields.includes('models')) preset.models = []
    return preset
  }

  if (parts[0] === 'providers' && parts[2] === 'models:resolve' && method === 'GET') {
    const providerId = parts[1]
    assertGateway(providerId)
    return asStringList(query.ids).map(
      (modelId) =>
        ({
          id: createUniqueModelId(providerId, modelId),
          providerId,
          apiModelId: modelId,
          name: modelId,
          capabilities: [],
          supportsStreaming: true,
          isEnabled: true,
          isHidden: false
        }) satisfies Model
    )
  }

  if (parts[0] === 'providers' && parts[2] === 'models:reconcile' && method === 'POST') {
    const providerId = parts[1]
    await ensureModels(runtime, providerId)
    const remove = new Set(asStringList(body.toRemove))
    for (const id of remove) state.tombstones.add(id as UniqueModelId)
    const created: Model[] = []
    for (const dto of Array.isArray(body.toAdd) ? body.toAdd : []) {
      if (!dto || typeof dto !== 'object') continue
      const model = modelFromCreate(dto as Record<string, unknown>)
      created.push(model)
    }
    const kept = (state.models.get(providerId) ?? []).filter((model) => !remove.has(model.id))
    rememberModels(providerId, [...kept, ...created])
    return state.models.get(providerId) ?? []
  }

  if (parts[0] === 'models' && parts.length === 1) {
    if (method === 'GET') {
      const providerId =
        typeof query.providerId === 'string' && query.providerId ? query.providerId : OS_GATEWAY_PROVIDER_ID
      await ensureModels(runtime, providerId)
      return state.models.get(providerId) ?? []
    }
    if (method === 'POST') {
      const dtos = Array.isArray(request.body) ? request.body : []
      const created: Model[] = []
      for (const dto of dtos) {
        if (!dto || typeof dto !== 'object') continue
        const model = modelFromCreate(dto as Record<string, unknown>)
        upsertMemoryModel(model)
        created.push(model)
      }
      return created
    }
    if (method === 'PATCH') {
      const items = Array.isArray(request.body) ? request.body : []
      const updated: Model[] = []
      for (const item of items) {
        if (!item || typeof item !== 'object') continue
        const recordBody = item as { uniqueModelId?: string; patch?: Record<string, unknown> }
        if (!recordBody.uniqueModelId) continue
        updated.push(patchModel(recordBody.uniqueModelId, recordBody.patch ?? {}))
      }
      return updated
    }
    if (method === 'DELETE') {
      for (const id of asStringList(query.ids)) dropModel(id)
      return undefined
    }
  }

  if (parts[0] === 'models' && parts.length > 1) {
    const uniqueModelId = parts.slice(1).join('/')
    if (method === 'GET') {
      const parsed = parseUniqueModelId(uniqueModelId as UniqueModelId)
      await ensureModels(runtime, parsed.providerId)
      const model = (state.models.get(parsed.providerId) ?? []).find((item) => item.id === uniqueModelId)
      if (model) return model
      // Auto is a catalog alias, not a stored row. A reload still has to resolve it.
      if (
        parsed.providerId === OS_GATEWAY_PROVIDER_ID &&
        parsed.modelId === OS_GATEWAY_AUTO_MODEL_ID &&
        (await fetchOrgModels()).autoModel
      ) {
        return gatewayAutoAlias()
      }
      throw new Error('model not found')
    }
    if (method === 'PATCH') return patchModel(uniqueModelId, body)
    if (method === 'DELETE') {
      dropModel(uniqueModelId)
      return undefined
    }
  }

  throw new Error('provider route not found')
}

function dropModel(uniqueModelId: string) {
  state.tombstones.add(uniqueModelId)
  try {
    const { providerId } = parseUniqueModelId(uniqueModelId as UniqueModelId)
    rememberModels(
      providerId,
      (state.models.get(providerId) ?? []).filter((model) => model.id !== uniqueModelId)
    )
  } catch {
    // A malformed id is already tombstoned.
  }
}

function patchModel(uniqueModelId: string, patch: Record<string, unknown>): Model {
  const parsed = parseUniqueModelId(uniqueModelId as UniqueModelId)
  if (parsed.providerId !== OS_GATEWAY_PROVIDER_ID) throw new Error('model not found')
  const models = state.models.get(parsed.providerId) ?? []
  const index = models.findIndex((model) => model.id === uniqueModelId)
  if (index < 0) throw new Error('model not found')
  const next: Model = {
    ...models[index],
    ...(typeof patch.name === 'string' ? { name: patch.name } : {}),
    ...(typeof patch.isEnabled === 'boolean' ? { isEnabled: patch.isEnabled } : {}),
    ...(typeof patch.isHidden === 'boolean' ? { isHidden: patch.isHidden } : {}),
    ...(typeof patch.notes === 'string' ? { notes: patch.notes } : {})
  }
  models[index] = next
  state.models.set(parsed.providerId, models)
  return next
}

export async function answerStudioProviderRequest(request: DataRequest): Promise<DataResponse | null> {
  const runtime = runtimeOrNull()
  if (!runtime || !planePath(String(request?.path || ''))) return null
  installStudioProviderIpc()
  try {
    return ok(request, await dispatch(runtime, request))
  } catch (error) {
    const message = error instanceof Error ? error.message : 'provider request failed'
    return fail(request, message)
  }
}
