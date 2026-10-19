import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

vi.unmock('@a3s-lab/ui')

import { WorkbenchPluginHost } from '@renderer/services/plugins'
import type { WorkbenchPlugin } from '@renderer/types/workbenchPlugin'

import { createAixWorkbenchPlugins } from '../AixWorkbenchRuntime'
import { PluginComposerActions } from '../PluginComposerActions'
import { PluginScopeProvider } from '../PluginScopeProvider'

afterEach(cleanup)

describe('scoped contribution composition', () => {
  // Catch ancestor inheritance widening a child allowlist or surviving the child's closure.
  it('inherits only permitted actions, observes ancestor withdrawal and closes with the consumer', async () => {
    const host = new WorkbenchPluginHost('composition')
    const definition = (id: string): WorkbenchPlugin => ({
      id,
      generation: 1,
      scopes: ['window'],
      contributions: [{ id: 'action', slot: 'composer', labelKey: 'plugins.title', title: id, invoke: () => id }]
    })
    const alpha = definition('alpha')
    const beta = definition('beta')
    await host.setPlugins([alpha, beta])
    const window = await host.openScope({ id: 'main', kind: 'window' })
    const workspace = await host.openScope({ id: 'workspace', kind: 'workspace', parent: window })
    const session = await host.openScope({
      id: 'session',
      kind: 'session',
      parent: workspace,
      plugins: ['alpha']
    })
    const edits: string[] = []
    render(
      <PluginScopeProvider scope={session}>
        <PluginComposerActions
          readDraft={() => ({ id: 'draft', revision: 0, text: '' })}
          applyEdit={(_draft, text) => edits.push(text)}
        />
      </PluginScopeProvider>
    )
    expect(screen.queryByRole('button', { name: 'beta' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'alpha' }))
    await waitFor(() => expect(edits).toEqual(['alpha']))
    await act(async () => {
      await host.setPlugins([beta])
    })
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
    await act(async () => {
      await host.setPlugins([alpha, beta])
    })
    expect(screen.getByRole('button', { name: 'alpha' })).toBeEnabled()
    await act(async () => {
      await session.close()
    })
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
    expect(window.isOpen()).toBe(true)
    await host.close()
  })

  // Catch borrowed window authority replacing a nearer activation or duplicate execution of one action.
  it('selects the nearest activation for one key and preserves deterministic ordering', async () => {
    const host = new WorkbenchPluginHost('composition')
    await host.setPlugins([
      {
        id: 'edit',
        generation: 1,
        scopes: ['window', 'workspace', 'session'],
        contributions: [
          {
            id: 'owner',
            slot: 'composer',
            labelKey: 'plugins.title',
            title: 'Owner',
            order: 5,
            invoke: (context) => `${context.scopeId}:${context.services.conversation?.sessionId}`
          },
          {
            id: 'other',
            slot: 'composer',
            labelKey: 'plugins.title',
            title: 'Other',
            order: 1,
            scopes: ['window'],
            invoke: () => 'other'
          }
        ]
      }
    ])
    const window = await host.openScope({ id: 'main', kind: 'window' })
    const workspace = await host.openScope({ id: 'workspace', kind: 'workspace', parent: window })
    const session = await host.openScope({
      id: 'session',
      kind: 'session',
      parent: workspace,
      services: { conversation: { sessionId: 'selected-session' } }
    })
    const edits: string[] = []
    render(
      <PluginScopeProvider scope={session}>
        <PluginComposerActions
          readDraft={() => ({ id: 'draft', revision: 0, text: '' })}
          applyEdit={(_draft, text) => edits.push(text)}
        />
      </PluginScopeProvider>
    )
    expect(screen.getAllByRole('button').map((button) => button.textContent)).toEqual(['Other', 'Owner'])
    fireEvent.click(screen.getByRole('button', { name: 'Owner' }))
    await waitFor(() => expect(edits).toEqual([`${session.id}:selected-session`]))
    await host.close()
  })

  // Catch a session contribution being admitted before its mandatory conversation service exists.
  it('refuses AIX session actions without the selected conversation', async () => {
    const host = new WorkbenchPluginHost('composition')
    await host.setPlugins(
      createAixWorkbenchPlugins({
        protocolVersion: 1,
        revision: `sha256:${'a'.repeat(64)}`,
        packages: [
          {
            slug: 'composer',
            packageId: 'use/proof/composer',
            generation: 1,
            revision: `sha256:${'b'.repeat(64)}`,
            containers: [],
            views: [
              {
                id: 'action',
                title: 'Action',
                description: '',
                icon: 'package',
                order: 1,
                container: null,
                point: 'studio.composer.actions',
                scope: 'session'
              }
            ]
          }
        ]
      })
    )
    const window = await host.openScope({ id: 'main', kind: 'window' })
    const workspace = await host.openScope({ id: 'workspace', kind: 'workspace', parent: window })
    const session = await host.openScope({ id: 'session', kind: 'session', parent: workspace })
    expect(session.getSnapshot().contributions).toEqual([])
    expect(session.getSnapshot().diagnostics?.map((item) => item.missing)).toEqual([['conversation']])
    for (const sessionId of ['', '   ']) {
      await expect(
        host.openScope({
          id: 'invalid-session',
          kind: 'session',
          parent: workspace,
          services: { conversation: { sessionId } }
        })
      ).rejects.toThrow('Plugin conversation identity is required')
    }
    const recovered = await host.openScope({
      id: 'invalid-session',
      kind: 'session',
      parent: workspace,
      services: { conversation: { sessionId: 'selected-session' } }
    })
    expect(recovered.getSnapshot().contributions.map((item) => item.contribution.title)).toEqual(['Action'])
    await host.close()
  })
})
