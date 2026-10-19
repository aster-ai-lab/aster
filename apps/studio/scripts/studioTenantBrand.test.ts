import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

import { loadStudioTenantBrand, parseTenantInfo, stampTenantHtml } from './studioTenantBrand'

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

describe('studio tenant brand', () => {
  it('reads the product and light-surface logo from tenant front matter', () => {
    const parsed = parseTenantInfo(
      '---\nname:\n  zh: "通友科技"\nproduct: "ToWork"\nlogos:\n  light_surface: logo-light.png\n---\n'
    )
    expect(parsed).toEqual({
      product: 'ToWork',
      companyZh: '通友科技',
      companyEn: null,
      modelService: null,
      lightLogo: 'logo-light.png',
      brand: null
    })
  })

  it('reads a six-digit tenant brand color', () => {
    const parsed = parseTenantInfo(
      '---\nproduct: "ToWork"\nbrand: "#D80C18"\nlogos:\n  light_surface: logo-light.png\n---\n'
    )
    expect(parsed?.brand).toBe('#D80C18')
  })

  it('loads the isolated ToWork CUA tenant without a model market', () => {
    const brand = loadStudioTenantBrand(projectRoot, 'towork-cua')
    expect(brand?.slug).toBe('towork-cua')
    expect(brand?.product).toBe('ToWork')
    expect(brand?.companyZh).toBe('通友科技')
    expect(brand?.modelService).toBeNull()
    expect(brand?.modelMarketPath).toBeNull()
    expect(brand?.iconPath.endsWith('/tenant/towork-cua/icon.png')).toBe(true)
  })

  it('loads the toyou icon, wordmark, and logo beside INFO.md', () => {
    const brand = loadStudioTenantBrand(projectRoot, 'toyou')
    expect(brand?.slug).toBe('toyou')
    expect(brand?.product).toBe('ToWork')
    expect(brand?.companyZh).toBe('通友科技')
    expect(brand?.companyEn).toBe('Toyou AI Labs')
    expect(brand?.modelService).toBe('ToGateway')
    expect(brand?.brand).toBe('#171717')
    expect(brand?.iconPath.endsWith('/tenant/toyou/icon.png')).toBe(true)
    expect(brand?.markPath.endsWith('/tenant/toyou/mark-light.png')).toBe(true)
    expect(brand?.logoPath.endsWith('/tenant/toyou/logo-light.png')).toBe(true)
    expect(brand?.modelMarketPath?.endsWith('/tenant/toyou/model-market.json')).toBe(true)
  })

  it('stamps the main window with the tenant and replaces the Cherry title', () => {
    const html = stampTenantHtml('<html lang="zh-CN"><head><title>Cherry Studio</title></head>', {
      slug: 'toyou',
      product: 'ToWork'
    })
    expect(html).toContain('<html lang="zh-CN" data-tenant="toyou" data-a3s-product="ToWork">')
    expect(
      stampTenantHtml('<html lang="zh-CN"><head><title>Cherry Studio</title></head>', {
        slug: 'toyou',
        product: 'ToWork',
        modelService: 'ToGateway'
      })
    ).toContain('data-a3s-model-service="ToGateway"')
    expect(
      stampTenantHtml('<html lang="zh-CN"><head><title>Cherry Studio</title></head>', {
        slug: 'toyou',
        product: 'ToWork',
        companyZh: '通友科技',
        companyEn: 'Toyou AI Labs'
      })
    ).toContain('data-a3s-company-zh="通友科技" data-a3s-company-en="Toyou AI Labs"')
    expect(
      stampTenantHtml('<html lang="zh-CN"><head><title>Cherry Studio</title></head>', {
        slug: 'toyou',
        product: 'ToWork',
        brand: '#D80C18'
      })
    ).toContain('style="--a3s-tenant-brand:#D80C18"')
    expect(html).toContain('<title>ToWork</title>')
    expect(stampTenantHtml(html, { slug: 'a3s', product: 'A3S Desktop' })).toBe(html)
  })
})
