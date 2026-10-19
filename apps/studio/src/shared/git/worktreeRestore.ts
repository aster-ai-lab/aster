export type WorktreeRestoreGit = (cwd: string, args: string[]) => Promise<string>

export function normalizePathSegments(input: string): string[] {
  const text = input.replace(/\\/g, '/').replace(/\/+$/g, '')
  const absolute = text.startsWith('/')
  const parts: string[] = []
  for (const raw of text.split('/')) {
    if (!raw || raw === '.') continue
    if (raw === '..') {
      if (parts.length > 0 && parts[parts.length - 1] !== '..') parts.pop()
      else if (!absolute) parts.push('..')
      continue
    }
    parts.push(raw)
  }
  return parts
}

function segmentKey(input: string): string[] {
  return normalizePathSegments(input).map((segment) => segment.toLowerCase())
}

/** True when the paths are the same directory or one contains the other. Segment compare, so `/repo` does not contain `/repo-feature`. */
export function pathsShareDirectory(left: string, right: string): boolean {
  const a = segmentKey(left)
  const b = segmentKey(right)
  if (a.length === 0 || b.length === 0) return false
  const length = Math.min(a.length, b.length)
  for (let index = 0; index < length; index += 1) {
    if (a[index] !== b[index]) return false
  }
  return true
}

export function worktreeFileRestoreAllowed(input: {
  sessionPath: string
  worktrees: readonly { path: string; primary: boolean }[]
  otherSessionPaths: readonly string[]
}): boolean {
  const sessionKey = segmentKey(input.sessionPath).join('/')
  if (!sessionKey) return false
  const match = input.worktrees.find((item) => segmentKey(item.path).join('/') === sessionKey)
  if (!match || match.primary) return false
  return input.otherSessionPaths.every((path) => !pathsShareDirectory(input.sessionPath, path))
}

export async function restoreWorktreeFiles(
  cwd: string,
  paths: readonly string[],
  git: WorktreeRestoreGit
): Promise<void> {
  const files = [...new Set(paths.map((file) => file.trim()).filter(Boolean))]
  if (files.length === 0) return
  if (files.some((file) => file.includes('\0') || file.includes('\n'))) throw new Error('invalid-path')
  await git(cwd, ['restore', '--source=HEAD', '--worktree', '--', ...files])
}

export async function rewindEditedWorktree(input: {
  sessionPath: string
  worktrees: readonly { path: string; primary: boolean }[]
  otherSessionPaths: readonly string[]
  paths: readonly string[]
  git: WorktreeRestoreGit
}): Promise<'skipped' | 'restored' | 'failed'> {
  if (!worktreeFileRestoreAllowed(input)) return 'skipped'
  try {
    await restoreWorktreeFiles(input.sessionPath, input.paths, input.git)
    return 'restored'
  } catch {
    return 'failed'
  }
}
