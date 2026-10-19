import DOMPurify from 'dompurify'

const MAIL_TAGS = [
  'a',
  'b',
  'blockquote',
  'br',
  'em',
  'h2',
  'h3',
  'i',
  'li',
  'mail-signature',
  'ol',
  'p',
  's',
  'strike',
  'strong',
  'u',
  'ul'
]

/** Plain letters, including the welcome note, stay text until a tag starts the body. */
export function isMailHtml(body: string): boolean {
  return /^\s*</.test(body)
}

export function escapeMailText(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

export function plainToMailHtml(value: string): string {
  const text = value.replace(/\r\n/g, '\n').trim()
  if (!text) return ''
  return text
    .split(/\n{2,}/)
    .map((part) => `<p>${escapeMailText(part).replace(/\n/g, '<br>')}</p>`)
    .join('')
}

export function sanitizeMailHtml(value: string): string {
  const clean = DOMPurify.sanitize(value, {
    ALLOWED_TAGS: MAIL_TAGS,
    ALLOWED_ATTR: ['href'],
    ALLOW_DATA_ATTR: false,
    ALLOWED_URI_REGEXP: /^(?:https?:|mailto:)/i
  })
  const root = document.createElement('div')
  root.innerHTML = clean
  for (const link of root.querySelectorAll('a')) {
    const href = link.getAttribute('href') ?? ''
    if (!/^(?:https?:|mailto:)/i.test(href)) {
      link.replaceWith(document.createTextNode(link.textContent ?? ''))
      continue
    }
    link.setAttribute('rel', 'noopener noreferrer')
    link.setAttribute('target', '_blank')
  }
  for (const block of root.querySelectorAll('mail-signature')) {
    for (const attr of [...block.attributes]) block.removeAttribute(attr.name)
  }
  return root.innerHTML
}

export function quoteMailBody(who: string, body: string): string {
  const inner = isMailHtml(body) ? sanitizeMailHtml(body) : plainToMailHtml(body)
  if (!inner) return ''
  const name = escapeMailText(who.trim())
  const label = name ? `<p>${name}</p>` : ''
  return `<p></p><blockquote>${label}${inner}</blockquote>`
}

export function editorSeed(body: string): string {
  const trimmed = body.trim()
  if (!trimmed) return ''
  const html = isMailHtml(trimmed) ? trimmed : plainToMailHtml(trimmed)
  return sanitizeMailHtml(html)
}

export function mailBodyText(body: string): string {
  return body
    .replace(/<(script|style)\b[^>]*>[\s\S]*?(?:<\/\1>|$)/gi, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/gi, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/\s+/g, ' ')
    .trim()
}

export function normalizeMailHref(value: string): string | null {
  const text = value.trim()
  if (!text || /[\s"'<>]/.test(text)) return null
  if (/^mailto:/i.test(text)) return text.slice('mailto:'.length).includes('@') ? text : null
  if (/^https?:\/\/.+/i.test(text)) return text
  if (/^[a-z0-9][a-z0-9.-]*\.[a-z]{2,}(?:[/?#][^\s]*)?$/i.test(text)) return `https://${text}`
  return null
}
