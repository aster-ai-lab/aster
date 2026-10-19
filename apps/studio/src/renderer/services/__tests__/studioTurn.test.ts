import { mockPreferenceService, mockPreferenceState } from '@test-mocks/renderer/PreferenceService'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { openStudioStream } from '@renderer/services/studioTurn'

const request = { topicId: 'agent-session:s1', trigger: 'submit-message' as const, userMessageParts: [] }

beforeEach(() => {
  mockPreferenceService._resetMockState()
  globalThis.localStorage.setItem('a3s.org.center', 'personal')
  vi.mocked(globalThis.window.api.ipcApi.request)
    .mockReset()
    .mockResolvedValue({ ok: true, data: { mode: 'started' } })
})

describe('Studio turn admission input', () => {
  it('captures the team before asynchronous recipe loading and does not borrow the next team', async () => {
    globalThis.localStorage.setItem('a3s.org.center', 'team-a')
    let complete: ((id: string) => void) | undefined
    mockPreferenceService.get.mockImplementationOnce(() => new Promise((resolve) => (complete = resolve)))
    const pending = openStudioStream(request)
    globalThis.localStorage.setItem('a3s.org.center', 'team-b')
    complete?.('lean')
    await pending
    expect(globalThis.window.api.ipcApi.request).toHaveBeenCalledWith('ai.stream.open', {
      ...request,
      orgId: 'team-a',
      harnessComponents: ['system', 'tools', 'budget', 'infer']
    })
  })

  it('keeps an explicitly captured team and recipe during a retry', async () => {
    globalThis.localStorage.setItem('a3s.org.center', 'team-b')
    const components = ['system', 'tools', 'budget', 'infer']
    const pending = openStudioStream({ ...request, orgId: 'team-a', harnessComponents: components })
    components.push('late-component')
    await pending
    expect(globalThis.window.api.ipcApi.request).toHaveBeenCalledWith('ai.stream.open', {
      ...request,
      orgId: 'team-a',
      harnessComponents: ['system', 'tools', 'budget', 'infer']
    })
  })

  it('does not start a run using a fallback from an unavailable preference store', async () => {
    mockPreferenceState.delete('app.harness.assemble')
    mockPreferenceService.get.mockResolvedValueOnce('default')
    await expect(openStudioStream(request)).rejects.toThrow('Harness preference is unavailable')
    expect(globalThis.window.api.ipcApi.request).not.toHaveBeenCalled()
  })
})
