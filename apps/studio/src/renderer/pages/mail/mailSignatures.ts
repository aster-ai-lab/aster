import type { ComposeKind } from './mailCompose'
import { isMailHtml, plainToMailHtml, sanitizeMailHtml } from './mailDocument'

export const SIGNATURE_TAG = 'mail-signature'

export const SIGNATURE_KEY = 'mail.signatures.v1'
export const SIGNATURE_CAP = 20
export const SIGNATURE_NAME_CAP = 40
export const SIGNATURE_TEXT_CAP = 2000

export type MailSignature = {
  id: string
  name: string
  text: string
  isDefault: boolean
}

function clip(value: string, max: number): string {
  return value.replace(/\r\n/g, '\n').trim().slice(0, max)
}

function asSignature(value: unknown): MailSignature | null {
  if (!value || typeof value !== 'object') return null
  const row = value as Record<string, unknown>
  if (typeof row.id !== 'string' || !row.id || row.id.length > 80) return null
  if (typeof row.name !== 'string' || typeof row.text !== 'string') return null
  const name = clip(row.name, SIGNATURE_NAME_CAP)
  const text = clip(row.text, SIGNATURE_TEXT_CAP)
  if (!name || !text) return null
  return { id: row.id, name, text, isDefault: row.isDefault === true }
}

export function loadSignatures(): MailSignature[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(SIGNATURE_KEY) ?? '[]') as unknown
    if (!Array.isArray(parsed)) return []
    const items = parsed.flatMap((item) => {
      const signature = asSignature(item)
      return signature ? [signature] : []
    })
    const capped = items.slice(0, SIGNATURE_CAP)
    const defaultId = capped.find((item) => item.isDefault)?.id
    return capped.map((item) => ({ ...item, isDefault: item.id === defaultId }))
  } catch {
    return []
  }
}

export function saveSignatures(items: MailSignature[]): MailSignature[] {
  const next = items.slice(0, SIGNATURE_CAP).map((item) => ({
    id: item.id,
    name: clip(item.name, SIGNATURE_NAME_CAP),
    text: clip(item.text, SIGNATURE_TEXT_CAP),
    isDefault: item.isDefault
  }))
  const usable = next.filter((item) => item.name && item.text)
  const defaultId = usable.find((item) => item.isDefault)?.id
  const stored = usable.map((item) => ({ ...item, isDefault: item.id === defaultId }))
  localStorage.setItem(SIGNATURE_KEY, JSON.stringify(stored))
  return stored
}

export function defaultSignature(): MailSignature | null {
  return loadSignatures().find((item) => item.isDefault) ?? null
}

export function signatureHtml(text: string): string {
  const inner = sanitizeMailHtml(plainToMailHtml(clip(text, SIGNATURE_TEXT_CAP)))
  if (!inner) return ''
  return `<${SIGNATURE_TAG}>${inner}</${SIGNATURE_TAG}>`
}

function htmlBody(body: string): string {
  const trimmed = body.trim()
  if (!trimmed) return ''
  return isMailHtml(trimmed) ? trimmed : plainToMailHtml(trimmed)
}

/**
 * Puts the signature after the letter and before a quoted reply.
 * A signature already on this letter is replaced. The letter itself stays.
 */
export function placeSignature(body: string, signatureText: string): string {
  const html = signatureHtml(signatureText)
  if (!html) return body
  const holder = document.createElement('div')
  holder.innerHTML = html
  const next = holder.firstElementChild
  if (!next) return body
  const root = document.createElement('div')
  root.innerHTML = htmlBody(body)
  const existing = [...root.querySelectorAll(SIGNATURE_TAG)].find((node) => !node.closest('blockquote'))
  if (existing) {
    existing.replaceWith(next)
    return root.innerHTML
  }
  if (root.childNodes.length === 0) {
    const gap = document.createElement('p')
    gap.appendChild(document.createElement('br'))
    root.appendChild(gap)
  }
  const quote = [...root.children].find((node) => node.tagName === 'BLOCKQUOTE')
  if (quote) root.insertBefore(next, quote)
  else root.appendChild(next)
  return root.innerHTML
}

/** The default signature is written once, after the letter or before a reply quote. Drafts stay as saved. */
export function openingBody(kind: ComposeKind, body: string, signatureText: string): string {
  if (kind === 'draft' || !signatureText.trim()) return body
  return placeSignature(body, signatureText)
}

export function newSignatureId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') return crypto.randomUUID()
  return `sig-${Date.now()}`
}
