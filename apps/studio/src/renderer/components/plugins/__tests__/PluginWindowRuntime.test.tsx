import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { useEffect, useState } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import type { WorkbenchPluginScope } from '@renderer/services/plugins'
import type { WorkbenchPlugin } from '@renderer/types/workbenchPlugin'

import { usePluginScope, usePluginScopeSnapshot } from '../PluginScopeProvider'
import { PluginWindowRuntime } from '../PluginWindowRuntime'

afterEach(cleanup)

describe('plugin window ownership', () => {
  it('updates the catalog in the same window scope while an old UI lease remains held', async () => {
    const released: number[] = []
    let activeScope: WorkbenchPluginScope | null = null
    const definition = (generation: number): readonly WorkbenchPlugin[] => [
      {
        id: 'extension',
        generation,
        scopes: ['window'],
        contributions: [],
        activate: (ctx) =>
          ctx.defer(() => {
            released.push(generation)
          })
      }
    ]
    function Child() {
      activeScope = usePluginScope()
      const snapshot = usePluginScopeSnapshot()
      return <p>Generation: {snapshot.fibers[0]?.generation ?? 0}</p>
    }
    const view = render(
      <PluginWindowRuntime namespace="test" windowId="main" plugins={definition(1)}>
        <Child />
      </PluginWindowRuntime>
    )
    await screen.findByText('Generation: 1')
    const original = activeScope!
    const lease = original.pin()
    view.rerender(
      <PluginWindowRuntime namespace="test" windowId="main" plugins={definition(2)}>
        <Child />
      </PluginWindowRuntime>
    )
    await screen.findByText('Generation: 2')
    expect(activeScope).toBe(original)
    expect(lease.snapshot.fibers[0].generation).toBe(1)
    expect(released).toEqual([])
    await lease.release()
    expect(released).toEqual([1])
    view.unmount()
    await waitFor(() => expect(released).toEqual([1, 2]))
  })
  it('publishes optional contributions without remounting the existing window', async () => {
    let ready!: () => void
    const activation = new Promise<void>((resolve) => {
      ready = resolve
    })
    const events: string[] = []
    const plugins: readonly WorkbenchPlugin[] = [
      {
        id: 'extension',
        generation: 1,
        scopes: ['window'],
        contributions: [],
        activate: async (ctx) => {
          ctx.defer(() => {
            events.push('plugin closed')
          })
          await activation
        }
      }
    ]
    function Child() {
      const snapshot = usePluginScopeSnapshot()
      const [draft] = useState('existing draft')
      useEffect(() => {
        events.push('window mounted')
        return () => {
          events.push('window unmounted')
        }
      }, [])
      return (
        <p>
          {draft}:{snapshot.fibers.length}
        </p>
      )
    }
    const window = render(
      <PluginWindowRuntime namespace="test" windowId="main" plugins={plugins}>
        <Child />
      </PluginWindowRuntime>
    )
    expect(screen.getByText('existing draft:0')).toBeTruthy()
    await act(async () => {
      ready()
      await activation
    })
    await screen.findByText('existing draft:1')
    expect(events).toEqual(['window mounted'])
    window.unmount()
    await waitFor(() => expect(events).toContain('plugin closed'))
    expect(events.filter((event) => event === 'window mounted')).toHaveLength(1)
    expect(events.filter((event) => event === 'window unmounted')).toHaveLength(1)
  })

  it('cancels activation owned by a window that unmounts before publication', async () => {
    let started!: () => void
    const activation = new Promise<void>((resolve) => {
      started = resolve
    })
    const events: string[] = []
    const plugins: readonly WorkbenchPlugin[] = [
      {
        id: 'extension',
        generation: 1,
        scopes: ['window'],
        contributions: [],
        activate: async (ctx) => {
          ctx.defer(() => {
            events.push('released')
          })
          ctx.spawn(async () => {
            events.push('published task')
          })
          started()
          await new Promise<void>((resolve) =>
            ctx.signal.addEventListener(
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
    ]
    const window = render(
      <PluginWindowRuntime namespace="test" windowId="main" plugins={plugins}>
        <p>Window</p>
      </PluginWindowRuntime>
    )
    await act(async () => {
      await activation
    })
    window.unmount()
    await waitFor(() => expect(events).toEqual(['cancelled', 'released']))
  })
})
