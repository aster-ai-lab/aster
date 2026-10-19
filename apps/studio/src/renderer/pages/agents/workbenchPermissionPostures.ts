import type { AgentPermissionMode } from '@shared/data/api/schemas/agents'

/**
 * a3s-code permission postures (`a3s_code_core::permissions::policy_for_posture`), in the
 * order the a3s-code TUI cycles them. Each posture is stored on the agent as the Studio
 * permission mode it maps to; a3s-code has no `acceptEdits`, so that mode reads as `default`.
 */
export const A3S_CODE_POSTURES = ['default', 'plan', 'auto', 'always-approve'] as const
export type A3sCodePosture = (typeof A3S_CODE_POSTURES)[number]

const POSTURE_PERMISSION_MODE: Record<A3sCodePosture, AgentPermissionMode> = {
  default: 'default',
  plan: 'plan',
  auto: 'auto',
  'always-approve': 'bypassPermissions'
}

export function permissionModeForPosture(posture: A3sCodePosture): AgentPermissionMode {
  return POSTURE_PERMISSION_MODE[posture]
}

export function postureForPermissionMode(mode: string | undefined): A3sCodePosture {
  switch (mode) {
    case 'plan':
      return 'plan'
    case 'auto':
      return 'auto'
    case 'bypassPermissions':
      return 'always-approve'
    default:
      return 'default'
  }
}

export function isRiskyPosture(posture: A3sCodePosture): boolean {
  return posture === 'always-approve'
}

const LEGACY_PERMISSION_MODE_KEY = 'workbench.pending_permission_mode:v1'
const PERMISSION_MODE_PREFIX = 'workbench.composer.permission-mode:v1:'
const MEMBER = /^[A-Za-z0-9_-]{1,80}$/

type PendingStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>

function defaultStorage(): PendingStorage | null {
  try {
    return globalThis.localStorage ?? null
  } catch {
    return null
  }
}

function permissionModeKey(memberId: string | null) {
  const id = memberId && MEMBER.test(memberId) ? memberId : 'local'
  return `${PERMISSION_MODE_PREFIX}${id}`
}

function readMode(storage: PendingStorage | null, key: string): AgentPermissionMode | null {
  const raw = storage?.getItem(key)
  if (!raw) return null
  return permissionModeForPosture(postureForPermissionMode(raw))
}

/**
 * Last execution mode for this member on this machine.
 * The composer shows it immediately. The agent record is updated to match.
 * An older unscoped value is copied into the member key on first read.
 */
export function readPendingPermissionMode(
  storage = defaultStorage(),
  memberId: string | null = null
): AgentPermissionMode | null {
  try {
    const key = permissionModeKey(memberId)
    const scoped = readMode(storage, key)
    if (scoped) return scoped
    const fallbacks = memberId ? [permissionModeKey(null), LEGACY_PERMISSION_MODE_KEY] : [LEGACY_PERMISSION_MODE_KEY]
    for (const fallback of fallbacks) {
      const mode = readMode(storage, fallback)
      if (!mode) continue
      storage?.setItem(key, mode)
      return mode
    }
    return null
  } catch {
    return null
  }
}

export function writePendingPermissionMode(
  mode: string | null,
  storage = defaultStorage(),
  memberId: string | null = null
) {
  try {
    const key = permissionModeKey(memberId)
    if (mode) storage?.setItem(key, permissionModeForPosture(postureForPermissionMode(mode)))
    else storage?.removeItem(key)
  } catch {
    // Private mode or quota: the choice still lives in component state for this session.
  }
}

const TASK_PERMISSION_CYCLE: AgentPermissionMode[] = ['default', 'auto', 'bypassPermissions']
let taskPermissionRevision = 0
const taskPermissionListeners = new Set<() => void>()

function notifyTaskPermission() {
  taskPermissionRevision += 1
  for (const listener of taskPermissionListeners) listener()
}

/** The chip re-reads after the command palette cycles a mode. */
export function subscribeTaskPermission(listener: () => void) {
  taskPermissionListeners.add(listener)
  return () => taskPermissionListeners.delete(listener)
}

export function readTaskPermissionRevision() {
  return taskPermissionRevision
}

export const WORKBENCH_TASK_PERMISSION_KEY = 'workbench.task.permission:v1'

export type TaskPermission = {
  mode: AgentPermissionMode
}

/** Mode stored on one task. Plan stays plan. Unknown values are not stored as ask-every-time or full access. */
export function storedPermissionMode(mode: string | null | undefined): AgentPermissionMode | null {
  switch (mode) {
    case 'default':
    case 'plan':
    case 'auto':
    case 'bypassPermissions':
      return mode
    case 'acceptEdits':
      return 'default'
    case 'always-approve':
      return 'bypassPermissions'
    default:
      return null
  }
}

function taskPermissionKey(memberId: string | null): string | null {
  if (memberId === null) return WORKBENCH_TASK_PERMISSION_KEY
  return MEMBER.test(memberId) ? `${WORKBENCH_TASK_PERMISSION_KEY}:${memberId}` : null
}

function readTaskMap(storage: PendingStorage | null, memberId: string | null): Record<string, unknown> | null {
  const key = taskPermissionKey(memberId)
  if (!key) return null
  const raw = storage?.getItem(key)
  if (!raw) return {}
  try {
    const parsed = JSON.parse(raw) as unknown
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return null
    return parsed as Record<string, unknown>
  } catch {
    return null
  }
}

/** A home draft may show its member preference. A named task shows only its own record, otherwise ask-every-time. */
export function displayedTaskPermissionMode(
  sessionId: string,
  memberId: string | null = null,
  storage = defaultStorage()
): AgentPermissionMode {
  const id = sessionId?.trim() ? sessionId : 'draft'
  return (
    readTaskPermission(id, storage, memberId)?.mode ??
    (id === 'draft' ? readPendingPermissionMode(storage, memberId) : null) ??
    'default'
  )
}

/** Displayed mode for one task. Blank ids, bad JSON, and unknown modes return null. */
export function readTaskPermission(
  sessionId: string,
  storage = defaultStorage(),
  memberId: string | null = null
): TaskPermission | null {
  if (!sessionId?.trim()) return null
  try {
    const map = readTaskMap(storage, memberId)
    const entry = map?.[sessionId]
    if (!entry || typeof entry !== 'object') return null
    const mode = storedPermissionMode((entry as { mode?: unknown }).mode as string | undefined)
    if (!mode) return null
    return { mode }
  } catch {
    return null
  }
}

/**
 * Next approval for one task: ask-every-time, smart approval, full access.
 * Plan and unknown values step to ask-every-time. Does not write the agent.
 */
export function cycleTaskPermissionMode(sessionId: string, memberId: string | null = null): AgentPermissionMode | null {
  if (!sessionId?.trim()) return null
  const base = displayedTaskPermissionMode(sessionId, memberId)
  const index = TASK_PERMISSION_CYCLE.indexOf(base)
  const next = TASK_PERMISSION_CYCLE[(index + 1) % TASK_PERMISSION_CYCLE.length] ?? 'default'
  writeTaskPermission(sessionId, { mode: next }, undefined, memberId)
  if (sessionId === 'draft') writePendingPermissionMode(next, undefined, memberId)
  return next
}

export function writeTaskPermission(
  sessionId: string,
  next: TaskPermission,
  storage = defaultStorage(),
  memberId: string | null = null
) {
  if (!sessionId?.trim()) return
  const mode = storedPermissionMode(next.mode)
  if (!mode) return
  try {
    const key = taskPermissionKey(memberId)
    if (!key) return
    const map = readTaskMap(storage, memberId)
    if (!map) return
    map[sessionId] = { mode }
    storage?.setItem(key, JSON.stringify(map))
    if (storage === defaultStorage() || storage == null) notifyTaskPermission()
  } catch {
    // Private mode or quota: the choice still lives in component state for this session.
  }
}
