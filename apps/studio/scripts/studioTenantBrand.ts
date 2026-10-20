import { existsSync, lstatSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'

export interface StudioTenantBrand {
  slug: string
  product: string
  /** Tenant company name in Chinese. Null when INFO.md omits name.zh. */
  companyZh: string | null
  /** Tenant company name in English. Null when INFO.md omits name.en. */
  companyEn: string | null
  /** Tenant model-service name. Null when INFO.md omits model_service. */
  modelService: string | null
  brand: string | null
  iconPath: string
  markPath: string
  logoPath: string
  /** Tenant model market. Null when model-market.json is absent. */
  modelMarketPath: string | null
}

const CHERRY_PRODUCT = 'Cherry Studio'

export function loadStudioTenantBrand(projectRoot: string, envSlug: string | undefined): StudioTenantBrand {
  const slug = (envSlug ?? 'aster').trim()
  if (!isSlug(slug)) throw new Error(`invalid tenant slug: ${slug}`)
  const directory = resolve(projectRoot, '../../apps/web/public/tenant', slug)
  const infoPath = resolve(directory, 'INFO.md')
  requireTenantFile(infoPath)
  const parsed = parseTenantInfo(readFileSync(infoPath, 'utf8'))
  if (!parsed) throw new Error(`invalid tenant INFO.md: ${slug}`)
  const iconPath = resolve(directory, 'icon.png')
  const markPath = resolve(directory, 'mark-light.png')
  const logoPath = resolve(directory, parsed.lightLogo)
  for (const asset of [iconPath, markPath, logoPath]) requireTenantFile(asset)
  const modelMarketPath = resolve(directory, 'model-market.json')
  return {
    slug,
    product: parsed.product,
    companyZh: parsed.companyZh,
    companyEn: parsed.companyEn,
    modelService: parsed.modelService,
    brand: parsed.brand,
    iconPath,
    markPath,
    logoPath,
    modelMarketPath: existsSync(modelMarketPath) ? modelMarketPath : null
  }
}

function requireTenantFile(path: string): void {
  if (!existsSync(path)) throw new Error(`missing tenant asset: ${path}`)
  const file = lstatSync(path)
  if (!file.isFile() || file.isSymbolicLink() || file.size === 0) throw new Error(`invalid tenant asset: ${path}`)
}

export function parseTenantInfo(text: string): {
  product: string
  companyZh: string | null
  companyEn: string | null
  modelService: string | null
  lightLogo: string
  brand: string | null
} | null {
  const body = text.startsWith('---\n') ? text.slice(4) : text.startsWith('---\r\n') ? text.slice(5) : null
  if (body === null) return null
  const end = body.indexOf('\n---')
  if (end < 0) return null
  let section = ''
  let product: string | null = null
  let companyZh: string | null = null
  let companyEn: string | null = null
  let modelService: string | null = null
  let lightLogo: string | null = null
  let brand: string | null = null
  for (const raw of body.slice(0, end).split('\n')) {
    if (raw.length === 0 || raw.startsWith('#')) continue
    const indent = raw.length - raw.trimStart().length
    const trimmed = raw.trim()
    if (indent === 0) {
      if (trimmed.startsWith('product:')) {
        product = fieldValue(trimmed, 'product:')
        section = ''
        continue
      }
      if (trimmed.startsWith('model_service:')) {
        const value = fieldValue(trimmed, 'model_service:')
        modelService = value && isDisplayText(value) ? value : null
        section = ''
        continue
      }
      if (trimmed.startsWith('brand:')) {
        const value = fieldValue(trimmed, 'brand:')
        brand = value && isBrandColor(value) ? value : null
        section = ''
        continue
      }
      section = trimmed === 'logos:' ? 'logos' : trimmed === 'name:' ? 'name' : ''
      continue
    }
    if (section === 'name' && trimmed.startsWith('zh:')) {
      const value = fieldValue(trimmed, 'zh:')
      companyZh = value && isDisplayText(value) ? value : null
    }
    if (section === 'name' && trimmed.startsWith('en:')) {
      const value = fieldValue(trimmed, 'en:')
      companyEn = value && isDisplayText(value) ? value : null
    }
    if (section === 'logos' && trimmed.startsWith('light_surface:')) {
      lightLogo = fieldValue(trimmed, 'light_surface:')
    }
  }
  if (!product || !lightLogo || !isLogoFile(lightLogo) || !isDisplayText(product)) return null
  return { product, companyZh, companyEn, modelService, lightLogo, brand }
}

export function stampTenantHtml(
  html: string,
  brand: Pick<StudioTenantBrand, 'slug' | 'product'> & {
    brand?: string | null
    modelService?: string | null
    companyZh?: string | null
    companyEn?: string | null
  }
): string {
  if (html.includes('data-a3s-product=')) return html
  const brandStyle = brand.brand ? ` style="--a3s-tenant-brand:${brand.brand}"` : ''
  const modelService = brand.modelService ? ` data-a3s-model-service="${escapeHtml(brand.modelService)}"` : ''
  const companyZh = brand.companyZh ? ` data-a3s-company-zh="${escapeHtml(brand.companyZh)}"` : ''
  const companyEn = brand.companyEn ? ` data-a3s-company-en="${escapeHtml(brand.companyEn)}"` : ''
  const stamped = html.replace(
    /<html\b([^>]*)>/,
    `<html$1 data-tenant="${escapeHtml(brand.slug)}" data-a3s-product="${escapeHtml(brand.product)}"${modelService}${companyZh}${companyEn}${brandStyle}>`
  )
  return stamped.replace(/<title>([^<]*)<\/title>/g, (_match, title: string) => {
    return `<title>${escapeHtml(title.replaceAll(CHERRY_PRODUCT, brand.product))}</title>`
  })
}

function fieldValue(line: string, key: string): string | null {
  const value = unquote(line.slice(key.length).trim())
  return value.length === 0 ? null : value
}

function unquote(value: string): string {
  if (value.length >= 2) {
    const first = value[0]
    const last = value[value.length - 1]
    if ((first === '"' && last === '"') || (first === "'" && last === "'")) return value.slice(1, -1)
  }
  return value
}

function isDisplayText(value: string): boolean {
  return value.length > 0 && value.length <= 80 && !value.includes('\n') && !value.includes('\r')
}

function isLogoFile(name: string): boolean {
  return (
    name.length <= 64 &&
    name.endsWith('.png') &&
    !name.includes('..') &&
    [...name].every((character) => /[A-Za-z0-9._-]/.test(character))
  )
}

function isBrandColor(value: string): boolean {
  return /^#[0-9A-Fa-f]{6}$/.test(value)
}

function isSlug(slug: string): boolean {
  return /^[a-z][a-z0-9-]{0,31}$/.test(slug)
}

function escapeHtml(value: string): string {
  return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;')
}
