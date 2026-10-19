import { getCodeReviewMode, type CodeReviewMode } from '@renderer/utils/codeReviewMode'
import type { StudioHostContext } from '@shared/ai/transport'

const SUBAGENT_LINE = '代码审查：在独立子会话中审查本次改动，不要把审查过程写进当前会话。'

export type AlignedPreface = {
  customInstructions: string
  reviewMode: CodeReviewMode
}

/** Saved instructions and the review mode travel with the turn. Permission switches stay on the host. */
export function applyAlignedSessionPreface(text: string, preface: AlignedPreface): string {
  const custom = preface.customInstructions.trim()
  const lines = [preface.reviewMode === 'subagent' ? SUBAGENT_LINE : ''].filter((line) => line.length > 0)
  let next = text
  if (custom && !next.includes(custom)) {
    const trimmed = next.trim()
    next = trimmed ? `${custom}\n\n${trimmed}` : custom
  }
  const extra = lines.filter((line) => !next.includes(line))
  if (extra.length === 0) return next
  const trimmed = next.replace(/\s+$/, '')
  return trimmed ? `${trimmed}\n\n${extra.join('\n')}` : extra.join('\n')
}

/** The text a chat composer sends after saved instructions and the review mode. */
export function prefaceComposerText(text: string, preface: AlignedPreface): string {
  const reviewMode = preface.reviewMode === 'subagent' || getCodeReviewMode() === 'subagent' ? 'subagent' : 'inline'
  return applyAlignedSessionPreface(text, { ...preface, reviewMode })
}

/** Standing instructions for an a3s-code session. They stay out of the user message. */
export function agentHostContext(
  preface: AlignedPreface,
  outputLanguage: StudioHostContext['outputLanguage'],
  workspaceRoot?: string
): StudioHostContext {
  const root = workspaceRoot
    ?.replaceAll('\0', '')
    .trim()
    .replace(/[\\/]+$/, '')
  const wiki = root ? `${root}/wiki` : ''
  return {
    guidelines: preface.customInstructions.replaceAll('\0', '').trim().slice(0, 8000),
    review: preface.reviewMode === 'subagent' ? 'subagent' : 'inline',
    outputLanguage,
    ...(wiki ? { wiki } : {})
  }
}
