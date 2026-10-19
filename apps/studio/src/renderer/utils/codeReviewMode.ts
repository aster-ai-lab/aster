import { preferenceService } from '@data/PreferenceService'

export const CODE_REVIEW_MODES = ['inline', 'subagent'] as const
export type CodeReviewMode = (typeof CODE_REVIEW_MODES)[number]

const KEY = 'app.code_review.mode' as const

export function getCodeReviewMode(): CodeReviewMode {
  return preferenceService.getCachedValue(KEY) === 'subagent' ? 'subagent' : 'inline'
}

export async function setCodeReviewMode(next: CodeReviewMode): Promise<CodeReviewMode> {
  await preferenceService.set(KEY, next)
  return getCodeReviewMode()
}
