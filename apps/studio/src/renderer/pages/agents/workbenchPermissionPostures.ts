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

/** One-turn instruction. The stored approval mode stays unchanged. */
export const PLAN_FIRST_DIRECTIVE =
  '本轮只做规划：可使用只读工具了解上下文，并使用 update_plan 建立或更新 2–8 个具体步骤。每个 step 使用简短的动作标题，详细要求和验收条件放在正文说明；尚未执行的步骤标记为 pending。向用户简洁说明计划与待确认的问题。不要修改项目文件，不要运行命令，不要开始执行。等待用户确认后再执行。'

export const EXECUTE_PLAN_DIRECTIVE =
  '按已确认的计划执行。使用 update_plan 持续更新完整清单：保留已经完成的步骤，将当前步骤标记为 in_progress，尚未执行的步骤保持 pending。验证结果后再标记 completed；失败、跳过或取消时如实记录状态。'

export function withPlanFirstDirectives(directives: readonly string[], enabled: boolean): string[] {
  if (!enabled) return [...directives]
  return [PLAN_FIRST_DIRECTIVE, ...directives].slice(0, 8)
}

function defaultStorage(): PendingStorage | null {
  try {
    return globalThis.localStorage ?? null
  } catch {
    return null
  }
}

function permissionModeKey(memberId: string | null) {
  const id = memberId === null ? 'local' : `member:${memberId}`
  return `${PERMISSION_MODE_PREFIX}${id}`
}

function readMode(storage: PendingStorage | null, key: string): AgentPermissionMode | null {
  const raw = storage?.getItem(key)
  if (!raw) return null
  return storedPermissionMode(raw)
}

/**
 * Last execution mode for this member on this machine.
 * The composer shows it immediately. The agent record is updated to match.
 * Legacy unscoped values belong only to the local scope.
 */
export function readPendingPermissionMode(
  storage = defaultStorage(),
  memberId: string | null = null
): AgentPermissionMode | null {
  try {
    if (memberId !== null && !MEMBER.test(memberId)) return null
    const key = permissionModeKey(memberId)
    const scoped = readMode(storage, key)
    if (scoped) return scoped
    if (memberId !== null) return null
    const fallbacks = [LEGACY_PERMISSION_MODE_KEY]
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
    if (memberId !== null && !MEMBER.test(memberId)) return
    const key = permissionModeKey(memberId)
    if (!mode) storage?.removeItem(key)
    else {
      const stored = storedPermissionMode(mode)
      if (stored) storage?.setItem(key, stored)
    }
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

function taskPermissionStorageChanged(event: StorageEvent) {
  if (event.storageArea && event.storageArea !== defaultStorage()) return
  if (
    event.key === null ||
    event.key === WORKBENCH_TASK_PERMISSION_KEY ||
    event.key.startsWith(`${WORKBENCH_TASK_PERMISSION_KEY}:`) ||
    event.key.startsWith(PERMISSION_MODE_PREFIX)
  )
    notifyTaskPermission()
}

/** The chip re-reads after a local command or another window changes its task choice. */
export function subscribeTaskPermission(listener: () => void) {
  if (taskPermissionListeners.size === 0) globalThis.addEventListener?.('storage', taskPermissionStorageChanged)
  taskPermissionListeners.add(listener)
  return () => {
    taskPermissionListeners.delete(listener)
    if (taskPermissionListeners.size === 0) globalThis.removeEventListener?.('storage', taskPermissionStorageChanged)
  }
}

export function readTaskPermissionRevision() {
  return taskPermissionRevision
}

export const WORKBENCH_TASK_PERMISSION_KEY = 'workbench.task.permission:v1'

export type TaskPermission = {
  mode: AgentPermissionMode
  planFirst?: boolean
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
    return { mode, planFirst: (entry as { planFirst?: unknown }).planFirst === true }
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
  const stored = readTaskPermission(sessionId, undefined, memberId)
  const base = displayedTaskPermissionMode(sessionId, memberId)
  const index = TASK_PERMISSION_CYCLE.indexOf(base)
  const next = TASK_PERMISSION_CYCLE[(index + 1) % TASK_PERMISSION_CYCLE.length] ?? 'default'
  writeTaskPermission(sessionId, { mode: next, planFirst: stored?.planFirst === true }, undefined, memberId)
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
    map[sessionId] = { mode, planFirst: next.planFirst === true }
    storage?.setItem(key, JSON.stringify(map))
    if (storage === defaultStorage() || storage == null) notifyTaskPermission()
  } catch {
    // Private mode or quota: the choice still lives in component state for this session.
  }
}
