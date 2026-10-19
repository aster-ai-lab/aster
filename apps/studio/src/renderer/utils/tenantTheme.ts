/** Cherry's stock primary. A saved value of this color means "no custom theme". */
export const LEGACY_THEME_PRIMARY = '#00b96b'

const BRAND_COLOR = /^#[0-9A-Fa-f]{6}$/

export function readTenantBrand(root: Pick<HTMLElement, 'style'> | null = rootElement()): string | null {
  if (!root) return null
  const inline = root.style.getPropertyValue('--a3s-tenant-brand').trim()
  if (BRAND_COLOR.test(inline)) return inline
  if (typeof document === 'undefined') return null
  const computed = getComputedStyle(document.documentElement).getPropertyValue('--a3s-tenant-brand').trim()
  return BRAND_COLOR.test(computed) ? computed : null
}

/** Theme primary for every mode. The stock green yields to the tenant brand. */
export function resolveThemePrimary(stored: string | undefined | null): string {
  const tenant = readTenantBrand()
  const value = (stored ?? '').trim()
  if (tenant && (!value || value.toLowerCase() === LEGACY_THEME_PRIMARY)) return tenant
  return value || LEGACY_THEME_PRIMARY
}

function rootElement(): HTMLElement | null {
  return typeof document === 'undefined' ? null : document.documentElement
}

