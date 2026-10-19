/** Which unread changes become a Dock count or one banner. The first inbox does not notify. */

export type AttentionRow = {
  id: string
  kind: string
  unread: number
  muted: boolean
  title: string
  body: string
}

export type AttentionAction =
  | { type: 'badge'; count: number }
  | { type: 'notify'; id: string; title: string; body: string }
  | { type: 'withdraw'; id: string }

export type TobotNotice =
  | { type: 'note'; id: string }
  | { type: 'publish'; id: string; body: string }
  | { type: 'withdraw'; id: string }
  | { type: 'ignore' }

const BODY_LIMIT = 140

/** One line for a banner. Empty text uses the fallback. Longer text keeps 139 characters and an ellipsis. */
export function notificationBody(text: string, fallback: string): string {
  const collapsed = text.replace(/\s+/g, ' ').trim()
  if (!collapsed) return fallback
  if (collapsed.length <= BODY_LIMIT) return collapsed
  return `${collapsed.slice(0, BODY_LIMIT - 1)}…`
}

export function attentionStep(
  previous: readonly AttentionRow[] | null,
  next: readonly AttentionRow[],
  lookedAtId: string | null
): AttentionAction[] {
  const count = next.reduce((sum, row) => (row.muted ? sum : sum + Math.max(0, row.unread)), 0)
  const actions: AttentionAction[] = [{ type: 'badge', count }]
  if (!previous) return actions
  const before = new Map(previous.map((row) => [row.id, row.unread]))
  const seen = new Set(next.map((row) => row.id))
  for (const row of next) {
    const oldUnread = before.get(row.id) ?? 0
    if (oldUnread > 0 && row.unread === 0) actions.push({ type: 'withdraw', id: row.id })
    if (row.kind === 'transfer' || row.kind === 'boss' || row.muted || row.id === lookedAtId || row.unread <= oldUnread)
      continue
    actions.push({ type: 'notify', id: row.id, title: row.title, body: row.body })
  }
  for (const row of previous) {
    if (row.unread > 0 && !seen.has(row.id)) actions.push({ type: 'withdraw', id: row.id })
  }
  return actions
}

/** The first live token marks ToBot unread once. A reply that is already unread is not counted again. */
export function tobotDeltaAction(input: {
  noted: boolean
  looking: boolean
  unread: number
  id: string | null
}): TobotNotice {
  if (input.noted || input.looking || !input.id || input.unread > 0) return { type: 'ignore' }
  return { type: 'note', id: input.id }
}

/** One banner when the reply finishes. The open conversation withdraws that banner instead. */
export function tobotSettleAction(input: {
  text: string
  looking: boolean
  id: string | null
  fallback: string
}): TobotNotice {
  if (!input.id) return { type: 'ignore' }
  if (input.looking) return { type: 'withdraw', id: input.id }
  return { type: 'publish', id: input.id, body: notificationBody(input.text, input.fallback) }
}
