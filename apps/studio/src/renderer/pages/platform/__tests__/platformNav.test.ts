import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  readPlatformExpanded,
  readPlatformFacet,
  readPlatformMenu,
  subscribePlatformView,
  writePlatformExpanded,
  writePlatformFacet,
  writePlatformMenu
} from '../platformNav'

afterEach(() => {
  vi.restoreAllMocks()
  writePlatformMenu('overview')
  localStorage.clear()
})

describe('platform navigation persistence', () => {
  it('keeps the active view in memory if storage refuses writes', () => {
    const listener = vi.fn()
    const unsubscribe = subscribePlatformView(listener)
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota')
    })
    writePlatformFacet('system', 'reference')
    writePlatformMenu('system')
    expect(readPlatformFacet('system', ['apps', 'reference'])).toBe('reference')
    expect(readPlatformMenu()).toBe('system')
    expect(listener).toHaveBeenCalledTimes(2)
    unsubscribe()
    writePlatformMenu('overview')
    expect(listener).toHaveBeenCalledTimes(2)
  })

  it('remembers open sub-menus and opens the one the reader navigates to', () => {
    localStorage.setItem('a3s.studio.platform.view:v1', JSON.stringify({ menu: 'services' }))
    expect([...readPlatformExpanded()]).toEqual(['services'])

    writePlatformExpanded('services', false)
    expect(readPlatformExpanded().has('services')).toBe(false)
    writePlatformMenu('gateway')
    writePlatformExpanded('system.reference', true)
    expect([...readPlatformExpanded()]).toEqual(['gateway', 'system.reference'])

    writePlatformExpanded('overview', true)
    expect(readPlatformExpanded().has('overview')).toBe(false)
    localStorage.setItem(
      'a3s.studio.platform.view:v1',
      JSON.stringify({ menu: 'services', expanded: ['fleet', 'billing', 7] })
    )
    expect([...readPlatformExpanded()]).toEqual(['fleet'])
  })
})
