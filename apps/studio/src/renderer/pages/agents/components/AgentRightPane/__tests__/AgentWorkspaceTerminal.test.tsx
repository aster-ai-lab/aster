import { act, render } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AgentWorkspaceTerminal } from '../AgentWorkspaceTerminal'

const invoke = vi.fn(async (command: string) => {
  if (command === 'workspace_terminal_create') return { sessionId: 'pty-1' }
  if (command === 'workspace_terminal_drain') return { bytes: [] }
  return {}
})

describe('AgentWorkspaceTerminal', () => {
  afterEach(() => {
    invoke.mockClear()
    delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  it('keeps the host shell when the session is hidden and kills it on unmount', async () => {
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    const view = render(<AgentWorkspaceTerminal active={false} cwd="/tmp/workspace" label="终端" sessionId="task-1" />)

    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(invoke).toHaveBeenCalledWith('workspace_terminal_create', {
      cols: 80,
      rows: 24,
      cwd: '/tmp/workspace'
    })
    expect(invoke).not.toHaveBeenCalledWith('workspace_terminal_kill', expect.anything())

    invoke.mockClear()
    view.rerender(<AgentWorkspaceTerminal active cwd="/tmp/workspace" label="终端" sessionId="task-1" />)

    await act(async () => {
      await Promise.resolve()
    })

    expect(invoke).not.toHaveBeenCalledWith('workspace_terminal_kill', expect.anything())
    expect(invoke).not.toHaveBeenCalledWith('workspace_terminal_create', expect.anything())

    view.unmount()

    expect(invoke).toHaveBeenCalledWith('workspace_terminal_kill', { sessionId: 'pty-1' })
  })
})
