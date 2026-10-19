import { execFile } from 'node:child_process'
import { stat } from 'node:fs/promises'
import path from 'node:path'

import {
  parseWorktreePorcelain,
  type GitWorktreeInput,
  type GitWorktreeOutput,
  type WorktreeRecord
} from './worktreePorcelain'

export type GitRunner = (cwd: string, args: string[]) => Promise<string>
export type MtimeReader = (target: string) => Promise<number>

const BRANCH_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._/-]{0,99}$/

function defaultGit(cwd: string, args: string[]): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile('git', args, { cwd, timeout: 20000, maxBuffer: 1_000_000 }, (error, stdout, stderr) => {
      if (error) {
        const detail = stderr?.toString().trim()
        reject(new Error(detail || error.message))
        return
      }
      resolve(stdout.toString())
    })
  })
}

async function defaultMtime(target: string): Promise<number> {
  const info = await stat(target)
  return info.mtimeMs
}

function assertCwd(cwd: string): string | null {
  if (!path.isAbsolute(cwd) || cwd.includes('\0') || cwd.includes('\n')) return 'invalid-cwd'
  return null
}

function assertBranch(branch: string | undefined): string | null {
  if (!branch || !BRANCH_PATTERN.test(branch) || branch.includes('..')) return 'invalid-branch'
  return null
}

async function listWorktrees(cwd: string, git: GitRunner, mtime: MtimeReader): Promise<WorktreeRecord[]> {
  const stdout = await git(cwd, ['worktree', 'list', '--porcelain'])
  const parsed = parseWorktreePorcelain(stdout)
  const items: WorktreeRecord[] = []
  for (const [index, entry] of parsed.entries()) {
    let dirty = true
    try {
      const status = await git(entry.path, ['status', '--porcelain'])
      dirty = status.trim().length > 0
    } catch {
      dirty = true
    }
    let mtimeMs = 0
    try {
      mtimeMs = await mtime(entry.path)
    } catch {
      mtimeMs = 0
    }
    items.push({
      id: entry.path,
      path: entry.path,
      dirty,
      primary: index === 0,
      mtimeMs
    })
  }
  return items
}

/**
 * Lists, removes, clears, restores, or creates git worktrees for one open project.
 * Create and clear refresh the list before they change anything.
 */
export async function runGitWorktrees(
  input: GitWorktreeInput,
  deps: { git?: GitRunner; mtime?: MtimeReader } = {}
): Promise<GitWorktreeOutput> {
  const git = deps.git ?? defaultGit
  const mtime = deps.mtime ?? defaultMtime
  const invalid = assertCwd(input.cwd)
  if (invalid) return { items: [], error: invalid }

  const list = () => listWorktrees(input.cwd, git, mtime)

  try {
    if (input.action === 'list') {
      return { items: await list() }
    }

    if (input.action === 'restore') {
      const paths = input.paths ?? []
      if (paths.some((file) => file.includes('\0') || file.includes('\n'))) {
        return { items: await list(), error: 'invalid-path' }
      }
      if (paths.length > 0) {
        await git(input.cwd, ['restore', '--source=HEAD', '--worktree', '--', ...paths])
      }
      return { items: await list() }
    }

    if (input.action === 'remove') {
      const items = await list()
      const target = items.find((item) => item.path === input.path || item.id === input.path)
      if (!target) return { items, refused: 'missing' }
      if (target.primary) return { items, refused: 'primary' }
      if (target.dirty) return { items, refused: 'dirty' }
      try {
        await git(input.cwd, ['worktree', 'remove', target.path])
      } catch {
        return { items, refused: 'dirty' }
      }
      return { items: await list() }
    }

    if (input.action === 'clear') {
      const items = await list()
      const sinceMs = input.sinceMs ?? 0
      for (const item of items) {
        if (item.primary || item.dirty || item.mtimeMs < sinceMs) continue
        try {
          await git(input.cwd, ['worktree', 'remove', item.path])
        } catch {
          // A locked tree stays in the refreshed list.
        }
      }
      return { items: await list() }
    }

    const branchError = assertBranch(input.branch)
    if (branchError || !input.branch) return { items: await list(), error: branchError ?? 'invalid-branch' }
    await list()
    const dest = path.join(
      path.dirname(input.cwd),
      `${path.basename(input.cwd)}-${input.branch.replace(/[\\/]/g, '-')}`
    )
    await git(input.cwd, ['worktree', 'add', '-b', input.branch, dest])
    return { items: await list() }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'git-failed'
    return { items: [], error: message }
  }
}
