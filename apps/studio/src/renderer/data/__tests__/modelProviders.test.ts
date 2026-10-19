import { describe, expect, it } from 'vitest'

import { providerLogoId, studioProviderId } from '../modelProviders'

describe('studioProviderId', () => {
  it('resolves aliases back to known registry providers only', () => {
    const known = new Set(['zhipu', 'openai'])
    expect(studioProviderId('zhipuai', known)).toBe('zhipu')
    expect(studioProviderId('openai', known)).toBe('openai')
    expect(studioProviderId('moonshotai-cn', known)).toBeUndefined()
    expect(studioProviderId('zhipu', known)).toBeUndefined()
  })
})

describe('providerLogoId', () => {
  it('maps provider ids and their regional or plan editions to icon keys', () => {
    expect(providerLogoId('zhipuai')).toBe('zhipu')
    expect(providerLogoId('moonshotai-cn')).toBe('moonshot')
    expect(providerLogoId('deepseek')).toBe('deepseek')
  })
})
