import i18next from 'i18next'

import { dataApiService } from '@data/DataApiService'

import { codeBranchName, isCodeBranchName, type CodePlacement, type CodeSessionRecord } from './codeMode'
import { writePendingProject } from './WorkbenchProjectTray'

interface TauriInternals {
  invoke?: (command: string, args: Record<string, unknown>) => Promise<unknown>
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

function errorText(error: unknown): string {
  if (typeof error === 'string') return error.trim()
  if (error instanceof Error) return error.message.trim()
  return ''
}

export function codeWorkspaceErrorText(error: unknown): string {
  const detail = errorText(error)
  if (detail === 'need-project') {
    return i18next.t('workbench.composer.code.need_project', {
      defaultValue: '先选择一个项目文件夹，再创建工作树'
    })
  }
  if (detail === 'invalid-branch') {
    return i18next.t('workbench.composer.code.branch_invalid', {
      defaultValue: '分支名只能使用字母、数字、点、下划线、斜线和连字符'
    })
  }
  if (!detail || detail === 'worktree-failed' || detail === 'git-missing') {
    return i18next.t('workbench.composer.code.worktree_failed', { defaultValue: '无法创建工作树' })
  }
  return detail
}

/**
 * Creates the worktree and the agent workspace when placement is `worktree`.
 * `pending` stores the workspace for the next `homeTask.create`. Otherwise the
 * current session is rebound. Checkout leaves the pending project alone.
 */
export async function placeCodeSend(input: {
  placement: CodePlacement
  projectPath: string | null
  topic: string
  suffix: string
  branch?: string | null
  base?: string | null
  existing?: boolean
  pending: boolean
  bindCurrent?: (workspaceId: string) => Promise<void> | void
}): Promise<CodeSessionRecord> {
  if (input.placement === 'checkout') {
    return { cwd: input.projectPath ?? '', branch: null, placement: 'checkout', topic: input.topic }
  }
  if (!input.projectPath) throw new Error('need-project')
  const invoke = tauriInvoke()
  if (!invoke) throw new Error('worktree-failed')
  const named = input.branch?.trim() ?? ''
  const branch = input.existing ? named : named || codeBranchName(input.topic, input.suffix)
  const start = input.existing ? '' : (input.base?.trim() ?? '')
  if (!isCodeBranchName(branch) || (start && !isCodeBranchName(start))) throw new Error('invalid-branch')
  let created: { path?: string; branch?: string }
  try {
    created = (await invoke(
      'code_worktree_add',
      input.existing
        ? { cwd: input.projectPath, branch, existing: true }
        : start
          ? { cwd: input.projectPath, branch, start }
          : { cwd: input.projectPath, branch }
    )) as {
      path?: string
      branch?: string
    }
  } catch (error) {
    throw new Error(errorText(error) || 'worktree-failed')
  }
  if (!created.path || !created.branch) throw new Error('worktree-failed')
  let workspaceId = ''
  try {
    // The directory already exists. A failed record leaves it for the next send to report.
    const workspace = await dataApiService.post('/agent-workspaces', {
      body: { path: created.path, name: created.branch }
    })
    workspaceId = workspace.id
  } catch {
    // The directory stays on disk. The next send reports git's own error for this path.
    throw new Error(created.path)
  }
  if (!workspaceId) throw new Error('worktree-failed')
  if (input.pending) writePendingProject(workspaceId)
  else if (input.bindCurrent) await input.bindCurrent(workspaceId)
  else throw new Error('worktree-failed')
  return { cwd: created.path, branch: created.branch, placement: 'worktree', topic: input.topic }
}

export async function createCodePullRequest(input: {
  cwd: string
  branch: string | null
  title: string
  body: string
}): Promise<string> {
  const invoke = tauriInvoke()
  if (!invoke) throw new Error('gh-missing')
  const result = (await invoke('code_pull_request', {
    cwd: input.cwd,
    branch: input.branch,
    title: input.title,
    body: input.body
  })) as { url?: string }
  if (!result.url) throw new Error('pr-url-missing')
  return result.url
}
