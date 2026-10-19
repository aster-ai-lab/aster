import { describe, expect, it } from 'vitest'

import { pathsShareDirectory, restoreWorktreeFiles, rewindEditedWorktree, worktreeFileRestoreAllowed } from './worktreeRestore'

const worktrees = [
  { path: '/repo', primary: true },
  { path: '/repo-feature', primary: false },
  { path: '/repo/child', primary: false }
]

describe('worktree path sharing', () => {
  it('does not treat a similarly named directory as a parent', () => {
    expect(pathsShareDirectory('/repo', '/repo-feature')).toBe(false)
    expect(pathsShareDirectory('/repo', '/repo/child')).toBe(true)
    expect(pathsShareDirectory('/repo/child', '/repo')).toBe(true)
    expect(pathsShareDirectory('/repo', '/repo')).toBe(true)
  })

  it('allows restore only for an exclusive non-primary worktree', () => {
    expect(
      worktreeFileRestoreAllowed({
        sessionPath: '/repo',
        worktrees,
        otherSessionPaths: []
      })
    ).toBe(false)
    expect(
      worktreeFileRestoreAllowed({
        sessionPath: '/repo-feature',
        worktrees,
        otherSessionPaths: ['/repo-feature/nested']
      })
    ).toBe(false)
    expect(
      worktreeFileRestoreAllowed({
        sessionPath: '/repo/child',
        worktrees,
        otherSessionPaths: ['/repo']
      })
    ).toBe(false)
    expect(
      worktreeFileRestoreAllowed({
        sessionPath: '/plain-project',
        worktrees,
        otherSessionPaths: []
      })
    ).toBe(false)
    expect(
      worktreeFileRestoreAllowed({
        sessionPath: '/repo-feature',
        worktrees,
        otherSessionPaths: ['/elsewhere']
      })
    ).toBe(true)
  })
})

describe('worktree file restore', () => {
  it('does not send when the git runner fails', async () => {
    const calls: string[][] = []
    const git = async (cwd: string, args: string[]) => {
      calls.push([cwd, ...args])
      throw new Error('restore failed')
    }
    const outcome = await rewindEditedWorktree({
      sessionPath: '/repo-feature',
      worktrees,
      otherSessionPaths: [],
      paths: ['src/a.ts'],
      git
    })
    expect(outcome).toBe('failed')
    expect(calls).toEqual([['/repo-feature', 'restore', '--source=HEAD', '--worktree', '--', 'src/a.ts']])
    await expect(restoreWorktreeFiles('/repo-feature', ['src/a.ts'], git)).rejects.toThrow('restore failed')
  })

  it('skips git when the directory is shared', async () => {
    const git = async () => {
      throw new Error('should not run')
    }
    const outcome = await rewindEditedWorktree({
      sessionPath: '/repo',
      worktrees,
      otherSessionPaths: [],
      paths: ['src/a.ts'],
      git
    })
    expect(outcome).toBe('skipped')
  })
})
