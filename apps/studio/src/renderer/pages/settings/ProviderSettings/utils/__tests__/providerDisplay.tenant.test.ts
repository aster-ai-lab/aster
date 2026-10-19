import { describe, expect, it, vi } from 'vitest'

import type { Provider } from '@shared/data/types/provider'

const modelService = vi.hoisted(() => ({ name: '' }))

vi.mock('@renderer/i18n/resolver', () => ({ default: { t: (key: string) => key } }))
vi.mock('@renderer/i18n/label', () => ({ getProviderLabelKey: (id: string) => `provider.${id}` }))
vi.mock('@renderer/tenantBrand', () => ({
  studioModelServiceName: () => modelService.name
}))

const { getFancyProviderName, getProviderSearchString } = await import('../providerDisplay')

const a3s = { id: 'a3s', name: 'A3S', presetProviderId: 'a3s' } as Provider

describe('tenant model service name', () => {
  it('shows the tenant service name for A3S', () => {
    modelService.name = 'ToGateway'
    expect(getFancyProviderName(a3s)).toBe('ToGateway')
    expect(getProviderSearchString(a3s)).toContain('ToGateway')
  })

  it('keeps the A3S label when the tenant has no service name', () => {
    modelService.name = ''
    expect(getFancyProviderName(a3s)).toBe('provider.a3s')
  })

  it('leaves other providers on their own label', () => {
    modelService.name = 'ToGateway'
    expect(getFancyProviderName({ id: 'openai', name: 'OpenAI', presetProviderId: 'openai' } as Provider)).toBe(
      'provider.openai'
    )
  })
})
