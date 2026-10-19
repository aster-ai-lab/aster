export type WorktreeRecord = {
  id: string
  path: string
  dirty: boolean
  primary: boolean
  mtimeMs: number
}

export type GitWorktreeAction = 'list' | 'remove' | 'clear' | 'create' | 'restore'

export type GitWorktreeInput = {
  cwd: string
  action: GitWorktreeAction
  path?: string
  paths?: string[]
  branch?: string
  sinceMs?: number
}

export type GitWorktreeOutput = {
  items: WorktreeRecord[]
  error?: string
  refused?: 'dirty' | 'missing' | 'primary'
}

export type WorktreeTimeFilter = 'all' | '7d' | '30d'

const DAY_MS = 24 * 60 * 60 * 1000

export function worktreeSinceMs(filter: WorktreeTimeFilter, now = Date.now()): number {
  if (filter === '7d') return now - 7 * DAY_MS
  if (filter === '30d') return now - 30 * DAY_MS
  return 0
}

export function filterWorktreesByTime(items: readonly WorktreeRecord[], sinceMs: number): WorktreeRecord[] {
  if (sinceMs <= 0) return items.map((item) => ({ ...item }))
  return items.filter((item) => item.mtimeMs >= sinceMs).map((item) => ({ ...item }))
}

/** Parses `git worktree list --porcelain`. The first worktree is the primary checkout. */
export function parseWorktreePorcelain(stdout: string): { path: string }[] {
  const paths: { path: string }[] = []
  for (const line of stdout.split('\n')) {
    if (!line.startsWith('worktree ')) continue
    const worktreePath = line.slice('worktree '.length).trim()
    if (worktreePath) paths.push({ path: worktreePath })
  }
  return paths
}
