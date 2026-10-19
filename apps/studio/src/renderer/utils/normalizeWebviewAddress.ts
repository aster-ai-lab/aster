const URL_SCHEME_PATTERN = /^[a-z][a-z\d+.-]*:/i
const HOST_PORT_PATTERN = /^(?:\[[^\]]+\]|[^:/?#\s]+):\d+(?:[/?#]|$)/
const LOCAL_ADDRESS_PATTERN = /^(?:localhost|127(?:\.\d{1,3}){3}|0\.0\.0\.0|\[?::1\]?)(?::\d+)?(?:[/?#]|$)/i
const HOST_PATTERN = /^(?:[^\s/?#]+\.)+[^\s/?#]+(?::\d+)?(?:[/?#]|$)/
const ALLOWED_PROTOCOLS = new Set(['file:', 'http:', 'https:'])
const QUARK_SEARCH_URL = 'https://quark.sm.cn/s'

export function normalizeWebviewAddress(value: string): string | null {
  const trimmedValue = value.trim()
  if (!trimmedValue) return null

  const candidate = LOCAL_ADDRESS_PATTERN.test(trimmedValue)
    ? `http://${trimmedValue}`
    : URL_SCHEME_PATTERN.test(trimmedValue) && !HOST_PORT_PATTERN.test(trimmedValue)
      ? trimmedValue
      : `https://${trimmedValue}`

  try {
    const url = new URL(candidate)
    return ALLOWED_PROTOCOLS.has(url.protocol) ? url.toString() : null
  } catch {
    return null
  }
}

function looksLikeWebAddress(value: string): boolean {
  if (/\s/.test(value)) return false
  return URL_SCHEME_PATTERN.test(value) || LOCAL_ADDRESS_PATTERN.test(value) || HOST_PATTERN.test(value)
}

/** A pasted absolute HTML path opens as a local page. Other text stays a search. */
function localHtmlAddress(value: string): string | null {
  const stripped = value.trim().replace(/^['"`]+|['"`]+$/g, '')
  const drive = /^[A-Za-z]:[\\/]/.test(stripped)
  if (!stripped || (!drive && /^[a-z][a-z\d+.-]*:/i.test(stripped))) return null
  if (!drive && !stripped.startsWith('/')) return null
  if (stripped.includes('\0') || /\s/u.test(stripped)) return null
  const name = stripped.split(/[/\\]/u).pop() ?? ''
  const extension = name.slice(name.lastIndexOf('.') + 1).toLowerCase()
  if (name.lastIndexOf('.') <= 0 || (extension !== 'html' && extension !== 'htm')) return null
  const slashed = stripped.replace(/\\/g, '/')
  try {
    const url = new URL(`file://${slashed.startsWith('/') ? '' : '/'}${slashed}`)
    return url.protocol === 'file:' ? url.toString() : null
  } catch {
    return null
  }
}

/** A web address stays a page. Other text opens on Quark search. */
export function workspaceBrowserAddress(value: string): string | null {
  const trimmed = value.trim()
  if (!trimmed) return null
  const localHtml = localHtmlAddress(trimmed)
  if (localHtml) return localHtml
  if (looksLikeWebAddress(trimmed)) return normalizeWebviewAddress(trimmed)
  const url = new URL(QUARK_SEARCH_URL)
  url.searchParams.set('q', trimmed)
  return url.toString()
}

/** Text that opens a Quark search. A web address returns null. */
export function quarkSearchQuery(value: string): string | null {
  const trimmed = value.trim()
  if (!trimmed) return null
  const address = workspaceBrowserAddress(trimmed)
  if (!address) return null
  try {
    const url = new URL(address)
    return url.origin === 'https://quark.sm.cn' && url.pathname === '/s' ? trimmed : null
  } catch {
    return null
  }
}
