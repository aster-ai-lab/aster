import { resolveProviderIconRef, useIcon } from '@a3s-lab/ui/icons'
import type { CSSProperties } from 'react'

import { type ProviderAvatarDisplayContext, ProviderAvatarPrimitive } from '@renderer/components/ProviderAvatar'
import { studioModelServiceLogo } from '@renderer/tenantBrand'
import type { Provider } from '@shared/data/types/provider'

interface ProviderAvatarProps {
  provider: Pick<Provider, 'id' | 'name' | 'logo' | 'logoSrc'>
  size?: number
  className?: string
  style?: CSSProperties
  displayContext?: ProviderAvatarDisplayContext
}

export function ProviderAvatar({ provider, size, className, style, displayContext }: ProviderAvatarProps) {
  const tenantLogo = provider.id === 'a3s' ? studioModelServiceLogo() : ''
  if (tenantLogo) {
    return (
      <ProviderAvatarPrimitive
        providerId={provider.id}
        providerName={provider.name}
        logo={tenantLogo}
        size={size}
        className={className}
        style={style}
        displayContext={displayContext}
      />
    )
  }
  // Existence is decided synchronously from the ref (meta catalog); only the
  // component itself loads async, so the branch below never flip-flops.
  const systemIconRef = resolveProviderIconRef(provider.id)
  const systemIcon = useIcon(systemIconRef)
  // Preset providers render the bundled icon; custom providers carry either a
  // preset brand key (`icon:<id>` on `logo`) or a main-resolved uploaded-logo
  // URL (`logoSrc`). The primitive dispatches on both.
  const customLogo = systemIconRef ? undefined : (provider.logo ?? provider.logoSrc)
  if (systemIconRef) {
    return (
      <ProviderAvatarPrimitive
        providerId={provider.id}
        providerName={provider.name}
        logo={systemIcon}
        size={size}
        className={className}
        style={style}
        displayContext={displayContext}
      />
    )
  }

  if (customLogo) {
    return (
      <ProviderAvatarPrimitive
        providerId={provider.id}
        providerName={provider.name}
        logo={customLogo}
        size={size}
        className={className}
        style={style}
        displayContext={displayContext}
      />
    )
  }

  return (
    <ProviderAvatarPrimitive
      providerId={provider.id}
      providerName={provider.name}
      size={size}
      className={className}
      style={style}
      displayContext={displayContext}
    />
  )
}
