import type { MailFile } from '@renderer/mail/mailApi'

export const ATTACHMENT_CAP = 8
export const ATTACHMENT_BYTES = 512_000
export const ATTACHMENT_TOTAL = 1_500_000

export type AttachmentProblem = 'name' | 'too_large' | 'too_many' | 'total' | 'read'

export function cleanAttachmentName(name: string): string | null {
  const base = name.split(/[/\\]/u).pop() ?? ''
  const cleaned = base.replace(/[\u0000-\u001f\u007f]/g, '').trim()
  if (!cleaned || cleaned.length > 120 || cleaned.includes('/') || cleaned.includes('\\')) return null
  return cleaned
}

export function decodedBytes(data: string): number {
  const trimmed = data.replace(/=+$/u, '')
  if (!trimmed) return 0
  return Math.floor((trimmed.length * 3) / 4)
}

export function attachmentProblem(files: MailFile[], extraBytes: number): AttachmentProblem | null {
  if (files.length >= ATTACHMENT_CAP) return 'too_many'
  if (extraBytes > ATTACHMENT_BYTES) return 'too_large'
  const total = files.reduce((sum, file) => sum + decodedBytes(file.data), 0) + extraBytes
  if (total > ATTACHMENT_TOTAL) return 'total'
  return null
}

function fileBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const text = String(reader.result ?? '')
      const comma = text.indexOf(',')
      resolve(comma >= 0 ? text.slice(comma + 1) : '')
    }
    reader.onerror = () => reject(reader.error ?? new Error('read'))
    reader.readAsDataURL(file)
  })
}

export async function readMailAttachment(file: File): Promise<{ file: MailFile } | { error: AttachmentProblem }> {
  const name = cleanAttachmentName(file.name)
  if (!name) return { error: 'name' }
  if (file.size > ATTACHMENT_BYTES) return { error: 'too_large' }
  try {
    const data = await fileBase64(file)
    if (!data) return { error: 'read' }
    return { file: { name, mime: file.type || 'application/octet-stream', data } }
  } catch {
    return { error: 'read' }
  }
}

export function attachmentDataUrl(file: MailFile): string | null {
  if (!file.data || !/^[A-Za-z0-9+/]+={0,2}$/u.test(file.data) || file.data.length % 4 !== 0) return null
  const mime = /^[a-z0-9][a-z0-9.+-]{0,40}\/[a-z0-9][a-z0-9.+-]{0,80}$/i.test(file.mime)
    ? file.mime
    : 'application/octet-stream'
  return `data:${mime};base64,${file.data}`
}

export function downloadMailAttachment(file: MailFile): void {
  const href = attachmentDataUrl(file)
  if (!href) return
  const link = document.createElement('a')
  link.href = href
  link.download = file.name
  link.click()
}
