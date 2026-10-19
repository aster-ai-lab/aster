import type { ProviderModelOverride } from '../schemas/provider-models'
import { defineProvider } from './types'
import { modeWire } from './wires'

const claudeWebToolModels = [
  'claude-opus-4',
  'claude-sonnet-4',
  'claude-haiku-4',
  'claude-3-5-haiku',
  'claude-3-5-sonnet',
  'claude-3-7-sonnet'
]
const geminiWebToolModels = [
  'gemini-2',
  'gemini-3',
  'gemini-flash-latest',
  'gemini-pro-latest',
  'gemini-flash-lite-latest'
]
const openAIWebSearchModels = ['gpt-4o', 'gpt-4-1', 'gpt-5', 'o3', 'o4']

const deepSeekThinkingWire = modeWire('extra_body.thinking.type', {
  off: 'disabled',
  auto: 'enabled',
  effort: 'enabled'
})

const deepSeekModelOverrides = [
  {
    apiModelId: 'deepseek/deepseek-v3.2',
    modelId: 'deepseek-v3-2',
    reasoningContracts: {
      'openai-chat-completions': { wire: deepSeekThinkingWire }
    }
  }
] satisfies Array<Partial<ProviderModelOverride>>

const qwenAudioCompatibilityOverrides = [
  {
    apiModelId: 'qwen/qwen3.5-9b',
    modelId: 'qwen3-5-9b',
    capabilities: { remove: ['audio-recognition', 'video-recognition'] },
    inputModalities: ['text', 'image'],
    reason: 'A3S rejects native audio and video for the Qwen3.5 9B endpoint'
  },
  {
    apiModelId: 'qwen/qwen3.5-122b-a10b',
    modelId: 'qwen3-5-122b-a10b',
    capabilities: { remove: ['audio-recognition'] },
    inputModalities: ['text', 'image', 'video'],
    reason: 'A3S rejects native audio; base Qwen3.5 supports text/image/video input'
  },
  {
    apiModelId: 'qwen/qwen3.5-27b',
    modelId: 'qwen3-5-27b',
    capabilities: { remove: ['audio-recognition'] },
    inputModalities: ['text', 'image', 'video'],
    reason: 'A3S rejects native audio; base Qwen3.5 supports text/image/video input'
  },
  {
    apiModelId: 'qwen/qwen3.5-35b-a3b',
    modelId: 'qwen3-5-35b-a3b',
    capabilities: { remove: ['audio-recognition'] },
    inputModalities: ['text', 'image', 'video'],
    reason: 'A3S rejects native audio; base Qwen3.5 supports text/image/video input'
  },
  {
    modelId: 'qwen3-5-35b-a3b-free',
    apiModelId: 'qwen/qwen3.5-35b-a3b(free)',
    modelVariants: ['35b', 'free'],
    name: 'Qwen3.5 35B A3B (Free)',
    capabilities: { remove: ['audio-recognition', 'video-recognition'] },
    inputModalities: ['text', 'image'],
    reason: 'A3S free endpoint accepts text and image_url parts only'
  },
  {
    apiModelId: 'qwen/qwen3.5-397b-a17b',
    modelId: 'qwen3-5-397b-a17b',
    capabilities: { remove: ['audio-recognition'] },
    inputModalities: ['text', 'image', 'video'],
    reason: 'A3S rejects native audio; base Qwen3.5 supports text/image/video input'
  },
  {
    modelId: 'qwen3-5-4b',
    apiModelId: 'qwen/qwen3.5-4b(free)',
    modelVariants: ['4b', 'free'],
    name: 'Qwen3.5 4B (Free)',
    capabilities: { remove: ['video-recognition'] },
    inputModalities: ['text', 'image'],
    reason: 'A3S free endpoint accepts text and image_url parts only'
  },
  {
    modelId: 'qwen3-5-9b-free',
    apiModelId: 'qwen/qwen3.5-9b(free)',
    modelVariants: ['9b', 'free'],
    name: 'Qwen3.5 9B (Free)',
    capabilities: { remove: ['audio-recognition', 'video-recognition'] },
    inputModalities: ['text', 'image'],
    reason: 'A3S free endpoint accepts text and image_url parts only'
  }
] satisfies Array<Partial<ProviderModelOverride>>

export default defineProvider({
  id: 'a3s',
  name: 'A3S',
  availableInEditions: ['global', 'cn'],
  defaultChatEndpoint: 'openai-chat-completions',
  endpointConfigs: {
    'anthropic-messages': {
      adapterFamily: 'a3s',
      baseUrl: 'http://127.0.0.1:8080'
    },
    'google-generate-content': {
      adapterFamily: 'a3s',
      baseUrl: 'http://127.0.0.1:8080'
    },
    'openai-responses': {
      adapterFamily: 'a3s',
      baseUrl: 'http://127.0.0.1:8080'
    },
    'openai-chat-completions': {
      adapterFamily: 'a3s',
      baseUrl: 'http://127.0.0.1:8080',
      reasoningFormat: { type: 'openai-chat' }
    }
  },
  // Gateway-mapped delivery: `resolveToolCapability` falls back to the vendor
  // segment of the model provider id (`a3s.gemini` → google's factory), so
  // only vendors owning a native tool factory are servable — a deepseek/glm/kimi
  // model would resolve no factory and inject nothing.
  serverTools: [
    {
      id: 'web-search',
      modelScope: 'model-dependent',
      modelIdPrefixes: [...claudeWebToolModels, ...geminiWebToolModels, ...openAIWebSearchModels],
      imageModelIds: ['gemini-3-pro-image', 'gemini-3-pro-image-preview'],
      vendors: ['anthropic', 'gemini', 'openai']
    },
    {
      id: 'url-context',
      modelScope: 'model-dependent',
      modelIdPrefixes: [...claudeWebToolModels, ...geminiWebToolModels],
      vendors: ['anthropic', 'gemini']
    }
  ],
  metadata: {
    website: {
      apiKey: 'http://127.0.0.1:8080/console/token',
      docs: 'http://127.0.0.1:8080',
      models: 'http://127.0.0.1:8080/pricing',
      official: 'http://127.0.0.1:8080'
    }
  },
  overrides: [...deepSeekModelOverrides, ...qwenAudioCompatibilityOverrides]
})
