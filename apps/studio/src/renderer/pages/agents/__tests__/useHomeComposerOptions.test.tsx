import { act, renderHook } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'

import { toast } from '@renderer/services/toast'

import { useHomeComposerOptions } from '../useHomeComposerOptions'
import { writeHomeComposerOptions } from '../workbenchHomeCapabilities'

vi.mock('@renderer/services/toast', () => ({ toast: { error: vi.fn() } }))
afterEach(() => {
  vi.restoreAllMocks()
  localStorage.clear()
})

it('shares settings across fallback, live mounts and storage events', () => {
  localStorage.clear()
  const fallback = renderHook(useHomeComposerOptions)
  act(() => fallback.result.current.changeOptions({ reviewMode: true, artifactTrace: false, researchMode: false }))
  const live = renderHook(useHomeComposerOptions)
  expect(live.result.current.options.reviewMode).toBe(true)
  act(() => live.result.current.changeOptions({ reviewMode: true, artifactTrace: true, researchMode: false }))
  expect(fallback.result.current.options.artifactTrace).toBe(true)
  act(() => {
    localStorage.setItem('workbench.composer.options.v1', JSON.stringify({ reviewMode: false, artifactTrace: false }))
    window.dispatchEvent(new StorageEvent('storage', { key: 'workbench.composer.options.v1' }))
  })
  expect(live.result.current.options.reviewMode).toBe(false)
})

it('retains saved options and reports a failed write', () => {
  writeHomeComposerOptions({ reviewMode: false, artifactTrace: true, researchMode: false })
  const options = renderHook(useHomeComposerOptions)
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
    throw new Error('quota')
  })
  act(() => options.result.current.changeOptions({ reviewMode: true, artifactTrace: false, researchMode: false }))
  expect(options.result.current.options).toEqual({ reviewMode: false, artifactTrace: true, researchMode: false })
  expect(toast.error).toHaveBeenCalled()
})
