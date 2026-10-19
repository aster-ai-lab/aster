import { useSyncExternalStore } from 'react'

import { normalizeFilePreviewPath } from '@renderer/utils/filePreview'
import type { CherryMessagePart } from '@shared/data/types/message'
import { reviewProgressSchema, type ReviewProgress } from '@shared/types/reviewSource'

export interface ReviewFinding {
  id: string
  title: string
  summary: string
  evidenceQuote: string
  guidance: string
}

export interface ReviewDocument {
  sessionId?: string
  path: string
  revision?: string
  phase?: 'reviewing' | 'finished' | 'failed' | 'cancelled'
  progress?: ReviewProgress
  findings: ReviewFinding[]
}

const FINDING_FENCE = /```a3s-review-findings?\s*([\s\S]*?)\r?\n```/giu

let current: ReviewDocument | null = null
const listeners = new Set<() => void>()

export function publishReviewDocument(next: ReviewDocument | null) {
  if (sameReviewDocument(current, next)) return
  current = next
  for (const listener of listeners) listener()
}

export function subscribeReviewDocument(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

export function getReviewDocument(): ReviewDocument | null {
  return current
}

export function useReviewDocument(): ReviewDocument | null {
  return useSyncExternalStore(subscribeReviewDocument, getReviewDocument, getReviewDocument)
}

let activeFindingId: string | null = null
let reviewFindingFocusNonce = 0
const activeFindingListeners = new Set<() => void>()

/** The finding the conversation is currently placing on the document. */
export function publishActiveReviewFinding(id: string | null) {
  if (activeFindingId === id) return
  activeFindingId = id
  for (const listener of activeFindingListeners) listener()
}

/** Select a finding and ask the document to scroll to its quote, even when it is already selected. */
export function requestReviewFindingFocus(id: string) {
  activeFindingId = id
  reviewFindingFocusNonce += 1
  for (const listener of activeFindingListeners) listener()
}

export function getReviewFindingFocusNonce(): number {
  return reviewFindingFocusNonce
}

export function getActiveReviewFinding(): string | null {
  return activeFindingId
}

export function subscribeActiveReviewFinding(listener: () => void) {
  activeFindingListeners.add(listener)
  return () => {
    activeFindingListeners.delete(listener)
  }
}

export function useActiveReviewFinding(): string | null {
  return useSyncExternalStore(subscribeActiveReviewFinding, getActiveReviewFinding, getActiveReviewFinding)
}

export function useReviewFindingFocusNonce(): number {
  return useSyncExternalStore(subscribeActiveReviewFinding, getReviewFindingFocusNonce, getReviewFindingFocusNonce)
}

export function reviewFilePathsMatch(left: string, right: string): boolean {
  return canonicalReviewPath(left) === canonicalReviewPath(right)
}

export function parseReviewFindings(source: string): ReviewFinding[] {
  const findings: ReviewFinding[] = []
  for (const match of source.matchAll(FINDING_FENCE)) {
    findings.push(...parseFindingPayload(match[1] ?? ''))
  }
  return dedupeFindings(findings)
}

function docxPathsOnLaterLines(text: string): string[] {
  return [
    ...new Set(
      text
        .trim()
        .split('\n')
        .slice(1)
        .map((line) => line.trim())
        .filter((path) => path.startsWith('/') && path.toLowerCase().endsWith('.docx') && !path.includes('\0'))
    )
  ]
}

export function docxPathOnLaterLine(text: string): string | null {
  const paths = docxPathsOnLaterLines(text)
  return paths.length === 1 ? paths[0] : null
}

export function docxPathFromReviewPrompt(text: string): string | null {
  const trimmed = text.trim()
  if (!/^\/review(?=\s|$)/iu.test(trimmed)) return null
  return docxPathOnLaterLine(trimmed)
}

export function reviewDocumentFromConversation(
  messages: readonly { id: string; role: string; status?: string; metadata?: { status?: string } }[],
  partsByMessageId: Readonly<Record<string, readonly CherryMessagePart[] | undefined>>
): ReviewDocument | null {
  let path: string | null = null
  let revision: string | undefined
  let findings: ReviewFinding[] = []
  let pendingDocx: string | null = null
  let phase: ReviewDocument['phase'] = 'finished'
  let progress: ReviewProgress | undefined
  for (const message of messages) {
    const text = messageText(partsByMessageId[message.id])
    if (message.role === 'user') {
      progress = undefined
      if (partsByMessageId[message.id]?.some((part) => part.type === 'data-review-source')) {
        path = null
        revision = undefined
        pendingDocx = null
        findings = []
        continue
      }
      const candidates = docxPathsOnLaterLines(text)
      pendingDocx = docxPathOnLaterLine(text)
      if (candidates.length > 1 || (pendingDocx && pendingDocx !== path)) {
        path = null
        revision = undefined
        findings = []
      }
      const slash =
        docxPathFromReviewPrompt(text) ||
        (['使用智能体 合同审查：', '使用智能体 法务专家：'].some((lead) => text.trim().startsWith(lead))
          ? pendingDocx
          : null)
      if (slash) {
        path = slash
        phase = 'reviewing'
        revision = undefined
        findings = []
      }
      continue
    }
    if (message.role !== 'assistant') continue
    if (
      pendingDocx &&
      partsByMessageId[message.id]?.some((part) => part.type === 'data-error') &&
      !partsByMessageId[message.id]?.some((part) => part.type === 'data-review-source')
    ) {
      path = pendingDocx
      revision = undefined
      pendingDocx = null
      findings = []
      phase = 'failed'
      continue
    }
    const selected = partsByMessageId[message.id]?.find(
      (part) => part.type === 'data-review-source' && part.data.kind === 'document'
    )
    if (selected?.type === 'data-review-source' && selected.data.kind === 'document') {
      path = selected.data.path
      revision = selected.data.sha256
      pendingDocx = null
      findings = []
    }
    const status = message.status ?? message.metadata?.status
    if (path && (selected?.type === 'data-review-source' || pendingDocx))
      phase =
        status === 'pending'
          ? 'reviewing'
          : status === 'error'
            ? 'failed'
            : status === 'paused'
              ? 'cancelled'
              : 'finished'
    const reported = partsByMessageId[message.id]?.find((part) => part.type === 'data-review-progress')
    const checked = reviewProgressSchema.safeParse(reported && 'data' in reported ? reported.data : undefined)
    if (path && selected?.type === 'data-review-source' && checked.success) progress = checked.data
    const next = parseReviewFindings(text)
    if (path) {
      findings = dedupeFindings([...findings, ...next])
      continue
    }
    if (next.length > 0 && pendingDocx) {
      path = pendingDocx
      findings = next
    }
  }
  return path
    ? {
        path,
        ...(revision ? { revision } : {}),
        ...(phase === 'finished' ? {} : { phase }),
        ...(progress ? { progress } : {}),
        findings
      }
    : null
}

function messageText(parts: readonly CherryMessagePart[] | undefined): string {
  if (!parts) return ''
  return parts.flatMap((part) => (part.type === 'text' && typeof part.text === 'string' ? [part.text] : [])).join('\n')
}

function parseFindingPayload(raw: string): ReviewFinding[] {
  const trimmed = raw.trim()
  if (!trimmed) return []
  try {
    const parsed = JSON.parse(trimmed) as unknown
    if (Array.isArray(parsed)) return parsed.flatMap((entry) => asFinding(entry) ?? [])
    const single = asFinding(parsed)
    return single ? [single] : []
  } catch {
    return []
  }
}

function asFinding(value: unknown): ReviewFinding | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const record = value as Record<string, unknown>
  const id = stringField(record, 'id')
  const title = stringField(record, 'title')
  const evidenceQuote = stringField(record, 'evidenceQuote') ?? stringField(record, 'quote')
  if (!id || !title || !evidenceQuote) return null
  return {
    id,
    title,
    summary: stringField(record, 'summary') ?? title,
    evidenceQuote,
    guidance: stringField(record, 'guidance') ?? ''
  }
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key]
  if (typeof value !== 'string') return undefined
  const trimmed = value.trim()
  return trimmed || undefined
}

function dedupeFindings(findings: readonly ReviewFinding[]): ReviewFinding[] {
  const byId = new Map<string, ReviewFinding>()
  for (const finding of findings) byId.set(finding.id, finding)
  return [...byId.values()]
}

function canonicalReviewPath(path: string): string {
  try {
    return normalizeFilePreviewPath(path)
  } catch {
    return path
  }
}

function sameReviewDocument(left: ReviewDocument | null, right: ReviewDocument | null): boolean {
  if (left === right) return true
  if (
    !left ||
    !right ||
    left.path !== right.path ||
    left.sessionId !== right.sessionId ||
    left.revision !== right.revision ||
    left.phase !== right.phase ||
    left.progress?.index !== right.progress?.index ||
    left.progress?.total !== right.progress?.total ||
    left.progress?.quote !== right.progress?.quote ||
    left.findings.length !== right.findings.length
  )
    return false
  return left.findings.every((finding, index) => {
    const other = right.findings[index]
    return (
      other?.id === finding.id &&
      other.title === finding.title &&
      other.evidenceQuote === finding.evidenceQuote &&
      other.summary === finding.summary &&
      other.guidance === finding.guidance
    )
  })
}
