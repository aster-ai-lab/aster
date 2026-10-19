import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { WorkbenchPluginHost } from '@renderer/services/plugins'
import type { WorkbenchPlugin } from '@renderer/types/workbenchPlugin'

import { PluginComposerActions } from '../PluginComposerActions'
import { PluginScopeProvider } from '../PluginScopeProvider'

afterEach(cleanup)

async function setup(invoke: Extract<WorkbenchPlugin['contributions'][number], { slot: 'composer' }>['invoke']) {
  const host = new WorkbenchPluginHost('test')
  await host.setPlugins([
    {
      id: 'edit',
      generation: 1,
      scopes: ['window'],
      contributions: [{ id: 'append', slot: 'composer', labelKey: 'common.add', invoke }]
    }
  ])
  return { host, scope: await host.openScope({ id: 'main', kind: 'window' }) }
}

describe('plugin composer ownership', () => {
  it('drops delayed edits after the originating scope is closed', async () => {
    let finish!: (text: string) => void
    const pending = new Promise<string>((resolve) => {
      finish = resolve
    })
    const { host, scope } = await setup(() => pending)
    const applied: string[] = []
    render(
      <PluginScopeProvider scope={scope}>
        <PluginComposerActions
          readDraft={() => Object.freeze({ id: 'draft', revision: 0, text: 'draft' })}
          applyEdit={(_expected, replacement) => {
            applied.push(replacement)
          }}
        />
      </PluginScopeProvider>
    )
    fireEvent.click(screen.getByRole('button'))
    let closing!: ReturnType<typeof scope.close>
    await act(async () => {
      closing = scope.close()
      finish('late')
      await closing
    })
    expect(applied).toEqual([])
    await host.close()
  })

  it('disables plugin edits when the consumer cannot accept a plain-text replacement', async () => {
    let calls = 0
    const { host, scope } = await setup((_ctx, text) => {
      calls++
      return text
    })
    render(
      <PluginScopeProvider scope={scope}>
        <PluginComposerActions
          readDraft={() => Object.freeze({ id: 'draft', revision: 0, text: 'draft' })}
          disabled
          applyEdit={() => {
            throw new Error('must not edit')
          }}
        />
      </PluginScopeProvider>
    )
    fireEvent.click(screen.getByRole('button'))
    expect(calls).toBe(0)
    await act(async () => {
      await host.close()
    })
  })
})
