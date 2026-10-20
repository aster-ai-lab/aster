import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { OrgModelCatalog } from '@renderer/data/orgModels'
import { OS_GATEWAY_AUTO_MODEL_ID, OS_GATEWAY_PROVIDER_ID } from '@renderer/data/osGateway'

import { WorkbenchModelPicker } from '../WorkbenchModelPicker'

const mocks = vi.hoisted(() => ({
  fetchOrgModels: vi.fn(),
  models: [] as import('@shared/data/types/model').Model[],
  providers: [] as import('@shared/data/types/provider').Provider[],
  writeShellModelDefault: vi.fn(),
  createModel: vi.fn(),
  setDefaultModelId: vi.fn()
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

vi.mock('@renderer/data/orgModels', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@renderer/data/orgModels')>()
  return {
    ...actual,
    fetchOrgModels: (...args: unknown[]) => mocks.fetchOrgModels(...args)
  }
})

vi.mock('@renderer/hooks/useModel', () => ({
  useModels: () => ({ models: mocks.models, isLoading: false, refetch: vi.fn() }),
  useModelMutations: () => ({ createModel: mocks.createModel })
}))

vi.mock('@renderer/hooks/useProvider', () => ({ useProviders: () => ({ providers: mocks.providers }) }))
vi.mock('@renderer/data/studioLocalProviderPlane', () => ({ writeShellModelDefault: mocks.writeShellModelDefault }))

vi.mock('@data/hooks/usePreference', () => ({
  usePreference: () => [undefined, mocks.setDefaultModelId]
}))

vi.mock('@renderer/pages/settings/ProviderSettings/components/ProviderAvatar', () => ({
  ProviderAvatar: () => <span data-testid="provider-avatar" />
}))

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

describe('WorkbenchModelPicker popup', () => {
  beforeEach(() => {
    mocks.models = []
    mocks.providers = []
    mocks.writeShellModelDefault.mockReset().mockResolvedValue(undefined)
    mocks.fetchOrgModels.mockReset()
    mocks.createModel.mockReset()
    mocks.setDefaultModelId.mockReset()
    mocks.setDefaultModelId.mockResolvedValue(undefined)
    mocks.createModel.mockImplementation(async (dto: { providerId: string; modelId: string; name: string }) => ({
      id: `${dto.providerId}::${dto.modelId}`,
      providerId: dto.providerId,
      apiModelId: dto.modelId,
      name: dto.name,
      capabilities: []
    }))
    vi.stubGlobal('ResizeObserver', ResizeObserverStub)
    HTMLElement.prototype.hasPointerCapture ??= () => false
    HTMLElement.prototype.setPointerCapture ??= () => undefined
    HTMLElement.prototype.releasePointerCapture ??= () => undefined
    HTMLElement.prototype.scrollIntoView ??= () => undefined
  })

  it('selects an anonymous local model without waiting for or calling the organization', async () => {
    vi.stubEnv('PUBLIC_STUDIO_TENANT', 'aster')
    try {
      mocks.fetchOrgModels.mockImplementation(() => new Promise(() => undefined))
      mocks.models = [{ id: 'openai::local-chat', providerId: 'openai', apiModelId: 'local-chat', name: 'Private chat', isEnabled: true, capabilities: [] } as import('@shared/data/types/model').Model]
      mocks.providers = [{ id: 'openai', name: 'Private endpoint', isEnabled: true } as import('@shared/data/types/provider').Provider]
      const user = userEvent.setup()
      const onSelect = vi.fn()
      render(<WorkbenchModelPicker trigger={<button>Local models</button>} onSelect={onSelect} />)
      await user.click(screen.getByRole('button', { name: 'Local models' }))
      await user.click(await screen.findByRole('option', { name: /Private chat/ }))
      expect(mocks.fetchOrgModels).not.toHaveBeenCalled()
      expect(mocks.writeShellModelDefault).toHaveBeenCalledWith({ providerId: 'openai', modelId: 'local-chat' })
      expect(mocks.setDefaultModelId).toHaveBeenCalledWith('openai::local-chat')
      expect(onSelect).toHaveBeenCalledWith(mocks.models[0])
    } finally { vi.unstubAllEnvs() }
  })

  it('lists models from the member catalog and selects one without throwing', async () => {
    const user = userEvent.setup()
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    let publish: (catalog: OrgModelCatalog) => void = () => undefined
    mocks.fetchOrgModels.mockImplementation(
      () =>
        new Promise<OrgModelCatalog>((resolve) => {
          publish = resolve
        })
    )
    const onSelect = vi.fn()

    try {
      render(<WorkbenchModelPicker trigger={<button type="button">选择模型</button>} onSelect={onSelect} />)

      await user.click(screen.getByRole('button', { name: '选择模型' }))

      expect(await screen.findByRole('status')).toHaveTextContent('workbench.model_picker.loading')
      expect(consoleError).not.toHaveBeenCalled()

      publish({
        items: [
          {
            id: 'deepseek-v4-flash',
            name: 'DeepSeek V4 Flash',
            brand: 'DeepSeek',
            capabilities: ['tool_call', 'reasoning'],
            contextWindow: 128000,
            outputLimit: null,
            inputPrice: 0.14,
            cacheReadPrice: null,
            cacheWritePrice: null,
            outputPrice: 0.28,
            priceMultiplier: null,
            reasoningEfforts: null,
            available: true
          },
          {
            id: 'bare-chat',
            name: 'Bare Chat',
            capabilities: undefined as unknown as string[],
            contextWindow: null,
            outputLimit: null,
            inputPrice: null,
            cacheReadPrice: null,
            cacheWritePrice: null,
            outputPrice: null,
            priceMultiplier: null,
            reasoningEfforts: null,
            available: true
          }
        ],
        defaultModel: 'deepseek-v4-flash'
      })

      expect(await screen.findByRole('option', { name: /DeepSeek V4 Flash/ })).toBeVisible()
      const bare = screen.getByRole('option', { name: /Bare Chat/ })
      expect(bare).toBeVisible()
      await user.hover(bare)
      expect(screen.getByRole('complementary', { name: 'Bare Chat' })).toBeVisible()
      expect(consoleError).not.toHaveBeenCalled()

      await user.click(screen.getByRole('option', { name: /DeepSeek V4 Flash/ }))

      await waitFor(() => {
        expect(onSelect).toHaveBeenCalledWith(
          expect.objectContaining({ apiModelId: 'deepseek-v4-flash', name: 'DeepSeek V4 Flash' })
        )
      })
      expect(mocks.createModel).toHaveBeenCalledWith(
        expect.objectContaining({
          providerId: OS_GATEWAY_PROVIDER_ID,
          modelId: 'deepseek-v4-flash',
          name: 'DeepSeek V4 Flash',
          capabilities: expect.arrayContaining(['function-call', 'reasoning'])
        })
      )
      expect(mocks.setDefaultModelId).toHaveBeenCalledWith(`${OS_GATEWAY_PROVIDER_ID}::deepseek-v4-flash`)
      const unexpected = consoleError.mock.calls.filter((call) => {
        const text = call.map((part) => String(part)).join(' ')
        return !text.includes('getThumbAriaLabel') && !text.includes('getThumbAriaValueText')
      })
      expect(unexpected).toEqual([])
    } finally {
      consoleError.mockRestore()
    }
  })

  it('selects Auto from the catalog model and clears the stored model', async () => {
    const user = userEvent.setup()
    mocks.fetchOrgModels.mockResolvedValue({
      items: [
        {
          id: 'deepseek-v4-flash',
          name: 'DeepSeek V4 Flash',
          brand: 'DeepSeek',
          capabilities: ['tool_call'],
          contextWindow: 128000,
          outputLimit: null,
          inputPrice: null,
          cacheReadPrice: null,
          cacheWritePrice: null,
          outputPrice: null,
          priceMultiplier: null,
          reasoningEfforts: null,
          available: true
        }
      ],
      autoModel: 'deepseek-v4-flash'
    })
    const onSelect = vi.fn()
    render(<WorkbenchModelPicker trigger={<button type="button">选择模型</button>} onSelect={onSelect} />)
    await user.click(screen.getByRole('button', { name: '选择模型' }))
    await user.click(await screen.findByRole('option', { name: /workbench.model_picker.auto/ }))

    await waitFor(() => {
      expect(onSelect).toHaveBeenCalledWith(
        expect.objectContaining({
          id: `${OS_GATEWAY_PROVIDER_ID}::${OS_GATEWAY_AUTO_MODEL_ID}`,
          apiModelId: OS_GATEWAY_AUTO_MODEL_ID
        })
      )
    })
    expect(mocks.setDefaultModelId).toHaveBeenCalledWith(null)
    expect(mocks.setDefaultModelId).not.toHaveBeenCalledWith(`${OS_GATEWAY_PROVIDER_ID}::deepseek-v4-flash`)
  })
})
