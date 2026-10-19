import { mockPreferenceService } from '@test-mocks/renderer/PreferenceService'
import { beforeEach, describe, expect, it } from 'vitest'

import { HARNESS_ASSEMBLE_KEY, withHarnessComponents, writeHarnessAssembleId } from '@renderer/services/harnessAssemble'

beforeEach(() => mockPreferenceService._resetMockState())

describe('captured Harness recipes', () => {
  it('freezes a new run independently of later preference and caller mutations', async () => {
    await writeHarnessAssembleId('lean')
    const captured = await withHarnessComponents({ userMessageParts: [] })
    await writeHarnessAssembleId('host')
    expect(captured.harnessComponents).toEqual(['system', 'tools', 'budget', 'infer'])
    captured.harnessComponents.push('caller-mutation')
    expect((await withHarnessComponents({})).harnessComponents).toEqual([
      'system',
      'tools',
      'host:intent_stamp',
      'budget',
      'compact',
      'infer'
    ])
  })

  it('preserves an already captured recipe during a retry', async () => {
    await writeHarnessAssembleId('host')
    const source = ['system', 'tools', 'budget', 'infer']
    const request = await withHarnessComponents({ harnessComponents: source })
    source.push('late-mutation')
    expect(request.harnessComponents).toEqual(['system', 'tools', 'budget', 'infer'])
  })

  it('refuses new runs when preference loading fails or returns an unknown recipe', async () => {
    mockPreferenceService.get.mockRejectedValueOnce(new Error('preference unavailable'))
    await expect(withHarnessComponents({})).rejects.toThrow('preference unavailable')
    await mockPreferenceService.set(HARNESS_ASSEMBLE_KEY, 'unknown')
    await expect(withHarnessComponents({})).rejects.toThrow('Harness preference is unavailable')
  })
})
