import { act, cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { StrictMode, useEffect } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { useCommandRuntime, useResolvedCommand } from '@renderer/hooks/command'
import { extensionCommandId } from '@renderer/services/rendererCommandService'
import type { CommandRuntime, ExtensionCommandId } from '@renderer/types/command'

vi.mock('@a3s-lab/ui', async () => await vi.importActual('@a3s-lab/ui'))

import { CommandButton } from '../CommandControls'
import { CommandProvider } from '../CommandProvider'

const id = extensionCommandId('aix.proof.views', 'a'.repeat(64), 'refresh')

afterEach(cleanup)

describe('dynamic commands in the renderer provider', () => {
  it('updates the real command control and revokes its captured operation on close under StrictMode', async () => {
    const user = userEvent.setup()
    const abort = new globalThis.AbortController()
    const owner = { signal: abort.signal, isCurrent: () => !abort.signal.aborted }
    const effects: string[] = []
    let runtime!: CommandRuntime
    let saved!: () => void
    let enabled = true
    let refresh!: () => void
    function Registration() {
      runtime = useCommandRuntime()
      useEffect(() => {
        const registration = runtime.registerCommand({ id, title: 'Refresh view' }, owner, {
          enabled: () => enabled,
          execute: () => {
            effects.push('refresh')
          }
        })
        refresh = registration.refresh
        return registration.dispose
      }, [])
      return null
    }
    function Control({ command }: { command: ExtensionCommandId }) {
      saved = useResolvedCommand(command).execute
      return <CommandButton command={command}>Refresh view</CommandButton>
    }
    const { unmount } = render(
      <StrictMode>
        <CommandProvider>
          <Registration />
          <Control command={id} />
        </CommandProvider>
      </StrictMode>
    )
    const button = screen.getByRole('button', { name: 'Refresh view' })
    expect(button).toBeEnabled()
    await user.click(button)
    expect(effects).toEqual(['refresh'])
    const oldAction = saved
    act(() => {
      enabled = false
      refresh()
    })
    expect(button).toBeDisabled()
    act(() => oldAction())
    expect(effects).toEqual(['refresh'])
    act(() => {
      enabled = true
      refresh()
    })
    expect(button).toBeEnabled()
    act(() => abort.abort())
    expect(button).toBeDisabled()
    act(() => oldAction())
    expect(effects).toEqual(['refresh'])
    unmount()
    const next = new globalThis.AbortController()
    expect(() =>
      runtime.registerCommand(
        { id, title: 'Late view' },
        {
          signal: next.signal,
          isCurrent: () => true
        },
        { enabled: () => true, execute: () => {} }
      )
    ).toThrow('closed')
  })
})
