// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

const modelService = vi.hoisted(() => ({ logo: '' }))

vi.mock('@renderer/tenantBrand', () => ({
  studioModelServiceLogo: () => modelService.logo
}))
vi.mock('@renderer/components/ProviderAvatar', () => ({
  ProviderAvatarPrimitive: ({ logo, providerName }: { logo?: string; providerName: string }) => (
    <img alt={providerName} src={typeof logo === 'string' ? logo : undefined} />
  )
}))
vi.mock('@a3s-lab/ui/icons', () => ({
  resolveProviderIconRef: () => undefined,
  useIcon: () => undefined
}))

import { ProviderAvatar } from '../ProviderAvatar'

afterEach(() => {
  cleanup()
  modelService.logo = ''
})

describe('ProviderAvatar tenant model service', () => {
  it('uses the tenant logo for A3S', () => {
    modelService.logo = '/tenant-logo.png'
    const { getByRole } = render(
      <ProviderAvatar provider={{ id: 'a3s', name: 'ToGateway' }} size={26} displayContext="provider-list" />
    )
    expect(getByRole('img', { name: 'ToGateway' })).toHaveAttribute('src', '/tenant-logo.png')
  })

  it('does not replace another provider logo', () => {
    modelService.logo = '/tenant-logo.png'
    const { getByRole } = render(
      <ProviderAvatar provider={{ id: 'openai', name: 'OpenAI' }} size={26} displayContext="provider-list" />
    )
    expect(getByRole('img', { name: 'OpenAI' }).getAttribute('src')).not.toBe('/tenant-logo.png')
  })
})
