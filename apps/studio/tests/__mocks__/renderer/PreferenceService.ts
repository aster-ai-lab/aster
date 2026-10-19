import { vi } from 'vitest'

/**
 * Mock PreferenceService for testing
 * Provides common preference defaults used across the application
 */

// Default preference values used in tests
export const mockPreferenceDefaults: Record<string, any> = {
  // Export preferences
  'data.export.markdown.force_dollar_math': false,
  'data.export.markdown.exclude_citations': false,
  'data.export.markdown.standardize_citations': true,
  'data.export.markdown.show_model_name': false,
  'data.export.markdown.show_model_provider': false,

  // UI preferences
  'ui.language': 'zh-CN',
  'ui.theme': 'light',
  'ui.font_size': 14,

  // AI preferences
  'ai.default_model': 'gpt-4',
  'ai.temperature': 0.7,
  'ai.max_tokens': 2000,

  // Feature flags
  'feature.web_search': true,
  'feature.reasoning': false,
  'feature.tool_calling': true,

  // User preferences
  'user.name': 'MockUser',

  // App preferences
  'app.user.name': 'MockUser',
  'app.language': 'zh-CN',
  'app.custom_instructions': '',
  'app.code_review.mode': 'inline',
  'app.permissions.computer_use': false,
  'app.permissions.agent_control': false,
  'app.workspace.path': '',
  'app.harness.assemble': 'default',
  'app.appearance.theme_pack': 'default',
  'app.appearance.density': 'comfortable',
  'app.appearance.chat_width': 'standard',
  'app.appearance.contrast': 'default',

  // Mirrors the generated schema defaults — without them the hook returns
  // null and UI gated on the master switch renders its disabled state.
  'chat.context_settings.enabled': true,
  'chat.context_settings.max_messages': null,
  'chat.context_settings.truncate_threshold': 50000,
  'chat.context_settings.compress.enabled': true,
  'chat.context_settings.compress.model_id': null

  // Add more defaults as needed
}

const createMockPreferenceState = (customDefaults: Record<string, any> = {}) =>
  new Map<string, any>(Object.entries({ ...mockPreferenceDefaults, ...customDefaults }))

export const mockPreferenceState = createMockPreferenceState()

/**
 * Mock implementation of PreferenceService
 */
export const createMockPreferenceService = (
  customDefaults: Record<string, any> = {},
  state = createMockPreferenceState(customDefaults)
) => {
  const listeners = new Map<string, Set<() => void>>()
  const notify = (key: string) => listeners.get(key)?.forEach((listener) => listener())
  const resetState = () => {
    state.clear()
    Object.entries({ ...mockPreferenceDefaults, ...customDefaults }).forEach(([key, value]) => state.set(key, value))
  }

  return {
    get: vi.fn((key: string) => {
      const value = state.get(key)
      return Promise.resolve(value !== undefined ? value : null)
    }),

    getMultiple: vi.fn((keys: Record<string, string>) => {
      const result: Record<string, any> = {}
      Object.entries(keys).forEach(([alias, key]) => {
        const value = state.get(key)
        result[alias] = value !== undefined ? value : null
      })
      return Promise.resolve(result)
    }),

    set: vi.fn((key: string, value: any) => {
      state.set(key, value)
      notify(key)
      return Promise.resolve()
    }),

    setMultiple: vi.fn((values: Record<string, any>) => {
      Object.entries(values).forEach(([key, value]) => {
        state.set(key, value)
        notify(key)
      })
      return Promise.resolve()
    }),

    preload: vi.fn(() => Promise.resolve()),

    preloadAll: vi.fn(() => Promise.resolve()),

    getCachedValue: vi.fn((key: string) => {
      return state.get(key)
    }),

    subscribeChange: (key: string) => (listener: () => void) => {
      const keyed = listeners.get(key) ?? new Set<() => void>()
      keyed.add(listener)
      listeners.set(key, keyed)
      return () => {
        keyed.delete(listener)
        if (!keyed.size) listeners.delete(key)
      }
    },

    isCached: vi.fn((key: string) => {
      return state.has(key)
    }),

    delete: vi.fn((key: string) => {
      state.delete(key)
      return Promise.resolve()
    }),

    clear: vi.fn(() => {
      state.clear()
      return Promise.resolve()
    }),

    // Internal state access for testing
    _getMockState: () => Object.fromEntries(state),
    _resetMockState: resetState
  }
}

// Default mock instance
export const mockPreferenceService = createMockPreferenceService({}, mockPreferenceState)

// Export for easy mocking in individual tests
export const MockPreferenceService = {
  preferenceService: mockPreferenceService
}
