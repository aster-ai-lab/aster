import { extractTaskCodeDiff, isMutation, type TaskCodeDiff } from './taskConversation/taskDiff'

const STORAGE_KEY = 'workbench.code-session.v1'

export type CodePlacement = 'checkout' | 'worktree'

export interface CodeSessionRecord {
  cwd: string
  branch: string | null
  placement: CodePlacement
  topic: string
  pullRequestUrl?: string
}

export interface CodeTurn {
  role: 'user' | 'assistant'
  tools: readonly { name: string; part: unknown }[]
}

/** ASCII slug. Chinese topics become `code/task-<suffix>` because branch names stay ASCII. */
export function codeBranchName(topic: string, suffix: string): string {
  const words =
    topic
      .toLowerCase()
      .match(/[a-z0-9]+/g)
      ?.slice(0, 4)
      .join('-') ?? ''
  const slug = (words.length > 0 ? words : 'task').slice(0, 40).replace(/-+$/g, '') || 'task'
  const tail =
    suffix
      .toLowerCase()
      .replace(/[^a-z0-9]/g, '')
      .slice(0, 4) || 'task'
  return `code/${slug}-${tail}`
}

/** Same closed set as `code_git::validate_branch`: ASCII, no traversal, at most 100 characters. */
export function isCodeBranchName(value: string): boolean {
  const first = value.charCodeAt(0)
  const firstOk = (first >= 48 && first <= 57) || (first >= 65 && first <= 90) || (first >= 97 && first <= 122)
  if (!firstOk || value.length > 100 || value.includes('..')) return false
  for (const char of value) {
    const code = char.charCodeAt(0)
    const ok =
      (code >= 48 && code <= 57) ||
      (code >= 65 && code <= 90) ||
      (code >= 97 && code <= 122) ||
      char === '.' ||
      char === '_' ||
      char === '/' ||
      char === '-'
    if (!ok) return false
  }
  return true
}

/** Blank draft keeps the generated name. An illegal draft returns null and must not create a branch. */
export function resolveCodeBranch(topic: string, suffix: string, draft: string): string | null {
  const trimmed = draft.trim()
  if (!trimmed) return codeBranchName(topic, suffix)
  return isCodeBranchName(trimmed) ? trimmed : null
}

/**
 * Files mutated by tools in the latest assistant turn.
 * A bash `git pull` is not a mutation, so files it brought in stay out.
 * A later edit of the same path replaces the earlier one.
 */
export function latestTurnChanges(turns: readonly CodeTurn[]): TaskCodeDiff[] {
  let userAt = -1
  for (let index = turns.length - 1; index >= 0; index -= 1) {
    if (turns[index]?.role === 'user') {
      userAt = index
      break
    }
  }
  const byPath = new Map<string, TaskCodeDiff>()
  for (const turn of turns.slice(userAt + 1)) {
    if (turn.role !== 'assistant') continue
    for (const tool of turn.tools) {
      const name = tool.name.trim().toLowerCase()
      if (!isMutation(name)) continue
      const diff = extractTaskCodeDiff(name, tool.part)
      const path = diff?.path?.trim()
      if (!diff || !path) continue
      byPath.delete(path)
      byPath.set(path, diff)
    }
  }
  return [...byPath.values()]
}

function isSession(value: unknown): value is CodeSessionRecord {
  if (!value || typeof value !== 'object') return false
  const row = value as CodeSessionRecord
  if (row.placement !== 'checkout' && row.placement !== 'worktree') return false
  if (typeof row.cwd !== 'string' || typeof row.topic !== 'string') return false
  return row.branch === null || typeof row.branch === 'string'
}

function readAll(): Record<string, CodeSessionRecord> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed: unknown = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object') return {}
    const next: Record<string, CodeSessionRecord> = {}
    for (const [id, value] of Object.entries(parsed)) {
      if (isSession(value)) next[id] = value
    }
    return next
  } catch {
    return {}
  }
}

export function readCodeSession(sessionId: string): CodeSessionRecord | null {
  return readAll()[sessionId] ?? null
}

export function writeCodeSession(sessionId: string, record: CodeSessionRecord) {
  const all = readAll()
  all[sessionId] = record
  localStorage.setItem(STORAGE_KEY, JSON.stringify(all))
}

export function writeCodePullRequest(sessionId: string, url: string) {
  const current = readCodeSession(sessionId)
  if (!current || !url) return
  writeCodeSession(sessionId, { ...current, pullRequestUrl: url })
}
