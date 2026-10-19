import AppLogo from '@renderer/assets/images/logo.png'

const CHERRY_PRODUCT = 'Cherry Studio'

export function studioProductName(): string {
  if (typeof document === 'undefined') return CHERRY_PRODUCT
  return document.documentElement.dataset.a3sProduct || CHERRY_PRODUCT
}

export function studioProductMark(): string {
  if (typeof document === 'undefined' || !document.documentElement.dataset.tenant) return AppLogo
  return '/tenant-icon.png'
}

/** Square tenant logo for the default user avatar. Empty when this build has no tenant. */
export function studioProductLogo(): string {
  if (typeof document === 'undefined' || !document.documentElement.dataset.tenant) return ''
  return '/tenant-logo.png'
}

/**
 * Tenant company name for the current UI language.
 * Chinese uses INFO.md `name.zh`; every other language uses `name.en`.
 * The other name fills in when the preferred one is absent.
 */
export function studioCompanyName(language: string): string {
  if (typeof document === 'undefined') return ''
  const zh = document.documentElement.dataset.a3sCompanyZh?.trim() ?? ''
  const en = document.documentElement.dataset.a3sCompanyEn?.trim() ?? ''
  const chinese = language.toLowerCase().startsWith('zh')
  return (chinese ? zh || en : en || zh) || ''
}

/** Tenant model-service name from INFO.md `model_service`. Empty when this tenant has none. */
export function studioModelServiceName(): string {
  if (typeof document === 'undefined') return ''
  return document.documentElement.dataset.a3sModelService?.trim() ?? ''
}

/** Tenant logo used for the model-service provider. Empty when this tenant has no service name. */
export function studioModelServiceLogo(): string {
  if (!studioModelServiceName()) return ''
  return studioProductLogo()
}

/** The launch tenant already configures this provider. Users do not add a key or host. */
export function isManagedModelService(providerId: string): boolean {
  return providerId === 'a3s' && studioModelServiceName().length > 0
}
