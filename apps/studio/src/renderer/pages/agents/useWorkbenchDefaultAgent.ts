import { useEffect, useRef, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import { loggerService } from '@logger'
import { useMemberProfile } from '@renderer/components/account/memberProfile'
import { useInvalidateCache } from '@renderer/data/hooks/useDataApi'
import { fetchOrgModels } from '@renderer/data/orgModels'
import { OS_GATEWAY_AUTO_MODEL_ID, OS_GATEWAY_PROVIDER_ID } from '@renderer/data/osGateway'
import { useDefaultModel, useModelMutations, useModels } from '@renderer/hooks/useModel'
import { useProviders } from '@renderer/hooks/useProvider'
import { createAgentAndRefresh } from '@renderer/services/createAgent'
import { toast } from '@renderer/services/toast'
import { studioProductName, studioUsesWorkbenchShell } from '@renderer/tenantBrand'
import { formatErrorMessageWithPrefix } from '@renderer/utils/error'
import { AGENT_RUNTIME_CAPABILITIES } from '@shared/ai/agentRuntimeCapabilities'
import { createUniqueModelId, type Model } from '@shared/data/types/model'
import type { Provider } from '@shared/data/types/provider'
import { isEmbeddingModel, isGenerateImageModel, isRerankModel } from '@shared/utils/model'

import { useOrgModels } from './useOrgModels'
import { readPendingPermissionMode } from './workbenchPermissionPostures'
import { readPendingReasoningEffort } from './workbenchReasoningEffort'

const logger = loggerService.withContext('useWorkbenchDefaultAgent')

export const WORKBENCH_DEFAULT_AGENT_TYPE = 'a3s-code' as const

/** The declared workbench tenants keep one auto-created agent. Other tenants keep their catalog. */
export function workbenchUsesSingleDefaultAgent(): boolean {
  return studioUsesWorkbenchShell()
}

/**
 * The agent new workbench tasks belong to. Prefer the earliest a3s-code agent
 * so a later catalog entry cannot become a second default.
 */
export function preferredWorkbenchAgentId(
  agents: readonly { id: string; type: string; createdAt?: string }[]
): string | null {
  const codeAgents = agents.filter((agent) => agent.type === WORKBENCH_DEFAULT_AGENT_TYPE)
  // Catalog id `a3s-code` is older than the member's work agent and must not take its tasks.
  const memberAgents = codeAgents.filter((agent) => agent.id !== 'a3s-code')
  const pool = memberAgents.length > 0 ? memberAgents : codeAgents.length > 0 ? codeAgents : agents
  const sorted = [...pool].sort((left, right) => (left.createdAt ?? '').localeCompare(right.createdAt ?? ''))
  return sorted[0]?.id ?? null
}

type ProvisionState = {
  runtimeUnavailable: boolean
  ensuredAgentId: string | null
  provisionInFlight: Promise<string | null> | null
}

const provisionByMember = new Map<string, ProvisionState>()
const listeners = new Set<() => void>()

function provisionBucket(memberId: string | null | undefined): string | null {
  if (memberId == null) return 'local'
  if (memberId === '') return null
  return memberId
}

function provisionState(bucket: string): ProvisionState {
  let state = provisionByMember.get(bucket)
  if (!state) {
    state = { runtimeUnavailable: false, ensuredAgentId: null, provisionInFlight: null }
    provisionByMember.set(bucket, state)
  }
  return state
}

function markRuntimeUnavailable(state: ProvisionState) {
  state.runtimeUnavailable = true
  for (const listener of listeners) listener()
}

type EnsureWorkbenchAgentInput = {
  name: string
  modelId: string
  memberId?: string | null
  refresh: () => Promise<unknown>
  failureMessage: string
}

/** One shared create, so a homepage send during first launch does not add a second agent. */
export function ensureWorkbenchDefaultAgent(input: EnsureWorkbenchAgentInput): Promise<string | null> {
  const bucket = provisionBucket(input.memberId)
  if (!bucket) {
    logger.warn('Cannot provision a workbench agent without an explicit member scope')
    return Promise.resolve(null)
  }
  const state = provisionState(bucket)
  if (state.runtimeUnavailable) return Promise.resolve(null)
  if (state.ensuredAgentId) return Promise.resolve(state.ensuredAgentId)
  if (state.provisionInFlight) return state.provisionInFlight
  const permissionMode =
    readPendingPermissionMode(undefined, input.memberId ?? null) ??
    AGENT_RUNTIME_CAPABILITIES[WORKBENCH_DEFAULT_AGENT_TYPE].createDefaults.permissionMode
  const reasoningEffort = readPendingReasoningEffort(undefined, input.memberId ?? null)
  const pending = createAgentAndRefresh(
    {
      type: WORKBENCH_DEFAULT_AGENT_TYPE,
      name: input.name,
      model: input.modelId,
      configuration: {
        permission_mode: permissionMode,
        ...(reasoningEffort ? { reasoning_effort: reasoningEffort } : {})
      }
    },
    input.refresh
  )
    .then((agent) => {
      if (typeof agent?.id !== 'string' || !agent.id) {
        logger.warn('Agent creation returned no agent; this shell has no agent runtime')
        markRuntimeUnavailable(state)
        return null
      }
      state.ensuredAgentId = agent.id
      return agent.id
    })
    .catch((error: unknown) => {
      logger.error('Failed to provision the workbench agent', error as Error)
      toast.error(formatErrorMessageWithPrefix(error, input.failureMessage))
      return null
    })
    .finally(() => {
      if (state.provisionInFlight === pending) state.provisionInFlight = null
    })
  state.provisionInFlight = pending
  return pending
}

/** True once agent creation came back without an agent, i.e. the shell has no agent runtime. */
export function useWorkbenchAgentRuntimeUnavailable(): boolean {
  const memberId = useMemberProfile()?.id ?? null
  const bucket = provisionBucket(memberId)
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => (bucket ? provisionState(bucket).runtimeUnavailable : false)
  )
}

export function resetWorkbenchDefaultAgentForTests() {
  provisionByMember.clear()
}

function isChatModel(model: Model): boolean {
  return model.isEnabled && !isEmbeddingModel(model) && !isRerankModel(model) && !isGenerateImageModel(model)
}

/**
 * The chat model the member catalog named. Match the catalog alias.
 * A local copy lives on the signed-in gateway provider. Auto never picks
 * the first row of the client model list.
 */
export function modelForCatalogAlias(models: readonly Model[], alias: string | undefined): Model | undefined {
  if (!alias) return undefined
  let gatewayId: string
  try {
    gatewayId = createUniqueModelId(OS_GATEWAY_PROVIDER_ID, alias)
  } catch {
    return undefined
  }
  return models.find((model) => isChatModel(model) && (model.apiModelId === alias || model.id === gatewayId))
}

/** A sendable gateway model for a catalog alias, before Studio has stored the row. */
export function publishedAutoModel(alias: string, name?: string): Model | undefined {
  let id: ReturnType<typeof createUniqueModelId>
  try {
    id = createUniqueModelId(OS_GATEWAY_PROVIDER_ID, alias)
  } catch {
    return undefined
  }
  const label = name?.trim() || alias
  return {
    id,
    providerId: OS_GATEWAY_PROVIDER_ID,
    apiModelId: alias,
    name: label,
    capabilities: [],
    supportsStreaming: true,
    isEnabled: true,
    isHidden: false
  }
}

/**
 * Ask the member catalog whether Auto can run, and add the gateway `auto`
 * alias locally when Studio has not stored it yet. The preference stays empty.
 * The gateway, not this client, chooses the upstream model.
 */
export async function resolvePublishedAutoModel(
  models: readonly Model[],
  createModel: (dto: { providerId: string; modelId: string; name: string }) => Promise<Model>
): Promise<Model | null> {
  const catalog = await fetchOrgModels()
  if (!catalog.autoModel) return null
  return (
    modelForCatalogAlias(models, OS_GATEWAY_AUTO_MODEL_ID) ??
    createModel({
      providerId: OS_GATEWAY_PROVIDER_ID,
      modelId: OS_GATEWAY_AUTO_MODEL_ID,
      name: OS_GATEWAY_AUTO_MODEL_ID
    })
  )
}

/** The first enabled chat model of an enabled provider, in list order. */
export function firstUsableChatModel(
  models: readonly Model[],
  providers: readonly Pick<Provider, 'id' | 'isEnabled'>[]
): Model | undefined {
  const enabledProviders = new Set(providers.filter((provider) => provider.isEnabled).map((provider) => provider.id))
  return models.find(
    (model) =>
      model.isEnabled &&
      enabledProviders.has(model.providerId) &&
      !isEmbeddingModel(model) &&
      !isRerankModel(model) &&
      !isGenerateImageModel(model)
  )
}

/**
 * The model the workbench sends with. A stored id is an explicit choice.
 * An empty preference is Auto: the client sends the gateway alias `auto`
 * when the catalog names a model, and a missing catalog falls back to the
 * first usable chat model. Neither result is written back, so Auto stays Auto.
 */
export function useWorkbenchModel(options: { enabled?: boolean } = {}) {
  const enabled = options.enabled ?? true
  const { defaultModel } = useDefaultModel({ enabled })
  const [defaultModelId] = usePreference('chat.default_model_id')
  const auto = !defaultModelId
  const member = useMemberProfile()
  const localBuild = import.meta.env.PUBLIC_STUDIO_TENANT === 'aster'
  const organizationEnabled = !localBuild || member !== null
  const org = useOrgModels({ enabled: enabled && auto && organizationEnabled })
  const { providers } = useProviders(undefined, { enabled: enabled && auto })
  const { models } = useModels(undefined, { fetchEnabled: enabled && auto })
  const { createModel } = useModelMutations()
  const catalogSettled = localBuild || org.phase === 'ready' || org.phase === 'error'
  const alias = organizationEnabled && org.phase === 'ready' && org.catalog.autoModel ? OS_GATEWAY_AUTO_MODEL_ID : undefined
  const stored = enabled && auto && alias && Array.isArray(models) ? modelForCatalogAlias(models, alias) : undefined
  const published = enabled && auto && alias ? publishedAutoModel(alias) : undefined
  const fallback =
    enabled && auto && catalogSettled && !alias && Array.isArray(models) && Array.isArray(providers)
      ? firstUsableChatModel(models, organizationEnabled ? providers : providers.filter((provider) => provider.id !== OS_GATEWAY_PROVIDER_ID))
      : undefined
  const model = !auto ? (!organizationEnabled && defaultModel?.providerId === OS_GATEWAY_PROVIDER_ID ? undefined : defaultModel) : (stored ?? published ?? fallback)

  const creating = useRef(false)
  useEffect(() => {
    if (!enabled || !auto || org.phase !== 'ready' || !alias || !Array.isArray(models)) return
    if (modelForCatalogAlias(models, alias) || creating.current) return
    creating.current = true
    void createModel({
      providerId: OS_GATEWAY_PROVIDER_ID,
      modelId: OS_GATEWAY_AUTO_MODEL_ID,
      name: OS_GATEWAY_AUTO_MODEL_ID
    }).catch((error) => {
      creating.current = false
      logger.warn('Failed to add the catalog auto model', { error })
    })
  }, [alias, auto, createModel, enabled, models, org.phase])

  return { model, auto }
}

type Options = {
  /** True only once the agent list has loaded empty and no session is open. */
  enabled: boolean
  onCreated: (agentId: string) => void | Promise<void>
}

/**
 * The workbench has no agent picker, so a first launch with no agents provisions one
 * a3s-code agent on the default model instead of asking the user to choose.
 * Failures are not retried until the page reloads, so a broken backend cannot toast in a loop.
 */
export function useWorkbenchDefaultAgent({ enabled, onCreated }: Options) {
  const { t } = useTranslation()
  const memberId = useMemberProfile()?.id ?? null
  const { model: defaultModel } = useWorkbenchModel({ enabled })
  const invalidate = useInvalidateCache()
  const attempted = useRef<string | null | undefined>(undefined)
  const currentMember = useRef(memberId)
  currentMember.current = memberId
  const onCreatedRef = useRef(onCreated)
  onCreatedRef.current = onCreated

  useEffect(() => {
    const bucket = provisionBucket(memberId)
    if (!enabled || !defaultModel || !bucket || attempted.current === bucket) return
    if (provisionState(bucket).runtimeUnavailable) return
    attempted.current = bucket
    void ensureWorkbenchDefaultAgent({
      name: t('workbench.default_agent.name', { product: studioProductName() }),
      modelId: defaultModel.id,
      memberId,
      refresh: () => invalidate('/agents'),
      failureMessage: t('workbench.default_agent.create_failed')
    }).then(async (agentId) => {
      if (!agentId || currentMember.current !== memberId) return
      await onCreatedRef.current(agentId)
    })
  }, [defaultModel, enabled, invalidate, memberId, t])

  return { needsDefaultModel: enabled && !defaultModel }
}
