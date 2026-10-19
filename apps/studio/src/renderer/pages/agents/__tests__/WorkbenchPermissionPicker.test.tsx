import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { act, type ReactNode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('@a3s-lab/ui', () => {
  const React = require('react') as typeof import('react')
  const MenuContext = React.createContext({
    open: false,
    onOpenChange: (_open: boolean) => undefined
  })
  return {
    DropdownMenu: ({
      children,
      open = false,
      onOpenChange
    }: {
      children?: ReactNode
      open?: boolean
      onOpenChange?: (open: boolean) => void
    }) =>
      React.createElement(
        MenuContext.Provider,
        { value: { open, onOpenChange: onOpenChange ?? (() => undefined) } },
        children
      ),
    DropdownMenuTrigger: ({ children }: { children?: ReactNode }) => {
      const menu = React.use(MenuContext)
      return React.createElement(
        'span',
        {
          onPointerDown: (event: React.PointerEvent) => {
            if (event.button === 0 && !event.ctrlKey) menu.onOpenChange(true)
          }
        },
        children
      )
    },
    DropdownMenuContent: ({
      children,
      ...props
    }: {
      children?: ReactNode
      'aria-label'?: string
      className?: string
      'data-workbench-menu'?: string
    }) => {
      const menu = React.use(MenuContext)
      if (!menu.open) return null
      return React.createElement(
        'div',
        {
          role: 'menu',
          'aria-label': props['aria-label'],
          className: props.className,
          'data-workbench-menu': props['data-workbench-menu']
        },
        children
      )
    },
    DropdownMenuItem: ({ children, onSelect, ...props }: { children?: ReactNode; onSelect?: () => void }) =>
      React.createElement('button', { type: 'button', onClick: () => onSelect?.(), ...props }, children)
  }
})

import { resetPermissionMenuOpenBlock, WorkbenchPermissionPicker } from '../WorkbenchPermissionPicker'
import {
  permissionModeForPosture,
  postureForPermissionMode,
  readPendingPermissionMode,
  writePendingPermissionMode
} from '../workbenchPermissionPostures'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

describe('workbench permission postures', () => {
  it('maps every a3s-code posture to a stored permission mode and back', () => {
    expect(permissionModeForPosture('default')).toBe('default')
    expect(permissionModeForPosture('plan')).toBe('plan')
    expect(permissionModeForPosture('auto')).toBe('auto')
    expect(permissionModeForPosture('always-approve')).toBe('bypassPermissions')
    expect(postureForPermissionMode('bypassPermissions')).toBe('always-approve')
  })

  it('reads modes a3s-code does not have as the default posture', () => {
    expect(postureForPermissionMode('acceptEdits')).toBe('default')
    expect(postureForPermissionMode(undefined)).toBe('default')
  })
})

describe('WorkbenchPermissionPicker', () => {
  beforeEach(() => {
    resetPermissionMenuOpenBlock()
    HTMLElement.prototype.hasPointerCapture ??= () => false
    HTMLElement.prototype.setPointerCapture ??= () => undefined
    HTMLElement.prototype.releasePointerCapture ??= () => undefined
  })

  it('offers exactly the a3s-code postures with descriptions', async () => {
    const user = userEvent.setup()
    render(<WorkbenchPermissionPicker permissionMode="auto" onPermissionModeChange={vi.fn()} />)

    const trigger = document.querySelector('[data-workbench-chip="authorize"]')!
    expect(trigger).toHaveAttribute('data-posture', 'auto')
    expect(trigger).toHaveTextContent('workbench.permission.auto.title')
    await user.click(trigger)

    for (const key of ['default', 'plan', 'auto', 'always_approve']) {
      expect(screen.getAllByText(`workbench.permission.${key}.title`).length).toBeGreaterThan(0)
      expect(screen.getByText(`workbench.permission.${key}.description`)).toBeInTheDocument()
    }
    expect(screen.queryByText(/acceptEdits/)).not.toBeInTheDocument()
    expect(screen.getByText('workbench.permission.risk')).toBeInTheDocument()
    expect(screen.getByText('workbench.permission.next_task_hint')).toBeInTheDocument()
  })

  it('opens from an accessibility click and names the next-action hint', () => {
    render(<WorkbenchPermissionPicker permissionMode="default" onPermissionModeChange={vi.fn()} />)
    expect(screen.queryByRole('group', { name: 'workbench.permission.next_task_hint' })).not.toBeInTheDocument()
    const trigger = document.querySelector('[data-workbench-chip="authorize"]') as HTMLButtonElement
    act(() => {
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    })
    expect(screen.getByRole('menu', { name: /workbench.permission.next_task_hint/ })).toBeInTheDocument()
    expect(screen.getByRole('group', { name: 'workbench.permission.next_task_hint' })).toBeVisible()
  })

  it('does not reopen when a choice clicks the trigger again', () => {
    const onPermissionModeChange = vi.fn()
    render(<WorkbenchPermissionPicker permissionMode="plan" onPermissionModeChange={onPermissionModeChange} />)
    const trigger = document.querySelector('[data-workbench-chip="authorize"]') as HTMLButtonElement
    act(() => {
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    })
    const item = screen.getByRole('menuitemradio', { name: /workbench.permission.default.title/ })
    act(() => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
      trigger.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, cancelable: true, button: 0 }))
    })
    expect(onPermissionModeChange).toHaveBeenCalledWith('default')
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('opens again when a later click is a new gesture', () => {
    vi.useFakeTimers()
    try {
      const onPermissionModeChange = vi.fn()
      render(<WorkbenchPermissionPicker permissionMode="plan" onPermissionModeChange={onPermissionModeChange} />)
      const trigger = document.querySelector('[data-workbench-chip="authorize"]') as HTMLButtonElement
      act(() => {
        vi.advanceTimersByTime(600)
        trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
      })
      const item = screen.getByRole('menuitemradio', { name: /workbench.permission.default.title/ })
      act(() => {
        item.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
      })
      expect(screen.queryByRole('menu')).not.toBeInTheDocument()
      act(() => {
        vi.advanceTimersByTime(600)
        trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
      })
      expect(screen.getByRole('menu', { name: /workbench.permission.next_task_hint/ })).toBeInTheDocument()
      expect(onPermissionModeChange).toHaveBeenCalledWith('default')
    } finally {
      vi.useRealTimers()
    }
  })

  it('closes its menu after a choice without hiding other nodes', () => {
    const leftover = document.createElement('div')
    leftover.setAttribute('data-workbench-menu', 'authorize')
    leftover.textContent = 'left in the page'
    document.body.appendChild(leftover)
    render(<WorkbenchPermissionPicker permissionMode="default" onPermissionModeChange={vi.fn()} />)
    const trigger = document.querySelector('[data-workbench-chip="authorize"]') as HTMLButtonElement
    act(() => {
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    })
    const item = screen.getByRole('menuitemradio', { name: /workbench.permission.plan.title/ })
    act(() => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    })
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(leftover.getAttribute('aria-hidden')).not.toBe('true')
    leftover.remove()
  })

  it('stays closed when a new chip receives the choice press', () => {
    const onPermissionModeChange = vi.fn()
    const first = render(
      <WorkbenchPermissionPicker permissionMode="default" onPermissionModeChange={onPermissionModeChange} />
    )
    const trigger = document.querySelector('[data-workbench-chip="authorize"]') as HTMLButtonElement
    act(() => {
      trigger.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    })
    const item = screen.getByRole('menuitemradio', { name: /workbench.permission.plan.title/ })
    act(() => {
      item.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    })
    expect(onPermissionModeChange).toHaveBeenCalledWith('plan')
    first.unmount()

    render(<WorkbenchPermissionPicker permissionMode="plan" onPermissionModeChange={onPermissionModeChange} />)
    const recreated = document.querySelector('[data-workbench-chip="authorize"]') as HTMLButtonElement
    act(() => {
      recreated.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, cancelable: true, button: 0 }))
      recreated.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }))
    })
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('stores the chosen posture as its permission mode', async () => {
    const user = userEvent.setup()
    const onPermissionModeChange = vi.fn()
    render(<WorkbenchPermissionPicker permissionMode="default" onPermissionModeChange={onPermissionModeChange} />)

    await user.click(document.querySelector('[data-workbench-chip="authorize"]')!)
    await user.click(screen.getByText('workbench.permission.always_approve.title'))
    expect(onPermissionModeChange).toHaveBeenCalledWith('bypassPermissions')

    onPermissionModeChange.mockClear()
    const openedAt = performance.now()
    vi.spyOn(performance, 'now').mockReturnValue(openedAt + 16000)
    await user.click(document.querySelector('[data-workbench-chip="authorize"]')!)
    vi.mocked(performance.now).mockRestore()
    await user.click(screen.getByText('workbench.permission.default.description'))
    expect(onPermissionModeChange).toHaveBeenCalledWith('default')
  })
})

describe('pending permission mode', () => {
  const memory = () => {
    const values = new Map<string, string>()
    return {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => void values.set(key, value),
      removeItem: (key: string) => void values.delete(key)
    }
  }

  it('copies an older unscoped mode onto the member key', () => {
    const storage = memory()
    storage.setItem('workbench.pending_permission_mode:v1', 'plan')
    expect(readPendingPermissionMode(storage, 'member-1')).toBe('plan')
    expect(storage.getItem('workbench.composer.permission-mode:v1:member-1')).toBe('plan')
  })

  it('remembers a mode picked before an agent exists and clears it once applied', () => {
    const storage = memory()
    expect(readPendingPermissionMode(storage)).toBeNull()
    writePendingPermissionMode('bypassPermissions', storage)
    expect(readPendingPermissionMode(storage)).toBe('bypassPermissions')
    writePendingPermissionMode(null, storage)
    expect(readPendingPermissionMode(storage)).toBeNull()
  })

  it('normalizes stored modes a3s-code does not support', () => {
    const storage = memory()
    writePendingPermissionMode('acceptEdits', storage)
    expect(readPendingPermissionMode(storage)).toBe('default')
  })
})
