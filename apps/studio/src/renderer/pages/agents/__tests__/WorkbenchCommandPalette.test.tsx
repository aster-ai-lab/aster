import { act, cleanup, fireEvent, render } from '@testing-library/react'
import { type ComponentProps } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CommandProvider } from '@renderer/components/command/CommandProvider'

import { registerWorkbenchComposer } from '../workbenchComposerBridge'
import { WorkbenchCommandPalette } from '../WorkbenchCommandPalette'
import { WORKBENCH_SETTLED_SESSIONS_KEY, WorkbenchHome, type WorkbenchSession } from '../WorkbenchHome'
import {
  readPendingPermissionMode,
  readTaskPermission,
  WORKBENCH_TASK_PERMISSION_KEY,
  writePendingPermissionMode
} from '../workbenchPermissionPostures'

const updateAgent = vi.hoisted(() => vi.fn(async () => undefined))

vi.mock('@renderer/hooks/agent/useAgent', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@renderer/hooks/agent/useAgent')>()
  return { ...actual, useUpdateAgent: () => ({ updateAgent, updateModel: vi.fn() }) }
})

function session(id: string): WorkbenchSession {
  return { id, name: id, updatedAt: '2026-10-18T00:00:00.000Z', pinned: false }
}

function renderHome(overrides: Partial<ComponentProps<typeof WorkbenchHome>> = {}) {
  const props = {
    sessions: [] as WorkbenchSession[],
    activeSessionId: null,
    homeActive: true,
    onNewTask: vi.fn(),
    onOpenSession: vi.fn(),
    onTogglePin: vi.fn(),
    onDeleteSession: vi.fn(),
    onRenameSession: vi.fn(),
    ...overrides
  }
  render(
    <CommandProvider>
      <WorkbenchHome {...props} />
    </CommandProvider>
  )
  return props
}

function item(id: string): HTMLButtonElement {
  const node = document.querySelector(`[data-ui="workbench.command.${id}"]`)
  if (!(node instanceof HTMLButtonElement)) throw new Error(`missing command ${id}`)
  return node
}

function openPalette() {
  act(() => {
    window.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'K',
        code: 'KeyK',
        ctrlKey: true,
        shiftKey: true,
        bubbles: true,
        cancelable: true
      })
    )
  })
}

describe('WorkbenchCommandPalette', () => {
  afterEach(() => {
    cleanup()
  })

  beforeEach(() => {
    updateAgent.mockClear()
    localStorage.removeItem(WORKBENCH_TASK_PERMISSION_KEY)
    localStorage.removeItem(WORKBENCH_SETTLED_SESSIONS_KEY)
    localStorage.removeItem('workbench.composer.permission-mode:v1:local')
    localStorage.removeItem('workbench.pending_permission_mode:v1')
  })

  it('lists the four actions and leaves settle unavailable on the home canvas', () => {
    const onNewTask = vi.fn()
    const onSettle = vi.fn()
    const onFocus = vi.fn()
    const onCycleMode = vi.fn()
    const onOpenChange = vi.fn()
    render(
      <WorkbenchCommandPalette
        open
        onOpenChange={onOpenChange}
        canSettle={false}
        onNewTask={onNewTask}
        onSettle={onSettle}
        onFocus={onFocus}
        onCycleMode={onCycleMode}
      />
    )

    const settle = item('settle')
    expect(settle).toBeDisabled()
    fireEvent.click(settle)
    expect(onSettle).not.toHaveBeenCalled()
    expect(onOpenChange).not.toHaveBeenCalled()

    fireEvent.click(item('new-task'))
    expect(onNewTask).toHaveBeenCalledOnce()
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it('opens from Shift+K, not from Ctrl+Shift+P, and runs the current task actions', () => {
    const focus = vi.fn()
    const unregister = registerWorkbenchComposer({ fill: vi.fn(), focus })
    const props = renderHome({
      homeActive: false,
      activeSessionId: 'today',
      sessions: [session('today')],
      useLiveComposer: true
    })

    act(() => {
      window.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'P',
          code: 'KeyP',
          ctrlKey: true,
          shiftKey: true,
          bubbles: true,
          cancelable: true
        })
      )
    })
    expect(document.querySelector('[data-ui="workbench.command-palette"]')).toBeNull()

    openPalette()
    expect(document.querySelector('[data-ui="workbench.command-palette"]')).toBeTruthy()
    fireEvent.click(item('new-task'))
    expect(props.onNewTask).toHaveBeenCalledOnce()
    expect(document.querySelector('[data-ui="workbench.command-palette"]')).toBeNull()

    focus.mockClear()
    openPalette()
    fireEvent.click(item('focus'))
    expect(focus).toHaveBeenCalledOnce()

    openPalette()
    fireEvent.click(item('settle'))
    expect(document.querySelector('[data-ui="workbench.session.settle.undo"]')).toBeTruthy()
    expect(JSON.parse(localStorage.getItem(WORKBENCH_SETTLED_SESSIONS_KEY) ?? '[]')).toEqual(['today'])
    fireEvent.click(document.querySelector('[data-ui="workbench.session.settle.undo"] button')!)
    expect(document.querySelector('[data-ui="workbench.session.settle.undo"]')).toBeNull()

    openPalette()
    fireEvent.click(item('cycle'))
    expect(readTaskPermission('today')).toEqual({ mode: 'auto' })
    expect(readPendingPermissionMode()).toBeNull()
    expect(updateAgent).not.toHaveBeenCalled()
    unregister()
  })

  it('leaves settle unavailable when the current task is already ended', () => {
    localStorage.setItem(WORKBENCH_SETTLED_SESSIONS_KEY, JSON.stringify(['today']))
    renderHome({ homeActive: false, activeSessionId: 'today', sessions: [session('today')] })
    openPalette()
    expect(item('settle')).toBeDisabled()
  })

  it('cycles the draft on the home canvas and updates the chip', () => {
    renderHome()
    openPalette()
    fireEvent.click(item('cycle'))
    expect(readTaskPermission('draft')).toEqual({ mode: 'auto' })
    expect(readPendingPermissionMode()).toBe('auto')
    expect(document.querySelector('[data-workbench-chip="authorize"]')).toHaveAttribute('data-posture', 'auto')
    expect(updateAgent).not.toHaveBeenCalled()
  })

  it('新任务后草稿芯片不是上一条的模式', () => {
    localStorage.setItem(WORKBENCH_TASK_PERMISSION_KEY, JSON.stringify({ 'task-a': { mode: 'bypassPermissions' } }))
    writePendingPermissionMode('auto')
    const props = renderHome({ sessions: [session('task-a')], activeSessionId: 'task-a' })
    const chip = () => document.querySelector('[data-workbench-chip="authorize"]')
    expect(chip()).toHaveAttribute('data-posture', 'auto')
    fireEvent.click(document.querySelector('[data-new-task-trigger="true"]')!)
    expect(props.onNewTask).toHaveBeenCalledOnce()
    expect(chip()).toHaveAttribute('data-posture', 'auto')
    expect(readTaskPermission('task-a')?.mode).toBe('bypassPermissions')
    expect(readPendingPermissionMode()).toBe('auto')
  })
})
