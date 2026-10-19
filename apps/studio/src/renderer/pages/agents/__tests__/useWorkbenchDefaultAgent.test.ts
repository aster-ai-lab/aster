import { renderHook, waitFor } from '@testing-library/react'
import { act } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { OS_GATEWAY_AUTO_MODEL_ID, OS_GATEWAY_PROVIDER_ID } from '@renderer/data/osGateway'
import { toast } from '@renderer/services/toast'
import type { Model } from '@shared/data/types/model'

import {
  ensureWorkbenchDefaultAgent,
  firstUsableChatModel,
  modelForCatalogAlias,
  preferredWorkbenchAgentId,
  resetWorkbenchDefaultAgentForTests,
  resolvePublishedAutoModel,
  useWorkbenchAgentRuntimeUnavailable,
  useWorkbenchDefaultAgent
} from '../useWorkbenchDefaultAgent'
import { writePendingPermissionMode } from '../workbenchPermissionPostures'
import { writePendingReasoningEffort } from '../workbenchReasoningEffort'

const mocks = vi.hoisted(() => ({
  defaultModel: { id: 'openai::gpt-5' } as { id: string } | undefined,
  defaultModelId: 'openai::gpt-5' as string | null,
  models: [] as Model[],
  providers: [] as { id: string; isEnabled: boolean }[],
  setDefaultModel: vi.fn(),
  createModel: vi.fn(),
  createAgentAndRefresh: vi.fn(),
  fetchOrgModels: vi.fn(),
  orgModels: {
    catalog: { items: [] as { id: string }[], autoModel: undefined as string | undefined },
    phase: 'error' as 'idle' | 'loading' | 'ready' | 'error'
  }
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))
vi.mock('@renderer/hooks/useModel', () => ({
  useDefaultModel: () => ({ defaultModel: mocks.defaultModel, setDefaultModel: mocks.setDefaultModel }),
  useModels: () => ({ models: mocks.models }),
  useModelMutations: () => ({ createModel: mocks.createModel })
}))
vi.mock('../useOrgModels', () => ({
  useOrgModels: () => mocks.orgModels
}))
vi.mock('@renderer/data/orgModels', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@renderer/data/orgModels')>()
  return { ...actual, fetchOrgModels: (...args: unknown[]) => mocks.fetchOrgModels(...args) }
})
vi.mock('@renderer/hooks/useProvider', () => ({
  useProviders: () => ({ providers: mocks.providers })
}))
vi.mock('@data/hooks/usePreference', () => ({
  usePreference: () => [mocks.defaultModelId, vi.fn()]
}))

function model(id: string, extra: Partial<Model> = {}): Model {
  const [providerId] = id.split('::')
  return { id, providerId, isEnabled: true, capabilities: [], ...extra } as unknown as Model
}
vi.mock('@renderer/data/hooks/useDataApi', () => ({
  useInvalidateCache: () => vi.fn()
}))
vi.mock('@renderer/services/createAgent', () => ({
  createAgentAndRefresh: mocks.createAgentAndRefresh
}))
vi.mock('@renderer/services/toast', () => ({
  toast: { error: vi.fn() }
}))
vi.mock('@renderer/tenantBrand', () => ({
  studioProductName: () => 'ToWork'
}))

describe('useWorkbenchDefaultAgent', () => {
  beforeEach(() => {
    mocks.defaultModel = { id: 'openai::gpt-5' }
    mocks.defaultModelId = 'openai::gpt-5'
    mocks.models = []
    mocks.providers = []
    mocks.setDefaultModel.mockReset().mockResolvedValue(undefined)
    mocks.createModel.mockReset().mockResolvedValue(undefined)
    mocks.fetchOrgModels.mockReset()
    mocks.orgModels = { catalog: { items: [], autoModel: undefined }, phase: 'error' }
    mocks.createAgentAndRefresh.mockReset().mockResolvedValue({ id: 'agent-1' })
    writePendingPermissionMode(null)
    writePendingReasoningEffort(null)
    resetWorkbenchDefaultAgentForTests()
  })

  it('creates one a3s-code agent on the default model and opens it', async () => {
    const onCreated = vi.fn()
    const { rerender } = renderHook(() => useWorkbenchDefaultAgent({ enabled: true, onCreated }))
    rerender()

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith('agent-1'))
    expect(mocks.createAgentAndRefresh).toHaveBeenCalledTimes(1)
    expect(mocks.createAgentAndRefresh.mock.calls[0][0]).toMatchObject({
      type: 'a3s-code',
      model: 'openai::gpt-5',
      configuration: { permission_mode: 'default' }
    })
  })

  it('carries the permission mode picked before the agent existed', async () => {
    writePendingPermissionMode('bypassPermissions')
    const onCreated = vi.fn()
    renderHook(() => useWorkbenchDefaultAgent({ enabled: true, onCreated }))

    await waitFor(() => expect(onCreated).toHaveBeenCalled())
    expect(mocks.createAgentAndRefresh.mock.calls[0][0].configuration).toEqual({ permission_mode: 'bypassPermissions' })
    expect(window.localStorage.getItem('workbench.composer.permission-mode:v1:local')).toBe('bypassPermissions')
  })

  it('carries the reasoning effort picked before the agent existed', async () => {
    writePendingReasoningEffort('high')
    const onCreated = vi.fn()
    renderHook(() => useWorkbenchDefaultAgent({ enabled: true, onCreated }))

    await waitFor(() => expect(onCreated).toHaveBeenCalled())
    expect(mocks.createAgentAndRefresh.mock.calls[0][0].configuration).toEqual({
      permission_mode: 'default',
      reasoning_effort: 'high'
    })
    expect(window.localStorage.getItem('workbench.composer.reasoning-effort:v1:local')).toBe('high')
  })

  it('waits for a default model instead of creating', () => {
    mocks.defaultModel = undefined
    mocks.defaultModelId = null
    const { result } = renderHook(() => useWorkbenchDefaultAgent({ enabled: true, onCreated: vi.fn() }))

    expect(result.current.needsDefaultModel).toBe(true)
    expect(mocks.createAgentAndRefresh).not.toHaveBeenCalled()
    expect(mocks.setDefaultModel).not.toHaveBeenCalled()
  })

  it('uses the catalog auto model without storing it', async () => {
    mocks.defaultModel = undefined
    mocks.defaultModelId = null
    mocks.orgModels = { catalog: { items: [], autoModel: 'deepseek-v4-flash' }, phase: 'ready' }
    mocks.providers = [{ id: 'zhipu', isEnabled: true }]
    const gatewayModelId = `${OS_GATEWAY_PROVIDER_ID}::${OS_GATEWAY_AUTO_MODEL_ID}`
    mocks.models = [model('zhipu::glm-4.6'), model(gatewayModelId, { apiModelId: OS_GATEWAY_AUTO_MODEL_ID })]
    const onCreated = vi.fn()
    renderHook(() => useWorkbenchDefaultAgent({ enabled: true, onCreated }))

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith('agent-1'))
    expect(mocks.createAgentAndRefresh.mock.calls[0][0]).toMatchObject({
      model: gatewayModelId
    })
    expect(mocks.setDefaultModel).not.toHaveBeenCalled()
    expect(mocks.createModel).not.toHaveBeenCalled()
  })

  it('sends with the catalog alias before Studio has stored the row', async () => {
    mocks.defaultModel = undefined
    mocks.defaultModelId = null
    mocks.orgModels = { catalog: { items: [], autoModel: 'deepseek-v4-flash' }, phase: 'ready' }
    mocks.providers = [{ id: 'zhipu', isEnabled: true }]
    mocks.models = [model('zhipu::glm-4.6')]
    const onCreated = vi.fn()
    renderHook(() => useWorkbenchDefaultAgent({ enabled: true, onCreated }))

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith('agent-1'))
    expect(mocks.createAgentAndRefresh.mock.calls[0][0]).toMatchObject({
      model: `${OS_GATEWAY_PROVIDER_ID}::${OS_GATEWAY_AUTO_MODEL_ID}`
    })
    expect(mocks.createModel).toHaveBeenCalledWith({
      providerId: OS_GATEWAY_PROVIDER_ID,
      modelId: OS_GATEWAY_AUTO_MODEL_ID,
      name: OS_GATEWAY_AUTO_MODEL_ID
    })
  })

  it('falls back to the first usable model when the catalog names none', async () => {
    mocks.defaultModel = undefined
    mocks.defaultModelId = null
    mocks.providers = [{ id: 'zhipu', isEnabled: true }]
    mocks.models = [
      model('zhipu::embedding-3', { capabilities: ['embedding'] as Model['capabilities'] }),
      model('zhipu::glm-4.6')
    ]
    const onCreated = vi.fn()
    renderHook(() => useWorkbenchDefaultAgent({ enabled: true, onCreated }))

    await waitFor(() => expect(onCreated).toHaveBeenCalledWith('agent-1'))
    expect(mocks.createAgentAndRefresh.mock.calls[0][0]).toMatchObject({ model: 'zhipu::glm-4.6' })
    expect(mocks.setDefaultModel).not.toHaveBeenCalled()
  })

  it('waits for a stored default that is still resolving', () => {
    mocks.defaultModel = undefined
    mocks.providers = [{ id: 'zhipu', isEnabled: true }]
    mocks.models = [model('zhipu::glm-4.6')]
    const { result } = renderHook(() => useWorkbenchDefaultAgent({ enabled: true, onCreated: vi.fn() }))

    expect(result.current.needsDefaultModel).toBe(true)
    expect(mocks.createAgentAndRefresh).not.toHaveBeenCalled()
  })

  it('does nothing while agents exist', () => {
    renderHook(() => useWorkbenchDefaultAgent({ enabled: false, onCreated: vi.fn() }))
    expect(mocks.createAgentAndRefresh).not.toHaveBeenCalled()
  })

  it('keeps each member on a separate agent and refuses an empty member id', async () => {
    const refresh = vi.fn(async () => undefined)
    const base = {
      name: 'ToWork 工作助手',
      modelId: 'openai::gpt-5',
      refresh,
      failureMessage: 'workbench.default_agent.create_failed'
    }
    mocks.createAgentAndRefresh.mockResolvedValueOnce({ id: 'agent-local' }).mockResolvedValueOnce({ id: 'agent-member' })

    await expect(ensureWorkbenchDefaultAgent(base)).resolves.toBe('agent-local')
    await expect(ensureWorkbenchDefaultAgent({ ...base, memberId: 'member-a' })).resolves.toBe('agent-member')
    await expect(ensureWorkbenchDefaultAgent({ ...base, memberId: null })).resolves.toBe('agent-local')
    await expect(ensureWorkbenchDefaultAgent({ ...base, memberId: '' })).resolves.toBeNull()
    expect(mocks.createAgentAndRefresh).toHaveBeenCalledTimes(2)
  })

  it('shares one creation when a homepage send overlaps the first launch', async () => {
    let resolveCreate: (agent: { id: string }) => void = () => undefined
    mocks.createAgentAndRefresh.mockReturnValue(
      new Promise((resolve) => {
        resolveCreate = resolve
      })
    )
    const input = {
      name: 'ToWork 工作助手',
      modelId: 'openai::gpt-5',
      refresh: vi.fn(async () => undefined),
      failureMessage: 'workbench.default_agent.create_failed'
    }
    const first = ensureWorkbenchDefaultAgent(input)
    const second = ensureWorkbenchDefaultAgent(input)
    resolveCreate({ id: 'agent-shared' })

    await expect(first).resolves.toBe('agent-shared')
    await expect(second).resolves.toBe('agent-shared')
    expect(mocks.createAgentAndRefresh).toHaveBeenCalledTimes(1)
    await expect(ensureWorkbenchDefaultAgent(input)).resolves.toBe('agent-shared')
    expect(mocks.createAgentAndRefresh).toHaveBeenCalledTimes(1)
  })

  it('toasts the creation failure once and allows a later try', async () => {
    mocks.createAgentAndRefresh.mockRejectedValueOnce(new Error('down'))
    const input = {
      name: 'ToWork 工作助手',
      modelId: 'openai::gpt-5',
      refresh: vi.fn(async () => undefined),
      failureMessage: 'workbench.default_agent.create_failed'
    }

    await expect(ensureWorkbenchDefaultAgent(input)).resolves.toBeNull()
    expect(toast.error).toHaveBeenCalledTimes(1)
    mocks.createAgentAndRefresh.mockResolvedValueOnce({ id: 'agent-2' })
    await expect(ensureWorkbenchDefaultAgent(input)).resolves.toBe('agent-2')
    expect(mocks.createAgentAndRefresh).toHaveBeenCalledTimes(2)
  })

  it('reports a missing agent runtime when creation returns no agent', async () => {
    mocks.createAgentAndRefresh.mockResolvedValue({ homePath: '' })
    const onCreated = vi.fn()
    const { result } = renderHook(() => {
      useWorkbenchDefaultAgent({ enabled: true, onCreated })
      return useWorkbenchAgentRuntimeUnavailable()
    })

    await waitFor(() => expect(result.current).toBe(true))
    await act(async () => {})
    expect(onCreated).not.toHaveBeenCalled()
  })
})

describe('preferredWorkbenchAgentId', () => {
  it('uses the earliest a3s-code agent when several agents already exist', () => {
    expect(
      preferredWorkbenchAgentId([
        { id: 'later', type: 'a3s-code', createdAt: '2026-02-01T00:00:00.000Z' },
        { id: 'pi', type: 'pi', createdAt: '2026-01-01T00:00:00.000Z' },
        { id: 'early', type: 'a3s-code', createdAt: '2026-01-02T00:00:00.000Z' }
      ])
    ).toBe('early')
  })

  it('keeps the member work agent when the catalog builtin is older', () => {
    expect(
      preferredWorkbenchAgentId([
        { id: 'a3s-code', type: 'a3s-code', createdAt: '2026-01-01T00:00:00.000Z' },
        { id: 'work', type: 'a3s-code', createdAt: '2026-01-01T00:00:01.000Z' }
      ])
    ).toBe('work')
    expect(
      preferredWorkbenchAgentId([{ id: 'a3s-code', type: 'a3s-code', createdAt: '2026-01-01T00:00:00.000Z' }])
    ).toBe('a3s-code')
  })

  it('falls back to the earliest agent when none is a3s-code', () => {
    expect(
      preferredWorkbenchAgentId([
        { id: 'second', type: 'pi', createdAt: '2026-03-01T00:00:00.000Z' },
        { id: 'first', type: 'claude-code', createdAt: '2026-02-01T00:00:00.000Z' }
      ])
    ).toBe('first')
  })
})

describe('firstUsableChatModel', () => {
  it('skips disabled providers, disabled models, and non-chat models', () => {
    const providers = [
      { id: 'off', isEnabled: false },
      { id: 'on', isEnabled: true }
    ]
    const models = [
      model('off::chat'),
      model('on::paused', { isEnabled: false }),
      model('on::rerank', { capabilities: ['rerank'] as Model['capabilities'] }),
      model('on::chat')
    ]

    expect(firstUsableChatModel(models, providers)?.id).toBe('on::chat')
    expect(firstUsableChatModel(models, [{ id: 'on', isEnabled: false }])).toBeUndefined()
  })
})

describe('modelForCatalogAlias', () => {
  it('matches the gateway model the catalog named', () => {
    const gatewayModelId = `${OS_GATEWAY_PROVIDER_ID}::deepseek-v4-flash`
    const models = [model('zhipu::glm-4.6'), model(gatewayModelId, { apiModelId: 'deepseek-v4-flash' })]
    expect(modelForCatalogAlias(models, 'deepseek-v4-flash')?.id).toBe(gatewayModelId)
    expect(modelForCatalogAlias(models, 'missing')).toBeUndefined()
  })
})

describe('resolvePublishedAutoModel', () => {
  it('creates the catalog model and leaves the preference untouched', async () => {
    mocks.fetchOrgModels.mockResolvedValue({ items: [], autoModel: 'deepseek-v4-flash' })
    const created = model(`${OS_GATEWAY_PROVIDER_ID}::${OS_GATEWAY_AUTO_MODEL_ID}`, {
      apiModelId: OS_GATEWAY_AUTO_MODEL_ID
    })
    const create = vi.fn().mockResolvedValue(created)

    await expect(resolvePublishedAutoModel([], create)).resolves.toBe(created)
    expect(create).toHaveBeenCalledWith({
      providerId: OS_GATEWAY_PROVIDER_ID,
      modelId: OS_GATEWAY_AUTO_MODEL_ID,
      name: OS_GATEWAY_AUTO_MODEL_ID
    })
    expect(mocks.setDefaultModel).not.toHaveBeenCalled()
  })

  it('returns null when the catalog has no auto model', async () => {
    mocks.fetchOrgModels.mockResolvedValue({ items: [] })
    const create = vi.fn()
    await expect(resolvePublishedAutoModel([], create)).resolves.toBeNull()
    expect(create).not.toHaveBeenCalled()
  })
})
