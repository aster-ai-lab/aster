import { classifyTurn, type TopicStreamStatus } from '@shared/ai/transport'

/**
 * The topic stream is the only "still working" fact. A missing status falls
 * back to the saved message row. A settled row on a live stream still waits,
 * but only on the latest turn, so older steps stay quiet.
 */
export function taskTurnPresentation(
  messagePending: boolean,
  stream: TopicStreamStatus | undefined,
  latest: boolean
): { streaming: boolean; awaiting: boolean; active: boolean } {
  if (stream === undefined) return { streaming: messagePending, awaiting: false, active: messagePending }
  const state = classifyTurn(stream)
  return {
    streaming: state.isStreamLive && messagePending,
    awaiting: latest && state.isStreamLive && !messagePending,
    active: latest && state.isTurnActive
  }
}
