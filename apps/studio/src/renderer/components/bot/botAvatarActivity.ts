import type { TopicStreamStatus } from '@shared/ai/transport'
import type { MessageStatus } from '@shared/data/types/message'

import type { A3SBotAvatarActivity } from './bot-avatar-character'

/**
 * Live topic stream is the work state. A missing entry means that task has
 * settled, so callers rest on the completed face.
 */
export function botActivityForStreamStatus(status: TopicStreamStatus | undefined): A3SBotAvatarActivity | undefined {
  switch (status) {
    case 'pending':
    case 'streaming':
      return 'working'
    case 'awaiting-approval':
    case 'awaiting-form':
    case 'aborted':
      return 'paused'
    case 'error':
      return 'failed'
    case 'done':
      return 'completed'
    default:
      return undefined
  }
}

export function botActivityForMessageStatus(status: MessageStatus | undefined): A3SBotAvatarActivity {
  if (status === 'pending') return 'working'
  if (status === 'error') return 'failed'
  if (status === 'paused') return 'paused'
  return 'completed'
}

/** Latest assistant row. The stream wins; a restored row falls back to its saved status. */
export function latestTurnBotActivity(
  stream: TopicStreamStatus | undefined,
  messageStatus: MessageStatus | undefined
): A3SBotAvatarActivity {
  return botActivityForStreamStatus(stream) ?? botActivityForMessageStatus(messageStatus)
}

export function taskListBotActivity(stream: TopicStreamStatus | undefined): A3SBotAvatarActivity {
  return botActivityForStreamStatus(stream) ?? 'completed'
}

/** A running task keeps moving in the list. The open task plays its pose. Settled rows stay still. */
export function taskListBotAnimated(activity: A3SBotAvatarActivity, current: boolean): boolean {
  return activity === 'working' || current
}
