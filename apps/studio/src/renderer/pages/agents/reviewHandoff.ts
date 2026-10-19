import { createAgentMentionToken } from '@renderer/components/composer/agentMentionToken'
import type { ComposerSerializedToken } from '@renderer/components/composer/tokens'
import { composerFileTokenId } from '@renderer/components/composer/variants/shared/composerTokens'
import { officialAgentAvatar } from '@renderer/pages/agents/officialAgentAvatars'
import { FILE_TYPE } from '@renderer/types/file'
import { createComposerFileTokenSourceId } from '@renderer/utils/message/composerFileTokenSource'
import { AbsoluteFilePathSchema } from '@shared/types/file'
import { reviewTextSourceSchema } from '@shared/types/reviewSource'

import { docxPathOnLaterLine } from './taskConversation/reviewFindings'

const DOCX = /\.docx$/i
const MAX_BYTES = 8 * 1024 * 1024

export function isDocxName(name: string): boolean {
  const trimmed = name.trim()
  return (
    trimmed.length > 5 &&
    trimmed.length <= 180 &&
    !trimmed.includes('/') &&
    !trimmed.includes('\\') &&
    DOCX.test(trimmed)
  )
}

/** Agent surface id in the review package. The host reads the skill that agent requires. */
export const REVIEW_AGENT_SURFACE_ID = 'legal-expert'

export const LEGAL_EXPERT_REVIEW_LEAD = '使用智能体 法务专家：'
const REVIEW_PLUGIN_LEAD = '使用智能体 合同审查：'
const REVIEW_LEADS = [REVIEW_PLUGIN_LEAD, LEGAL_EXPERT_REVIEW_LEAD]

export function isReviewComposerText(text: string): boolean {
  const value = text.trim()
  return REVIEW_LEADS.some((lead) => value.startsWith(lead)) || /^\/review(?=\s|$)/iu.test(value)
}

export function reviewDraftNeedsSource(draft: ReviewComposerDraft, paths: readonly string[] = []): boolean {
  if (!isReviewComposerText(draft.text)) return false
  if (
    draft.tokens.some((token) => {
      const payload = token.payload as { reviewSource?: unknown } | undefined
      return reviewTextSourceSchema.safeParse(payload?.reviewSource).success
    })
  )
    return false
  return !docxPathOnLaterLine(ensureReviewDocumentLine(draft.text, 'a3s-review', paths, draft.tokens))
}

export function reviewComposerText(fileLine: string, path: string): string {
  return `${REVIEW_PLUGIN_LEAD}\n${fileLine.trim()}\n${path.trim()}`
}

const REVIEW_MENTION_IDS = new Set([REVIEW_AGENT_SURFACE_ID, 'a3s-review'])

/** Preserve every candidate so the host can ask before opening one. */
export function ensureReviewDocumentLine(
  text: string,
  mentionId: string | undefined,
  docxPaths: readonly string[],
  tokens: readonly ComposerSerializedToken[] = []
): string {
  const trimmed = text.trim()
  const review =
    (mentionId !== undefined && REVIEW_MENTION_IDS.has(mentionId)) ||
    REVIEW_LEADS.some((lead) => trimmed.startsWith(lead))
  if (!review) return text
  const body = trimmed.length > 0 ? trimmed : REVIEW_PLUGIN_LEAD
  if (docxPathOnLaterLine(body)) return text
  const paths = [
    ...new Set(
      [
        ...docxPaths,
        ...tokens.flatMap((token) => (token.kind === 'file' && token.promptText ? [token.promptText] : []))
      ].filter((item) => AbsoluteFilePathSchema.safeParse(item).success && item.toLowerCase().endsWith('.docx'))
    )
  ]
  const lines = new Set(body.split('\n').map((line) => line.trim()))
  const missing = paths.filter((path) => !lines.has(path))
  if (missing.length === 0) return text
  return `${body}\n${missing.join('\n')}`
}

export interface ReviewComposerDraft {
  text: string
  tokens: ComposerSerializedToken[]
}

/** The path stays in the text so `/review` can open the file. The chip hides that line. */
export function reviewComposerDraft(fileLine: string, path: string, fileName: string): ReviewComposerDraft {
  const trimmedPath = path.trim()
  const name = fileName.trim()
  const text = reviewComposerText(fileLine, trimmedPath)
  const sourceId = createComposerFileTokenSourceId()
  const parsedPath = AbsoluteFilePathSchema.safeParse(trimmedPath)
  const mention = createAgentMentionToken({
    id: REVIEW_AGENT_SURFACE_ID,
    name: '合同审查',
    avatar: officialAgentAvatar(REVIEW_AGENT_SURFACE_ID) ?? ''
  })
  return {
    text,
    tokens: [
      {
        ...mention,
        index: 0,
        textOffset: 0
      },
      {
        id: composerFileTokenId({ fileTokenSourceId: sourceId }),
        kind: 'file',
        label: name,
        promptText: trimmedPath,
        index: 1,
        textOffset: text.length - trimmedPath.length,
        payload: {
          fileTokenSourceId: sourceId,
          ...(parsedPath.success ? { path: parsedPath.data } : {}),
          name,
          origin_name: name,
          ext: '.docx',
          type: FILE_TYPE.DOCUMENT
        }
      }
    ]
  }
}

export function reviewTextComposerDraft(task: string, text: string): ReviewComposerDraft {
  const source = reviewTextSourceSchema.parse({ kind: 'text', text })
  const mention = createAgentMentionToken({
    id: REVIEW_AGENT_SURFACE_ID,
    name: '合同审查',
    avatar: officialAgentAvatar(REVIEW_AGENT_SURFACE_ID) ?? ''
  })
  return {
    text: `${REVIEW_PLUGIN_LEAD}\n${task.trim()}`,
    tokens: [{ ...mention, payload: { reviewSource: source }, index: 0, textOffset: 0 }]
  }
}

export function isTextReviewHandoff(value: unknown): value is { kind: 'text'; text: string; task: string } {
  if (!value || typeof value !== 'object') return false
  const record = value as Record<string, unknown>
  return (
    reviewTextSourceSchema.safeParse({ kind: record.kind, text: record.text }).success &&
    typeof record.task === 'string' &&
    record.task.trim().length > 0 &&
    !record.task.includes('\0') &&
    new TextEncoder().encode(record.task).length <= 4096 &&
    !('path' in record) &&
    !('fileName' in record)
  )
}

function reviewDocxLabel(path: string): string {
  const base = path.split(/[/\\]/).pop()?.trim() ?? ''
  const stripped = base.replace(/^\d+-/, '')
  return stripped !== base && isDocxName(stripped) ? stripped : base
}

/** A `/review` sentence or this package's mention becomes the same chip a structured fill would insert. */
export function reviewFillFromText(text: string): ReviewComposerDraft | null {
  const trimmed = text.trim()
  const slash = /^\/review(?=\s|$)/iu.test(trimmed)
  const handoff = REVIEW_LEADS.some((lead) => trimmed.startsWith(lead))
  if (!slash && !handoff) return null
  const path = docxPathOnLaterLine(trimmed)
  if (!path) return null
  const fileLine =
    text
      .trim()
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line && line !== path && !REVIEW_LEADS.some((lead) => line.startsWith(lead)))
      .map((line) => line.replace(/^\/review\s*/iu, ''))
      .find((line) => line.length > 0) ?? '请审查这份合同'
  return reviewComposerDraft(fileLine, path, reviewDocxLabel(path))
}

export function isReviewHandoff(value: unknown): value is { path: string; fileName: string } {
  if (!value || typeof value !== 'object') return false
  const record = value as Record<string, unknown>
  return (
    typeof record.path === 'string' &&
    isDocxName(record.path.split(/[/\\]/).pop() ?? '') &&
    typeof record.fileName === 'string' &&
    isDocxName(record.fileName)
  )
}

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = typeof reader.result === 'string' ? reader.result : ''
      const comma = result.indexOf(',')
      resolve(comma >= 0 ? result.slice(comma + 1) : result)
    }
    reader.onerror = () => reject(reader.error ?? new Error('read'))
    reader.readAsDataURL(file)
  })
}

/** Stage a Word file and ask the main window to start `/review`. */
export async function deliverWorkbenchReview(file: File): Promise<void> {
  if (!isDocxName(file.name)) throw new Error('word_only')
  if (file.size < 1) throw new Error('empty')
  if (file.size > MAX_BYTES) throw new Error('too_large')
  const invoke = tauriInvoke()
  if (!invoke) throw new Error('unavailable')
  await invoke('deliver_workbench_review', {
    fileName: file.name.trim(),
    bytesBase64: await fileToBase64(file)
  })
}

export async function deliverWorkbenchTextReview(text: string, task: string): Promise<void> {
  if (!isTextReviewHandoff({ kind: 'text', text, task })) throw new Error('invalid_text')
  const invoke = tauriInvoke()
  if (!invoke) throw new Error('unavailable')
  await invoke('deliver_workbench_review', { text, task })
}
