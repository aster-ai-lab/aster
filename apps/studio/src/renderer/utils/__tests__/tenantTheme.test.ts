import { afterEach, describe, expect, it } from 'vitest'

import { LEGACY_THEME_PRIMARY, readTenantBrand, resolveThemePrimary } from '../tenantTheme'

describe('resolveThemePrimary', () => {
  afterEach(() => {
    document.documentElement.removeAttribute('style')
  })

  it('keeps the stock green when no tenant brand is stamped', () => {
    expect(readTenantBrand()).toBeNull()
    expect(resolveThemePrimary(undefined)).toBe(LEGACY_THEME_PRIMARY)
    expect(resolveThemePrimary('')).toBe(LEGACY_THEME_PRIMARY)
    expect(resolveThemePrimary('#00B96B')).toBe('#00B96B')
  })

  it('uses the stamped tenant brand for an empty or stock-green preference', () => {
    document.documentElement.style.setProperty('--a3s-tenant-brand', '#D80C18')

    expect(readTenantBrand()).toBe('#D80C18')
    expect(resolveThemePrimary(undefined)).toBe('#D80C18')
    expect(resolveThemePrimary('  ')).toBe('#D80C18')
    expect(resolveThemePrimary(LEGACY_THEME_PRIMARY)).toBe('#D80C18')
    expect(resolveThemePrimary('#00B96B')).toBe('#D80C18')
  })

  it('keeps a preference the user chose', () => {
    document.documentElement.style.setProperty('--a3s-tenant-brand', '#D80C18')

    expect(resolveThemePrimary('#3B82F6')).toBe('#3B82F6')
  })

  it('ignores a tenant stamp that is not a 6-digit hex color', () => {
    document.documentElement.style.setProperty('--a3s-tenant-brand', 'initial')

    expect(readTenantBrand()).toBeNull()
    expect(resolveThemePrimary(LEGACY_THEME_PRIMARY)).toBe(LEGACY_THEME_PRIMARY)
  })
})
