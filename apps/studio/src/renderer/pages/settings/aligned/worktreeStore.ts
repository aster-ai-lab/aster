import { dataApiService } from '@data/DataApiService'
import { ipcApi } from '@renderer/ipc'
import type { GitWorktreeInput, GitWorktreeOutput, WorktreeRecord } from '@shared/git/worktreePorcelain'

export type { WorktreeRecord }

type TauriInternals = {
  invoke?: (command: string, args: { input: GitWorktreeInput }) => Promise<GitWorktreeOutput>
}

export async function loadOpenWorkspacePath(): Promise<string | null> {
  try {
    const latest = await dataApiService.get('/agent-sessions/latest')
    const session = latest.session
    if (!session || session.workspace?.type !== 'user' || !session.workspace.path) return null
    return session.workspace.path
  } catch {
    return null
  }
}

export async function requestWorktrees(input: GitWorktreeInput): Promise<GitWorktreeOutput> {
  if (window.api?.ipcApi) return ipcApi.request('app.git.worktrees', input)
  const internals = (globalThis as { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__
  if (typeof internals?.invoke === 'function') return internals.invoke('git_worktrees', { input })
  throw new Error('worktree-unavailable')
}
