import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, expect, it, vi } from 'vitest'

import { WorkbenchWorktreeSlot } from '../WorkbenchWorktreeSlot'

const requestWorktrees = vi.hoisted(() => vi.fn())
const post = vi.hoisted(() => vi.fn())

vi.mock('@renderer/pages/settings/aligned/worktreeStore', () => ({
  requestWorktrees
}))

vi.mock('@data/DataApiService', () => ({
  dataApiService: { post }
}))

const primary = { id: '/repo', path: '/repo', dirty: false, primary: true, mtimeMs: 1 }
const clean = { id: '/repo-feature', path: '/repo-feature', dirty: false, primary: false, mtimeMs: 2 }
const dirty = { id: '/repo-wip', path: '/repo-wip', dirty: true, primary: false, mtimeMs: 3 }

function renderSlot(hasMessages: boolean, onSend = vi.fn(), onWorkspaceChange = vi.fn()) {
  const view = render(
    <>
      <WorkbenchWorktreeSlot cwd="/repo" hasMessages={hasMessages} onWorkspaceChange={onWorkspaceChange} />
      <button type="button" onClick={onSend}>
        发送
      </button>
    </>
  )
  return { ...view, onSend, onWorkspaceChange }
}

beforeEach(() => {
  requestWorktrees.mockReset()
  post.mockReset()
  requestWorktrees.mockResolvedValue({ items: [primary, clean, dirty] })
  post.mockResolvedValue({ id: 'workspace-feature' })
})

it('lets an empty task pick a clean worktree and leaves a dirty one out', async () => {
  const { onWorkspaceChange } = renderSlot(false)
  const pick = await screen.findByRole('combobox', { name: '工作树' })
  fireEvent.click(pick)
  expect(screen.getByRole('option', { name: '/repo-feature' })).toBeInTheDocument()
  expect(screen.queryByRole('option', { name: '/repo' })).not.toBeInTheDocument()
  expect(screen.queryByRole('option', { name: '/repo-wip' })).not.toBeInTheDocument()
  fireEvent.click(screen.getByRole('option', { name: '/repo-feature' }))
  await waitFor(() => expect(onWorkspaceChange).toHaveBeenCalledWith('workspace-feature'))
  expect(post).toHaveBeenCalledWith('/agent-workspaces', { body: { path: '/repo-feature' } })
  expect(requestWorktrees).toHaveBeenCalledWith({ cwd: '/repo', action: 'list' })
  expect(requestWorktrees.mock.calls.some((call) => call[0]?.action === 'create')).toBe(false)
})

it('hides the control once the task has a message', async () => {
  renderSlot(true)
  await waitFor(() => expect(requestWorktrees).not.toHaveBeenCalled())
  expect(document.querySelector('[data-ui="workbench.worktree"]')).toBeNull()
})

it('still sends when the project has only the primary checkout', async () => {
  requestWorktrees.mockResolvedValue({ items: [primary] })
  const { onSend } = renderSlot(false)
  await waitFor(() => expect(requestWorktrees).toHaveBeenCalled())
  expect(document.querySelector('[data-ui="workbench.worktree"]')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: '发送' }))
  expect(onSend).toHaveBeenCalledOnce()
})

it('still sends when the worktree list fails', async () => {
  requestWorktrees.mockRejectedValue(new Error('git failed'))
  const { onSend, onWorkspaceChange } = renderSlot(false)
  await waitFor(() => expect(requestWorktrees).toHaveBeenCalled())
  expect(document.querySelector('[data-ui="workbench.worktree"]')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: '发送' }))
  expect(onSend).toHaveBeenCalledOnce()
  expect(onWorkspaceChange).not.toHaveBeenCalled()
})
