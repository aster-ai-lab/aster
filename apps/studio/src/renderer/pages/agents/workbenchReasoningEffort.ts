import type { ThinkingOption } from '@renderer/types/reasoning'

/**
 * Effort chosen in the model picker. The composer shows it on the next open.
 * An older unscoped value is copied into the member key on first read.
 */
const LEGACY_REASONING_EFFORT_KEY = 'workbench.pending_reasoning_effort:v1'
const REASONING_EFFORT_PREFIX = 'workbench.composer.reasoning-effort:v1:'
const MEMBER = /^[A-Za-z0-9_-]{1,80}$/

const EFFORTS: readonly ThinkingOption[] = [
  'default',
  'none',
  'minimal',
  'low',
  'medium',
  'high',
  'xhigh',
  'max',
  'ultra',
  'auto'
]

type PendingStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>

function defaultStorage(): PendingStorage | null {
  try {
    return globalThis.localStorage ?? null
  } catch {
    return null
  }
}

function isEffort(value: string): value is ThinkingOption {
  return (EFFORTS as readonly string[]).includes(value)
}

function reasoningEffortKey(memberId: string | null) {
  const id = memberId && MEMBER.test(memberId) ? memberId : 'local'
  return `${REASONING_EFFORT_PREFIX}${id}`
}

function readEffort(storage: PendingStorage | null, key: string): ThinkingOption | null {
  const raw = storage?.getItem(key)
  return raw && isEffort(raw) ? raw : null
}

export function readPendingReasoningEffort(
  storage = defaultStorage(),
  memberId: string | null = null
): ThinkingOption | null {
  try {
    const key = reasoningEffortKey(memberId)
    const scoped = readEffort(storage, key)
    if (scoped) return scoped
    const fallbacks = memberId ? [reasoningEffortKey(null), LEGACY_REASONING_EFFORT_KEY] : [LEGACY_REASONING_EFFORT_KEY]
    for (const fallback of fallbacks) {
      const effort = readEffort(storage, fallback)
      if (!effort) continue
      storage?.setItem(key, effort)
      return effort
    }
    return null
  } catch {
    return null
  }
}

export function writePendingReasoningEffort(
  effort: ThinkingOption | null,
  storage = defaultStorage(),
  memberId: string | null = null
) {
  try {
    const key = reasoningEffortKey(memberId)
    if (effort && isEffort(effort)) storage?.setItem(key, effort)
    else storage?.removeItem(key)
  } catch {
    // Private mode or quota: the choice still lives in component state for this session.
  }
}
