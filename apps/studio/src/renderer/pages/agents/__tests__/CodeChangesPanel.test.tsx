import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CodeChangesPanel } from '../CodeChangesPanel'
import { writeCodeSession } from '../codeMode'
import type { TaskCodeDiff } from '../taskConversation/taskDiff'

const { createCodePullRequest } = vi.hoisted(() => ({
  createCodePullRequest: vi.fn()
}))

vi.mock('../prepareCodeWorkspace', () => ({
  createCodePullRequest
}))

afterEach(() => {
  localStorage.removeItem('workbench.code-session.v1')
})

function diff(path: string): TaskCodeDiff {
  return {
    path,
    operation: 'edit',
    lines: [{ kind: 'addition', text: `line ${path}`, newLine: 1 }],
    additions: 1,
    deletions: 0,
    comparisonComplete: true,
    truncated: false
  }
}

describe('CodeChangesPanel', () => {
  it('opens the first file and switches the diff when another row is chosen', () => {
    render(<CodeChangesPanel diffs={[diff('src/a.ts'), diff('src/b.ts')]} />)

    const row = screen.getByRole('button', { name: /a\.ts/ })
    const region = screen.getByRole('region', { name: '代码修改 · a.ts' })
    expect(row.className).toContain('h-9')
    expect(row.className).toContain('bg-[#f2f3f5]')
    const columns = row.closest('ul')?.parentElement
    expect(columns?.className).toContain('flex')
    expect(columns?.className).not.toContain('flex-col')
    expect(region.parentElement?.parentElement).toBe(columns)
    expect(screen.queryByRole('region', { name: '代码修改 · b.ts' })).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: /b\.ts/ }))

    expect(screen.getByRole('region', { name: '代码修改 · b.ts' })).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: '代码修改 · a.ts' })).toBeNull()
  })

  it('keeps a diff path that is still present selected on the next render', () => {
    const view = render(<CodeChangesPanel diffs={[diff('src/a.ts'), diff('src/b.ts')]} />)
    fireEvent.click(screen.getByRole('button', { name: /b\.ts/ }))

    view.rerender(<CodeChangesPanel diffs={[diff('src/b.ts'), diff('src/c.ts')]} />)

    expect(screen.getByRole('region', { name: '代码修改 · b.ts' })).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: '代码修改 · a.ts' })).toBeNull()
    expect(screen.queryByRole('region', { name: '代码修改 · c.ts' })).toBeNull()
    expect(screen.getByRole('button', { name: /b\.ts/ })).toHaveAttribute('aria-current', 'true')
  })

  it('shows 这一回合还没有改文件 when the diff list is empty', () => {
    render(<CodeChangesPanel diffs={[]} />)
    expect(screen.getByText('这一回合还没有改文件')).toBeInTheDocument()
  })

  it('returns to the first file when the chosen path leaves the turn', () => {
    const view = render(<CodeChangesPanel diffs={[diff('src/a.ts'), diff('src/b.ts')]} />)
    fireEvent.click(screen.getByRole('button', { name: /b\.ts/ }))

    view.rerender(<CodeChangesPanel diffs={[diff('src/c.ts'), diff('src/d.ts')]} />)

    expect(screen.getByRole('region', { name: '代码修改 · c.ts' })).toBeInTheDocument()
    expect(screen.queryByRole('region', { name: '代码修改 · b.ts' })).toBeNull()
  })

  it('shows the working directory and keeps the pull request disabled without one', () => {
    const { rerender } = render(<CodeChangesPanel sessionId="session-code" diffs={[]} />)
    expect(screen.getByRole('button', { name: '创建拉取请求' })).toBeDisabled()

    writeCodeSession('session-code', {
      cwd: '/tmp/demo',
      branch: 'code/fix-login-ab12',
      placement: 'worktree',
      topic: 'Fix login'
    })
    rerender(<CodeChangesPanel sessionId="session-code" diffs={[]} />)

    expect(screen.getByTitle('/tmp/demo')).toHaveTextContent('/tmp/demo')
    expect(screen.getByLabelText('/tmp/demo')).toHaveTextContent('/tmp/demo')
    expect(screen.getByText('code/fix-login-ab12')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '创建拉取请求' })).toBeEnabled()
  })

  it('creates the pull request only after the confirm', async () => {
    createCodePullRequest.mockReset()
    createCodePullRequest.mockResolvedValue('https://github.com/acme/repo/pull/7')
    writeCodeSession('session-code', {
      cwd: '/tmp/demo',
      branch: 'code/fix-login-ab12',
      placement: 'worktree',
      topic: 'Fix login'
    })
    render(<CodeChangesPanel sessionId="session-code" diffs={[diff('src/a.ts')]} />)

    fireEvent.click(screen.getByRole('button', { name: '创建拉取请求' }))
    expect(createCodePullRequest).not.toHaveBeenCalled()
    const dialog = screen.getByRole('dialog', { name: '创建拉取请求' })
    expect(dialog).toHaveTextContent('将当前分支推送到 origin，并在 GitHub 上创建拉取请求')

    fireEvent.click(within(dialog).getByRole('button', { name: '取消' }))
    expect(createCodePullRequest).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: '创建拉取请求' }))
    fireEvent.click(within(screen.getByRole('dialog', { name: '创建拉取请求' })).getByRole('button', { name: '确认' }))

    expect(createCodePullRequest).toHaveBeenCalledTimes(1)
    expect(createCodePullRequest).toHaveBeenCalledWith({
      cwd: '/tmp/demo',
      branch: 'code/fix-login-ab12',
      title: 'Fix login',
      body: 'src/a.ts'
    })
    await waitFor(() =>
      expect(screen.getByRole('link', { name: '拉取请求' })).toHaveAttribute(
        'href',
        'https://github.com/acme/repo/pull/7'
      )
    )
  })
})
