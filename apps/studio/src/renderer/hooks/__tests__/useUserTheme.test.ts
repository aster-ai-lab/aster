import { MockUsePreferenceUtils } from '@test-mocks/renderer/usePreference'
import { act, renderHook } from '@testing-library/react'
import Color from 'color'
import { beforeEach, describe, expect, it } from 'vitest'

import useUserTheme from '../useUserTheme'

describe('useUserTheme', () => {
  beforeEach(() => {
    MockUsePreferenceUtils.resetMocks()
    document.documentElement.removeAttribute('style')
    delete document.documentElement.dataset.themePack
    delete document.documentElement.dataset.density
    delete document.documentElement.dataset.chatWidth
    delete document.documentElement.dataset.contrast
  })

  it.each([
    ['#FFFFFF', '#000000'],
    ['#000000', '#FFFFFF'],
    ['#00B96B', '#000000']
  ])('derives a contrast-safe foreground for primary %s', (primary, foreground) => {
    const { result } = renderHook(() => useUserTheme())

    act(() => result.current.initUserTheme({ colorPrimary: primary }))

    expect(document.documentElement.style.getPropertyValue('--cs-theme-primary-foreground')).toBe(foreground)
  })

  it('keeps user font inputs in the renderer-owned namespace', () => {
    MockUsePreferenceUtils.setMultiplePreferenceValues({
      'ui.theme_user.font_family': 'Inter',
      'ui.theme_user.code_font_family': 'Fira Code'
    })
    const { result } = renderHook(() => useUserTheme())

    act(() => result.current.initUserTheme({ colorPrimary: '#00B96B' }))

    expect(document.documentElement.style.getPropertyValue('--app-user-font-family')).toBe("'Inter'")
    expect(document.documentElement.style.getPropertyValue('--app-user-code-font-family')).toBe("'Fira Code'")
  })

  it('paints the stamped tenant brand when the saved primary is the stock green', () => {
    document.documentElement.style.setProperty('--a3s-tenant-brand', '#D80C18')
    MockUsePreferenceUtils.setPreferenceValue('ui.theme_user.color_primary', '#00b96b')
    const { result } = renderHook(() => useUserTheme())

    act(() => result.current.initUserTheme({ colorPrimary: '#00b96b' }))

    expect(document.documentElement.style.getPropertyValue('--cs-theme-primary')).toBe(Color('#D80C18').toString())
    expect(result.current.userTheme.colorPrimary).toBe('#D80C18')
  })

  it('leaves the aster default primary to its light and dark tenant stylesheet', () => {
    document.documentElement.dataset.tenant = 'aster'
    document.documentElement.style.setProperty('--a3s-tenant-brand', '#005ABB')
    try {
      const { result } = renderHook(() => useUserTheme())
      act(() => result.current.initUserTheme({ colorPrimary: '#00b96b' }))
      expect(document.documentElement.style.getPropertyValue('--cs-theme-primary')).toBe('')
      expect(document.documentElement.style.getPropertyValue('--cs-theme-primary-foreground')).toBe('')
      act(() => result.current.initUserTheme({ colorPrimary: '#AB2244' }))
      expect(document.documentElement.style.getPropertyValue('--cs-theme-primary')).toBe(Color('#AB2244').toString())
    } finally {
      delete document.documentElement.dataset.tenant
    }
  })

  it('keeps a saved primary that is not the stock green', () => {
    document.documentElement.style.setProperty('--a3s-tenant-brand', '#D80C18')
    MockUsePreferenceUtils.setPreferenceValue('ui.theme_user.color_primary', '#3B82F6')
    const { result } = renderHook(() => useUserTheme())

    act(() => result.current.initUserTheme({ colorPrimary: '#3B82F6' }))

    expect(document.documentElement.style.getPropertyValue('--cs-theme-primary')).toBe(Color('#3B82F6').toString())
    expect(result.current.userTheme.colorPrimary).toBe('#3B82F6')
  })

  it('applies the saved theme pack, density, chat width, and contrast', () => {
    MockUsePreferenceUtils.setMultiplePreferenceValues({
      'app.appearance.theme_pack': 'ocean',
      'app.appearance.density': 'compact',
      'app.appearance.chat_width': 'full',
      'app.appearance.contrast': 'high'
    })
    const { result } = renderHook(() => useUserTheme())

    act(() => result.current.initUserTheme())

    expect(document.documentElement.dataset.themePack).toBe('ocean')
    expect(document.documentElement.dataset.density).toBe('compact')
    expect(document.documentElement.dataset.chatWidth).toBe('full')
    expect(document.documentElement.dataset.contrast).toBe('high')
    expect(document.documentElement.style.getPropertyValue('--app-chat-max-width')).toBe('none')
  })
})
