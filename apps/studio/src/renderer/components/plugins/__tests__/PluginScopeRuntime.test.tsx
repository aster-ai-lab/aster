import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { useState } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { WorkbenchPluginHost } from '@renderer/services/plugins'
import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'

import { PluginScopeProvider, usePluginScopeSnapshot } from '../PluginScopeProvider'
import { PluginScopeRuntime } from '../PluginScopeRuntime'
import { PluginSlot } from '../PluginSlot'

afterEach(cleanup)

describe('plugin child UI ownership', () => {
  // Catch a view activating unrelated plugins, stale allowlist effects or permissions excluded by its parent.
  it('binds selected plugins to the view owner and cannot widen its parent selection', async () => {
    const host = new WorkbenchPluginHost('test')
    const contexts: PluginActivationContext[] = []
    const released: string[] = []
    const definitions = ['alpha', 'beta'].map((id) => ({
      id,
      generation: 1,
      scopes: ['view'] as const,
      contributions: [
        { id: 'page', slot: 'page' as const, labelKey: 'plugins.title', component: () => <p>{id} view</p> }
      ],
      activate: (ctx: PluginActivationContext) => {
        contexts.push(ctx)
        ctx.defer(() => {
          released.push(id)
        })
      }
    }))
    await host.setPlugins(definitions)
    const parent = await host.openScope({ id: 'main', kind: 'window' })
    const limited = await host.openScope({ id: 'limited', kind: 'window', plugins: ['alpha'] })
    const tree = (plugins: readonly string[], owner = parent) => (
      <PluginScopeProvider scope={owner}>
        <PluginScopeRuntime kind="view" plugins={plugins}>
          <PluginSlot slot="page" />
        </PluginScopeRuntime>
      </PluginScopeProvider>
    )
    const rendered = render(tree(['alpha']))
    await screen.findByText('alpha view')
    expect(screen.queryByText('beta view')).not.toBeInTheDocument()
    const original = contexts[0]
    rendered.rerender(tree(['beta']))
    await screen.findByText('beta view')
    expect(screen.queryByText('alpha view')).not.toBeInTheDocument()
    expect(original.signal.aborted).toBe(true)
    await waitFor(() => expect(released).toEqual(['alpha']))
    rendered.rerender(tree(['alpha', 'beta']))
    await screen.findByText('alpha view')
    await screen.findByText('beta view')
    const current = [...contexts]
    rendered.rerender(tree(['beta', 'alpha']))
    expect(contexts).toEqual(current)
    expect(current.slice(-2).every((ctx) => ctx.isCurrent() && !ctx.signal.aborted)).toBe(true)
    rendered.rerender(tree(['alpha', 'beta'], limited))
    await act(async () => {
      await host.setPlugins(definitions)
    })
    expect(screen.queryByText('alpha view')).not.toBeInTheDocument()
    expect(screen.queryByText('beta view')).not.toBeInTheDocument()
    rendered.rerender(tree(['alpha'], limited))
    await screen.findByText('alpha view')
    rendered.rerender(tree([], limited))
    await waitFor(() => expect(screen.queryByText('alpha view')).not.toBeInTheDocument())
    await host.close()
    expect(contexts.every((ctx) => ctx.signal.aborted)).toBe(true)
  })

  it('rebinds a workspace without remounting content and withdraws descendants when the parent closes', async () => {
    const host = new WorkbenchPluginHost('test')
    const contexts: PluginActivationContext[] = []
    const released: string[] = []
    await host.setPlugins([
      {
        id: 'review',
        generation: 1,
        scopes: ['workspace'],
        services: ['workspace'],
        contributions: [],
        activate: (context) => {
          contexts.push(context)
          context.defer(() => {
            released.push(context.services.workspace!.id)
          })
        }
      }
    ])
    const parent = await host.openScope({ id: 'main', kind: 'window' })
    function Content() {
      const [draft, setDraft] = useState('keep draft')
      const snapshot = usePluginScopeSnapshot()
      return (
        <>
          <input aria-label="draft" value={draft} onChange={(event) => setDraft(event.target.value)} />
          <p>
            {draft}:{snapshot.fibers.length}
          </p>
        </>
      )
    }
    const view = (id: string, reverse = false) => (
      <PluginScopeProvider scope={parent}>
        <PluginScopeRuntime
          kind="workspace"
          services={{ workspace: reverse ? { root: `/work/${id}`, id } : { id, root: `/work/${id}` } }}>
          <Content />
        </PluginScopeRuntime>
      </PluginScopeProvider>
    )
    const rendered = render(view('alpha'))
    await screen.findByText('keep draft:1')
    fireEvent.change(screen.getByRole('textbox', { name: 'draft' }), { target: { value: 'edited draft' } })
    rendered.rerender(view('alpha', true))
    expect(contexts).toHaveLength(1)
    rendered.rerender(view('beta'))
    await waitFor(() => expect(contexts).toHaveLength(2))
    await screen.findByText('edited draft:1')
    expect(contexts[0].signal.aborted).toBe(true)
    expect(contexts[0].isCurrent()).toBe(false)
    expect(contexts[1].services.workspace?.id).toBe('beta')
    await waitFor(() => expect(released).toEqual(['alpha']))
    await act(async () => {
      await parent.close()
    })
    expect(screen.getByText('edited draft:0')).toBeTruthy()
    expect(contexts[1].signal.aborted).toBe(true)
    expect(released).toEqual(['alpha', 'beta'])
    await host.close()
  })

  it('aborts an unmounted child activation while its parent window remains usable', async () => {
    const host = new WorkbenchPluginHost('test')
    const parent = await host.openScope({ id: 'main', kind: 'window' })
    let started!: () => void
    const ready = new Promise<void>((resolve) => {
      started = resolve
    })
    const events: string[] = []
    await host.setPlugins([
      {
        id: 'review',
        generation: 1,
        scopes: ['workspace'],
        contributions: [],
        activate: async (context) => {
          context.defer(() => {
            events.push('released')
          })
          context.spawn(async () => {
            events.push('published')
          })
          started()
          await new Promise<void>((resolve) =>
            context.signal.addEventListener(
              'abort',
              () => {
                events.push('cancelled')
                resolve()
              },
              { once: true }
            )
          )
        }
      }
    ])
    const view = render(
      <PluginScopeProvider scope={parent}>
        <PluginScopeRuntime kind="workspace">
          <p>Existing content</p>
        </PluginScopeRuntime>
      </PluginScopeProvider>
    )
    await act(async () => {
      await ready
    })
    view.unmount()
    await waitFor(() => expect(events).toEqual(['cancelled', 'released']))
    expect(parent.isOpen()).toBe(true)
    await host.setPlugins([])
    await host.close()
  })
})
