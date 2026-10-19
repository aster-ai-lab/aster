import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { ChatConversationControls } from '@renderer/components/composer/variants/chat/ChatConversationControls'
import { useChatMentionedModels } from '@renderer/components/composer/variants/chat/useChatMentionedModels'
import { resolveReasoningEffortForModel } from '@shared/ai/reasoning'
import { type Model, MODEL_CAPABILITY, type UniqueModelId } from '@shared/data/types/model'
import type { Provider } from '@shared/data/types/provider'

const mocks = vi.hoisted(() => ({
  useModels: vi.fn(),
  useProviders: vi.fn(),
  usePins: vi.fn()
}))

vi.mock('@renderer/hooks/useModel', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@renderer/hooks/useModel')>()
  return {
    ...actual,
    useModels: (...args: unknown[]) => mocks.useModels(...args)
  }
})

vi.mock('@renderer/hooks/useProvider', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@renderer/hooks/useProvider')>()
  return {
    ...actual,
    useProviders: (...args: unknown[]) => mocks.useProviders(...args)
  }
})

vi.mock('@renderer/hooks/usePins', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@renderer/hooks/usePins')>()
  return {
    ...actual,
    usePins: (...args: unknown[]) => mocks.usePins(...args)
  }
})

vi.mock('@renderer/components/resourceCatalog/selectors', () => ({
  AssistantSelector: () => <button type="button">Assistant</button>
}))

const provider = {
  id: 'lab',
  name: 'Lab',
  apiKeys: [],
  authType: 'api-key',
  reportsActualCost: false,
  settings: {},
  isEnabled: true
} as Provider

function makeModel(modelId: UniqueModelId, name: string, capabilities: Model['capabilities'] = []): Model {
  return {
    id: modelId,
    providerId: provider.id,
    name,
    capabilities,
    supportsStreaming: true,
    isEnabled: true,
    isHidden: false
  } as Model
}

const currentModel = makeModel('lab::current', 'Current Chat')
const nextModel = makeModel('lab::next', 'Next Chat')
const embeddingModel = makeModel('lab::embed', 'Embed Only', [MODEL_CAPABILITY.EMBEDDING])
const bareModel = {
  id: 'lab::bare',
  providerId: provider.id,
  name: 'Bare Chat',
  supportsStreaming: true,
  isEnabled: true,
  isHidden: false
} as Model

const catalog = [currentModel, nextModel, embeddingModel, bareModel]

function HomepageModelControl() {
  const [mentionedModels, setMentionedModels] = useState<Model[]>([])
  const selector = useChatMentionedModels({
    enabled: true,
    runtimeModel: currentModel,
    runtimeModelPending: false,
    selectedAssistantId: 'assistant',
    topicId: 'topic',
    mentionedModels,
    setMentionedModels,
    onModelSelect: (model) => {
      if (model) resolveReasoningEffortForModel(model, undefined)
    }
  })

  return (
    <ChatConversationControls
      assistantId="assistant"
      assistantName="Assistant"
      model={currentModel}
      providers={[provider]}
      mentionedModels={mentionedModels}
      mentionedModelSelectorValue={selector.mentionedModelSelectorValue}
      lockedMentionedModels={[]}
      mentionedModelMultiSelectMode={selector.mentionedModelMultiSelectMode}
      selectModelLabel="选择模型"
      useMentionedModelSelector
      shouldAutoSelectCreatedAssistant={false}
      side="bottom"
      onAssistantChange={() => undefined}
      onModelSelect={() => undefined}
      onMentionedModelsSelect={selector.handleMentionedModelsSelect}
      onMentionedModelMultiSelectModeChange={selector.handleMentionedModelMultiSelectModeChange}
      onMentionedModelSelectorRestore={selector.handleMentionedModelSelectorRestore}
    />
  )
}

describe('homepage composer model popup', () => {
  beforeEach(() => {
    mocks.useModels.mockReset()
    mocks.useProviders.mockReset()
    mocks.usePins.mockReset()
    mocks.useModels.mockImplementation((_query: unknown, options?: { fetchEnabled?: boolean }) => ({
      models: options?.fetchEnabled === false ? [] : catalog,
      isLoading: false,
      refetch: vi.fn(async () => undefined)
    }))
    mocks.useProviders.mockReturnValue({
      providers: [provider],
      isLoading: false,
      refetch: vi.fn(async () => undefined)
    })
    mocks.usePins.mockReturnValue({
      isLoading: false,
      isRefreshing: false,
      isMutating: false,
      error: undefined,
      pinnedIds: [],
      refetch: vi.fn(async () => undefined),
      togglePin: vi.fn(async () => undefined)
    })
    HTMLElement.prototype.hasPointerCapture ??= () => false
    HTMLElement.prototype.setPointerCapture ??= () => undefined
    HTMLElement.prototype.releasePointerCapture ??= () => undefined
    HTMLElement.prototype.scrollIntoView ??= () => undefined
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, get: () => 480 })
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, get: () => 400 })
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 480 })
    Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 400 })
  })

  it('opens the model popup, selects a chat model, and keeps it on the input control', async () => {
    const user = userEvent.setup()
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined)

    try {
      render(<HomepageModelControl />)

      const trigger = await screen.findByRole('button', { name: /已选模型|Selected models/ })
      await waitFor(() => {
        expect(trigger).toHaveTextContent('Current Chat')
      })

      await user.click(trigger)

      expect(screen.getByTestId('model-selector-content')).toBeVisible()
      expect(screen.getByRole('option', { name: /Next Chat/ })).toBeVisible()
      expect(screen.getByRole('option', { name: /Bare Chat/ })).toBeVisible()
      expect(screen.queryByRole('option', { name: /Embed Only/ })).not.toBeInTheDocument()

      await user.click(screen.getByRole('option', { name: /Bare Chat/ }))

      await waitFor(() => {
        expect(screen.queryByTestId('model-selector-content')).not.toBeVisible()
      })
      expect(trigger).toHaveTextContent('Bare Chat')
      expect(consoleError).not.toHaveBeenCalled()
    } finally {
      consoleError.mockRestore()
    }
  })
})
