import { excludeComposerDraftTokens } from '@renderer/components/composer/composerDraft'
import type { ComposerSerializedDraft } from '@renderer/components/composer/tokens'

import { agentIdFromMentionToken } from '../agentWiki'
import { PAPERLOOM_SLUG } from './paperloomApi'

export function paperloomDraftQuestion(draft: Pick<ComposerSerializedDraft, 'text' | 'tokens'>): string | null {
  const isPaperloom = (token: ComposerSerializedDraft['tokens'][number]) =>
    token.kind === 'reference' && agentIdFromMentionToken(token.id) === PAPERLOOM_SLUG
  if (!draft.tokens.some(isPaperloom)) return null
  return excludeComposerDraftTokens(
    draft,
    (token) => isPaperloom(token) || token.kind === 'file' || token.kind === 'folder'
  ).text.trim()
}

export function samePaperloomDraft(current: ComposerSerializedDraft, sent: ComposerSerializedDraft): boolean {
  return (
    current.text === sent.text &&
    current.tokens.length === sent.tokens.length &&
    current.tokens.every((token, index) => token.id === sent.tokens[index].id)
  )
}
