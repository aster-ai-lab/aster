import { describe, expect, it } from 'vitest'

import { parseWorktreePorcelain } from './worktreePorcelain'
import { runGitWorktrees, type GitRunner } from './worktrees'

const PORCELAIN = [
  'worktree /repo',
  'HEAD aaa',
  'branch refs/heads/main',
  '',
  'worktree /repo-feature',
  'HEAD bbb',
  'branch refs/heads/feature',
  '',
  'worktree /repo-clean',
  'HEAD ccc',
  'branch refs/heads/clean',
  ''
].join('\n')

function fakeGit(calls: string[][]): GitRunner {
  let added = false
  return async (cwd, args) => {
    calls.push([cwd, ...args])
    if (args[0] === 'worktree' && args[1] === 'list') {
      return added ? `${PORCELAIN}worktree /repo-topic\nHEAD ddd\nbranch refs/heads/topic\n` : PORCELAIN
    }
    if (args[0] === 'status') return cwd.endsWith('feature') ? ' M file\n' : ''
    if (args[0] === 'worktree' && args[1] === 'add') {
      added = true
      return ''
    }
    if (args[0] === 'worktree' && args[1] === 'remove') return ''
    return ''
  }
}

const mtime = async (target: string) => (target.endsWith('clean') ? 10 : 1_700_000_000_000)

describe('git worktrees', () => {
  it('parses porcelain paths and keeps the first checkout primary', async () => {
    expect(parseWorktreePorcelain(PORCELAIN).map((entry) => entry.path)).toEqual([
      '/repo',
      '/repo-feature',
      '/repo-clean'
    ])
    const calls: string[][] = []
    const listed = await runGitWorktrees({ cwd: '/repo', action: 'list' }, { git: fakeGit(calls), mtime })
    expect(listed.items).toEqual([
      { id: '/repo', path: '/repo', dirty: false, primary: true, mtimeMs: 1_700_000_000_000 },
      { id: '/repo-feature', path: '/repo-feature', dirty: true, primary: false, mtimeMs: 1_700_000_000_000 },
      { id: '/repo-clean', path: '/repo-clean', dirty: false, primary: false, mtimeMs: 10 }
    ])
  })

  it('refuses to remove a dirty worktree and does not run git worktree remove', async () => {
    const calls: string[][] = []
    const result = await runGitWorktrees(
      { cwd: '/repo', action: 'remove', path: '/repo-feature' },
      { git: fakeGit(calls), mtime }
    )
    expect(result.refused).toBe('dirty')
    expect(result.items.some((item) => item.path === '/repo-feature')).toBe(true)
    expect(calls.some((call) => call.includes('remove'))).toBe(false)
  })

  it('refreshes the list before creating a worktree', async () => {
    const calls: string[][] = []
    const result = await runGitWorktrees(
      { cwd: '/repo', action: 'create', branch: 'topic' },
      { git: fakeGit(calls), mtime }
    )
    const listAt = calls.findIndex((call) => call[1] === 'worktree' && call[2] === 'list')
    const addAt = calls.findIndex((call) => call[1] === 'worktree' && call[2] === 'add')
    expect(listAt).toBeGreaterThanOrEqual(0)
    expect(addAt).toBeGreaterThan(listAt)
    expect(calls[addAt]).toEqual(['/repo', 'worktree', 'add', '-b', 'topic', '/repo-topic'])
    expect(result.items.some((item) => item.path === '/repo-topic')).toBe(true)
  })

  it('restores named files from HEAD in the session directory', async () => {
    const calls: string[][] = []
    await runGitWorktrees(
      { cwd: '/repo-feature', action: 'restore', paths: ['src/a.ts'] },
      { git: fakeGit(calls), mtime }
    )
    expect(calls).toContainEqual(['/repo-feature', 'restore', '--source=HEAD', '--worktree', '--', 'src/a.ts'])
  })

  it('clears only clean non-primary worktrees inside the time window', async () => {
    const calls: string[][] = []
    await runGitWorktrees({ cwd: '/repo', action: 'clear', sinceMs: 0 }, { git: fakeGit(calls), mtime })
    const removed = calls.filter((call) => call[1] === 'worktree' && call[2] === 'remove')
    expect(removed).toEqual([['/repo', 'worktree', 'remove', '/repo-clean']])
  })
})
