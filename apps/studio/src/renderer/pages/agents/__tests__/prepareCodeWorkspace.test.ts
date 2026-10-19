import { beforeEach, describe, expect, it, vi } from 'vitest'

import { dataApiService } from '@data/DataApiService'

import { codeWorkspaceErrorText, placeCodeSend } from '../prepareCodeWorkspace'
import { readPendingProject } from '../WorkbenchProjectTray'

vi.mock('@data/DataApiService', () => ({
  dataApiService: { post: vi.fn() }
}))

const post = vi.mocked(dataApiService.post)
const invoke = vi.fn()

beforeEach(() => {
  post.mockReset()
  invoke.mockReset()
  localStorage.removeItem('workbench.pending_project:v1')
  ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
})

describe('placeCodeSend', () => {
  it('leaves the pending project alone on the current checkout', async () => {
    const record = await placeCodeSend({
      placement: 'checkout',
      projectPath: '/tmp/demo',
      topic: 'Fix login',
      suffix: 'ab12',
      pending: false
    })

    expect(record).toMatchObject({ cwd: '/tmp/demo', branch: null, placement: 'checkout' })
    expect(invoke).not.toHaveBeenCalled()
    expect(post).not.toHaveBeenCalled()
    expect(readPendingProject()).toBeNull()
  })

  it('refuses a worktree when no project folder is chosen', async () => {
    await expect(
      placeCodeSend({
        placement: 'worktree',
        projectPath: null,
        topic: 'Fix login',
        suffix: 'ab12',
        pending: true
      })
    ).rejects.toThrow('need-project')
    expect(invoke).not.toHaveBeenCalled()
  })

  it('names the directory when the workspace record cannot be saved', async () => {
    invoke.mockResolvedValue({ path: '/tmp/demo-code-fix-login-ab12', branch: 'code/fix-login-ab12' })
    post.mockRejectedValue(new Error('unavailable'))

    const failed = placeCodeSend({
      placement: 'worktree',
      projectPath: '/tmp/demo',
      topic: 'Fix login',
      suffix: 'ab12',
      pending: true
    })

    await expect(failed).rejects.toThrow('/tmp/demo-code-fix-login-ab12')
    await expect(failed.catch((error: unknown) => codeWorkspaceErrorText(error))).resolves.toBe(
      '/tmp/demo-code-fix-login-ab12'
    )
    expect(readPendingProject()).toBeNull()
  })

  it('stores the workspace id before the next task is created', async () => {
    invoke.mockResolvedValue({ path: '/tmp/demo-code-fix-login-ab12', branch: 'code/fix-login-ab12' })
    post.mockResolvedValue({ id: 'ws-1' })

    const record = await placeCodeSend({
      placement: 'worktree',
      projectPath: '/tmp/demo',
      topic: 'Fix login',
      suffix: 'ab12',
      pending: true
    })

    expect(invoke).toHaveBeenCalledWith('code_worktree_add', {
      cwd: '/tmp/demo',
      branch: 'code/fix-login-ab12'
    })
    expect(readPendingProject()).toBe('ws-1')
    expect(record).toMatchObject({
      cwd: '/tmp/demo-code-fix-login-ab12',
      branch: 'code/fix-login-ab12',
      placement: 'worktree'
    })
  })

  it('creates the named branch from the chosen base', async () => {
    invoke.mockResolvedValue({ path: '/tmp/demo-feature-billing', branch: 'feature/billing' })
    post.mockResolvedValue({ id: 'ws-2' })

    await placeCodeSend({
      placement: 'worktree',
      projectPath: '/tmp/demo',
      topic: 'Fix login',
      suffix: 'ab12',
      branch: 'feature/billing',
      base: 'main',
      pending: true
    })

    expect(invoke).toHaveBeenCalledWith('code_worktree_add', {
      cwd: '/tmp/demo',
      branch: 'feature/billing',
      start: 'main'
    })
  })

  it('checks out an existing branch instead of creating one', async () => {
    invoke.mockResolvedValue({ path: '/tmp/demo-release', branch: 'release' })
    post.mockResolvedValue({ id: 'ws-3' })

    await placeCodeSend({
      placement: 'worktree',
      projectPath: '/tmp/demo',
      topic: 'Fix login',
      suffix: 'ab12',
      branch: 'release',
      base: 'main',
      existing: true,
      pending: true
    })

    expect(invoke).toHaveBeenCalledWith('code_worktree_add', {
      cwd: '/tmp/demo',
      branch: 'release',
      existing: true
    })
  })

  it('does not create a worktree for an illegal branch name', async () => {
    await expect(
      placeCodeSend({
        placement: 'worktree',
        projectPath: '/tmp/demo',
        topic: 'Fix login',
        suffix: 'ab12',
        branch: 'bad..name',
        pending: true
      })
    ).rejects.toThrow('invalid-branch')
    expect(invoke).not.toHaveBeenCalled()
    expect(codeWorkspaceErrorText(new Error('invalid-branch'))).toBe(
      '分支名只能使用字母、数字、点、下划线、斜线和连字符'
    )
  })
})
