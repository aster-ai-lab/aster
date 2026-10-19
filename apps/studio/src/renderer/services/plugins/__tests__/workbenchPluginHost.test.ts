import { describe, expect, it } from 'vitest'

import type { PluginActivationContext, WorkbenchPlugin } from '@renderer/types/workbenchPlugin'

import { WorkbenchPluginHost } from '../workbenchPluginHost'

function plugin(overrides: Partial<WorkbenchPlugin> = {}): WorkbenchPlugin {
  return { id: 'review', generation: 1, scopes: ['workspace'], contributions: [], ...overrides }
}

async function workspace(host: WorkbenchPluginHost, id = 'alpha') {
  const root = await host.openScope({ id: 'main', kind: 'window' })
  return host.openScope({ id, kind: 'workspace', parent: root, services: { workspace: { id, root: `/work/${id}` } } })
}

describe('workbench plugin contracts', () => {
  // Catch a child fabricating another execution identity or using a model ordinal without a Run.
  it('freezes inherited Run observations and refuses execution rebinding or unqualified turns', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const parent = await workspace(host)
    const session = await host.openScope({
      id: 'session',
      kind: 'session',
      parent,
      services: { conversation: { sessionId: 'session' } }
    })
    const run = await host.openScope({
      id: 'run',
      kind: 'run',
      parent: session,
      services: { run: { coreSessionId: 'core', runId: 'run-one', promptId: 'prompt' } }
    })
    const turn = await host.openScope({ id: 'turn', kind: 'turn', parent: run, services: { turn: { modelTurn: 1 } } })
    for (const replacement of [
      { ...run.services.run!, runId: 'other' },
      { ...run.services.run!, coreSessionId: 'other' },
      { ...run.services.run!, promptId: 'other' }
    ])
      await expect(
        host.openScope({ id: 'foreign', kind: 'view', parent: turn, services: { run: replacement } })
      ).rejects.toThrow('parent Run')
    await expect(
      host.openScope({ id: 'bad-turn', kind: 'view', parent: turn, services: { turn: { modelTurn: 2 } } })
    ).rejects.toThrow('parent model turn')
    await expect(
      host.openScope({ id: 'missing-run', kind: 'run', parent: session, services: { turn: { modelTurn: 1 } } })
    ).rejects.toThrow('admitted Run')
    await expect(
      host.openScope({
        id: 'unsafe',
        kind: 'turn',
        parent: run,
        services: { turn: { modelTurn: Number.MAX_SAFE_INTEGER + 1 } }
      })
    ).rejects.toThrow('positive ordinal')
    expect(Object.isFrozen(turn.services.run)).toBe(true)
    await host.close()
  })
  it('cancels an opening child before it can publish tasks and releases its identity', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const parent = await host.openScope({ id: 'main', kind: 'window' })
    let started!: () => void
    const ready = new Promise<void>((resolve) => {
      started = resolve
    })
    const events: string[] = []
    await host.setPlugins([
      plugin({
        activate: async (ctx) => {
          ctx.defer(() => {
            events.push('released')
          })
          ctx.spawn(async () => {
            events.push('published')
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
      })
    ])
    const cancellation = new globalThis.AbortController()
    const opening = host.openScope({ id: 'alpha', kind: 'workspace', parent, signal: cancellation.signal })
    const refused = expect(opening).rejects.toThrow(/cancelled|closed/)
    await ready
    cancellation.abort()
    await refused
    expect(events).toEqual(['cancelled', 'released'])
    await host.setPlugins([])
    const replacement = await host.openScope({ id: 'alpha', kind: 'workspace', parent })
    expect(replacement.isOpen()).toBe(true)
    await expect(
      host.openScope({ id: 'beta', kind: 'workspace', parent, signal: cancellation.signal })
    ).rejects.toThrow('cancelled')
    await host.close()
  })

  it('treats an explicitly absent optional workspace as unavailable', async () => {
    const host = new WorkbenchPluginHost('tenant')
    await host.setPlugins([plugin({ services: ['workspace'] })])
    const parent = await host.openScope({ id: 'main', kind: 'window' })
    const child = await host.openScope({
      id: 'unassigned',
      kind: 'workspace',
      parent,
      services: { workspace: undefined }
    })
    expect(child.getSnapshot().fibers[0]).toMatchObject({ state: 'blocked', missing: ['service:workspace'] })
    const bound = await host.openScope({
      id: 'bound',
      kind: 'workspace',
      parent,
      services: { workspace: { id: 'alpha', root: '/work/alpha' } }
    })
    const descendant = await host.openScope({
      id: 'chat',
      kind: 'session',
      parent: bound,
      services: { workspace: undefined }
    })
    expect(descendant.services.workspace).toEqual({ id: 'alpha', root: '/work/alpha' })
    await host.close()
  })
  it('withdraws the entire subtree and revokes leases before slow cleanup completes', async () => {
    const host = new WorkbenchPluginHost('tenant')
    let finish!: () => void
    const cleanup = new Promise<void>((resolve) => {
      finish = resolve
    })
    await host.setPlugins([plugin({ scopes: ['workspace', 'session'], activate: (ctx) => ctx.defer(() => cleanup) })])
    const parent = await workspace(host)
    const child = await host.openScope({ id: 'chat', kind: 'session', parent })
    const lease = child.pin()
    const childClosed: boolean[] = []
    parent.subscribe(() => {
      childClosed.push(!child.isOpen())
      void parent.close()
    })
    const closing = parent.close()
    expect(parent.getSnapshot().fibers).toEqual([])
    expect(child.getSnapshot().fibers).toEqual([])
    expect(() => lease.snapshot).toThrow('closed')
    expect(childClosed).toEqual([true])
    finish()
    expect(await closing).toEqual({ failed: [], timedOut: [] })
    await lease.release()
    await host.close()
  })
  it('reserves built-in application identities and treats icon changes as a new generation', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const Page = () => null
    const Icon = () => null
    const contributions: WorkbenchPlugin['contributions'] = [
      { id: 'page', slot: 'page', labelKey: 'plugins.title', component: Page },
      {
        id: 'activity',
        slot: 'activity',
        labelKey: 'plugins.title',
        pageId: 'page',
        applicationId: 'plugins',
        icon: Icon
      }
    ]
    await expect(host.setPlugins([plugin({ contributions })])).rejects.toThrow('built-in activity identity')
    const builtin = plugin({ id: 'studio.plugins', scopes: ['window'], contributions })
    await host.setPlugins([builtin])
    await expect(
      host.setPlugins([
        {
          ...builtin,
          contributions: [contributions[0], { ...contributions[1], slot: 'activity', pageId: 'page', icon: () => null }]
        }
      ])
    ).rejects.toThrow('immutable')
    await host.close()
  })
  it('refuses updates when retained generations exhaust the budget and resumes after release', async () => {
    const host = new WorkbenchPluginHost('tenant')
    await host.setPlugins([plugin()])
    const scope = await workspace(host)
    const leases = [scope.pin()]
    for (let generation = 2; generation <= 32; generation++) {
      await host.setPlugins([plugin({ generation })])
      leases.push(scope.pin())
    }
    const before = scope.getSnapshot()
    await expect(host.setPlugins([plugin({ generation: 33 })])).rejects.toThrow('generation limit')
    expect(scope.getSnapshot()).toBe(before)
    await leases[0].release()
    await host.setPlugins([plugin({ generation: 33 })])
    expect(scope.getSnapshot().fibers[0].generation).toBe(33)
    for (const lease of leases) await lease.release()
    await host.close()
  })
  it('rolls the entire catalog back when a second workspace fails preparation', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const old = plugin()
    await host.setPlugins([old])
    const alpha = await workspace(host)
    const beta = await host.openScope({
      id: 'beta',
      kind: 'workspace',
      parent: alpha.options.parent,
      services: { workspace: { id: 'beta', root: '/work/beta' } }
    })
    const before = [alpha.getSnapshot(), beta.getSnapshot()]
    const events: string[] = []
    const bus = new globalThis.EventTarget()
    await expect(
      host.setPlugins([
        plugin({
          generation: 2,
          activate: (context) => {
            const id = context.services.workspace!.id
            context.defer(() => {
              events.push(`close:${id}`)
            })
            context.listen(bus, 'change', () => {
              events.push(`event:${id}`)
            })
            context.spawn(async () => {
              events.push(`task:${id}`)
            })
            if (id === 'beta') throw new Error('beta unavailable')
          }
        })
      ])
    ).rejects.toThrow('beta unavailable')
    bus.dispatchEvent(new globalThis.Event('change'))
    expect(alpha.getSnapshot()).toBe(before[0])
    expect(beta.getSnapshot()).toBe(before[1])
    expect(events.toSorted()).toEqual(['close:alpha', 'close:beta'])
    const gamma = await host.openScope({ id: 'gamma', kind: 'workspace', parent: alpha.options.parent })
    expect(gamma.getSnapshot().fibers[0].generation).toBe(1)
    await host.close()
  })

  it('publishes every workspace before notifying observers and starting staged tasks', async () => {
    const host = new WorkbenchPluginHost('tenant')
    await host.setPlugins([plugin()])
    const alpha = await workspace(host)
    const beta = await host.openScope({ id: 'beta', kind: 'workspace', parent: alpha.options.parent })
    const seen: number[] = []
    alpha.subscribe(() => {
      const fiber = beta.getSnapshot().fibers[0]
      if (fiber) seen.push(fiber.generation)
    })
    await host.setPlugins([
      plugin({
        generation: 2,
        activate: (ctx) =>
          ctx.spawn(async () => {
            seen.push(alpha.getSnapshot().fibers[0].generation, beta.getSnapshot().fibers[0].generation)
          })
      })
    ])
    expect(seen).toEqual([2, 2, 2, 2, 2])
    await host.close()
  })

  it('retains immutable generation identity across removal and serializes concurrent updates', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const first = plugin()
    await host.setPlugins([first])
    const scope = await workspace(host)
    const held = scope.pin()
    await host.setPlugins([])
    await expect(host.setPlugins([plugin({ activate: () => undefined })])).rejects.toThrow('immutable')
    await host.setPlugins([first])
    await held.release()
    await Promise.all([host.setPlugins([plugin({ generation: 2 })]), host.setPlugins([plugin({ generation: 3 })])])
    expect(scope.getSnapshot().fibers[0].generation).toBe(3)
    await expect(host.setPlugins([first])).rejects.toThrow('immutable')
    await host.close()
  })

  it('cancels an in-flight catalog transaction when the host closes', async () => {
    const host = new WorkbenchPluginHost('tenant')
    await host.setPlugins([plugin()])
    await workspace(host)
    let activated!: () => void
    const started = new Promise<void>((resolve) => {
      activated = resolve
    })
    const cleaned: string[] = []
    const update = host.setPlugins([
      plugin({
        generation: 2,
        activate: async (ctx) => {
          ctx.defer(() => {
            cleaned.push('released')
          })
          activated()
          await new Promise<void>((resolve) => ctx.signal.addEventListener('abort', () => resolve(), { once: true }))
        }
      })
    ])
    await started
    const rejected = expect(update).rejects.toThrow('cancel')
    expect(await host.close()).toEqual({ failed: [], timedOut: [] })
    await rejected
    expect(cleaned).toEqual(['released'])
  })

  it('refuses same-generation factory substitution and orphan activity pages', async () => {
    const host = new WorkbenchPluginHost('tenant')
    await host.setPlugins([plugin()])
    await expect(host.setPlugins([plugin({ activate: () => undefined })])).rejects.toThrow('immutable')
    await expect(
      host.setPlugins([
        plugin({
          generation: 2,
          contributions: [{ id: 'entry', slot: 'activity', labelKey: 'common.add', pageId: 'missing' }]
        })
      ])
    ).rejects.toThrow('local page')
    await host.close()
  })
  it('keeps missing services and transitive dependents inactive', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const effects: string[] = []
    await host.setPlugins([
      plugin({
        services: ['document'],
        activate: () => {
          effects.push('review')
        }
      }),
      plugin({
        id: 'findings',
        requires: ['review'],
        activate: () => {
          effects.push('findings')
        }
      })
    ])
    const scope = await workspace(host)
    expect(effects).toEqual([])
    expect(scope.getSnapshot().fibers).toEqual([
      { id: 'review', generation: 1, state: 'blocked', missing: ['service:document'] },
      { id: 'findings', generation: 1, state: 'blocked', missing: ['plugin:review'] }
    ])
    await host.close()
  })

  it('activates dependencies first and tears dependents down first', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const events: string[] = []
    const tracked = (name: string) => (context: PluginActivationContext) => {
      events.push(`open:${name}`)
      context.defer(() => {
        events.push(`close:${name}`)
      })
    }
    await host.setPlugins([
      plugin({ id: 'consumer', requires: ['provider'], activate: tracked('consumer') }),
      plugin({ id: 'provider', activate: tracked('provider') })
    ])
    await workspace(host)
    await host.close()
    expect(events).toEqual(['open:provider', 'open:consumer', 'close:consumer', 'close:provider'])
  })

  it('refuses cycles and duplicate identities without replacing a published catalog', async () => {
    const host = new WorkbenchPluginHost('tenant')
    await host.setPlugins([plugin()])
    const scope = await workspace(host)
    const before = scope.getSnapshot()
    await expect(host.setPlugins([plugin(), plugin()])).rejects.toThrow('Duplicate plugin')
    await expect(host.setPlugins([plugin({ requires: ['review'] })])).rejects.toThrow('cycle')
    await expect(
      host.setPlugins([
        plugin({
          contributions: [
            { id: 'same', slot: 'composer', labelKey: 'common.add', invoke: (_ctx, text) => text },
            { id: 'same', slot: 'composer', labelKey: 'common.add', invoke: (_ctx, text) => text }
          ]
        })
      ])
    ).rejects.toThrow('contribution')
    expect(scope.getSnapshot()).toBe(before)
    await host.close()
  })

  it('isolates workspace services and fences listeners after one workspace closes', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const bus = new globalThis.EventTarget()
    const observed: string[] = []
    await host.setPlugins([
      plugin({
        activate: (ctx) =>
          ctx.listen(bus, 'change', () => {
            observed.push(ctx.services.workspace!.id)
          })
      })
    ])
    const alpha = await workspace(host)
    const beta = await host.openScope({
      id: 'beta',
      kind: 'workspace',
      parent: alpha.options.parent,
      services: { workspace: { id: 'beta', root: '/work/beta' } }
    })
    bus.dispatchEvent(new globalThis.Event('change'))
    await alpha.close()
    bus.dispatchEvent(new globalThis.Event('change'))
    expect(observed).toEqual(['alpha', 'beta', 'beta'])
    expect(beta.getSnapshot().fibers[0].state).toBe('active')
    await host.close()
  })

  it('rolls failed activation back in reverse order and preserves the old generation', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const events: string[] = []
    await host.setPlugins([plugin()])
    const scope = await workspace(host)
    const before = scope.getSnapshot()
    await expect(
      host.setPlugins([
        plugin({
          generation: 2,
          activate: (ctx) => {
            ctx.defer(() => {
              events.push('first')
            })
            ctx.defer(() => {
              events.push('second')
            })
            throw new Error('backend not ready')
          }
        })
      ])
    ).rejects.toThrow('backend not ready')
    expect(events).toEqual(['second', 'first'])
    expect(scope.getSnapshot()).toBe(before)
    await host.close()
  })

  // Catch an unrelated publication or failed candidate closing effects shared by a current scope and old UI lease.
  it('preserves unchanged callbacks across publication, rollback and old lease release', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const bus = new globalThis.EventTarget()
    const events: string[] = []
    let context!: PluginActivationContext
    const base = plugin({
      activate: (ctx) => {
        context = ctx
        ctx.listen(bus, 'change', () => events.push('change'))
        ctx.defer(() => {
          events.push('closed')
        })
      }
    })
    const other = plugin({ id: 'other' })
    await host.setPlugins([base])
    const scope = await workspace(host)
    const original = context
    const first = scope.pin()
    await host.setPlugins([base, other])
    const second = scope.pin()
    await first.release()
    expect(context).toBe(original)
    expect(original.isCurrent()).toBe(true)
    expect(original.signal.aborted).toBe(false)
    await expect(
      host.setPlugins([
        base,
        other,
        plugin({
          id: 'z-fail',
          activate: () => {
            throw new Error('candidate refused')
          }
        })
      ])
    ).rejects.toThrow('candidate refused')
    bus.dispatchEvent(new globalThis.Event('change'))
    expect(events).toEqual(['change'])
    await host.setPlugins([base])
    await second.release()
    expect(original.isCurrent()).toBe(true)
    expect(original.signal.aborted).toBe(false)
    const held = scope.pin()
    const closing = scope.close()
    expect(original.signal.aborted).toBe(true)
    expect(() => held.snapshot).toThrow('closed')
    await closing
    await held.release()
    bus.dispatchEvent(new globalThis.Event('change'))
    expect(events).toEqual(['change', 'closed'])
    await host.close()
  })

  // Catch a successfully prepared scope releasing shared live effects when another scope rejects publication.
  it('keeps both scopes live after cross-scope candidate rollback', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const contexts: PluginActivationContext[] = []
    const bus = new globalThis.EventTarget()
    const seen: string[] = []
    const base = plugin({
      activate: (ctx) => {
        contexts.push(ctx)
        ctx.listen(bus, 'change', () => seen.push(ctx.services.workspace!.id))
        ctx.defer(() => {
          seen.push('closed')
        })
      }
    })
    await host.setPlugins([base])
    const alpha = await workspace(host)
    await host.openScope({
      id: 'beta',
      kind: 'workspace',
      parent: alpha.options.parent,
      services: { workspace: { id: 'beta', root: '/work/beta' } }
    })
    await expect(
      host.setPlugins([
        base,
        plugin({
          id: 'z-fail',
          activate: (ctx) => {
            if (ctx.services.workspace!.id === 'beta') throw new Error('beta refused')
          }
        })
      ])
    ).rejects.toThrow('beta refused')
    for (const ctx of contexts) {
      expect(ctx.isCurrent()).toBe(true)
      expect(ctx.signal.aborted).toBe(false)
    }
    bus.dispatchEvent(new globalThis.Event('change'))
    expect(seen).toEqual(['alpha', 'beta'])
    await host.close()
    expect(seen.filter((value) => value === 'closed')).toHaveLength(2)
  })

  // Catch reuse of a dependent's activation after its provider is upgraded or withdrawn.
  it('replaces dependent effects when provider identity changes and preserves independent effects', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const contexts = new Map<string, PluginActivationContext>()
    const closed: string[] = []
    const tracked = (id: string) => (ctx: PluginActivationContext) => {
      contexts.set(id, ctx)
      ctx.defer(() => {
        closed.push(id)
      })
    }
    const provider = plugin({ id: 'provider', activate: tracked('provider') })
    const consumer = plugin({ id: 'consumer', requires: ['provider'], activate: tracked('consumer') })
    const independent = plugin({ id: 'independent', activate: tracked('independent') })
    await host.setPlugins([provider, consumer, independent])
    const scope = await workspace(host)
    const oldProvider = contexts.get('provider')!
    const oldConsumer = contexts.get('consumer')!
    const unchanged = contexts.get('independent')!
    await host.setPlugins([{ ...provider, generation: 2 }, consumer, independent])
    expect(oldProvider.signal.aborted).toBe(true)
    expect(oldConsumer.signal.aborted).toBe(true)
    expect(contexts.get('consumer')).not.toBe(oldConsumer)
    expect(unchanged.isCurrent()).toBe(true)
    expect(unchanged.signal.aborted).toBe(false)
    expect(closed).toEqual(['consumer', 'provider'])
    const replacement = contexts.get('consumer')!
    await host.setPlugins([consumer, independent])
    expect(replacement.signal.aborted).toBe(true)
    expect(scope.getSnapshot().fibers.find((item) => item.id === 'consumer')).toMatchObject({
      state: 'blocked',
      missing: ['plugin:provider']
    })
    expect(unchanged.isCurrent()).toBe(true)
    await host.close()
    expect(closed.filter((id) => id === 'independent')).toHaveLength(1)
  })

  it('pins old resources until the exact contribution lease is released', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const closed: number[] = []
    let oldContext: PluginActivationContext | undefined
    await host.setPlugins([
      plugin({
        activate: (ctx) => {
          oldContext = ctx
          ctx.defer(() => {
            closed.push(1)
          })
        }
      })
    ])
    const scope = await workspace(host)
    const lease = scope.pin()
    await host.setPlugins([
      plugin({
        generation: 2,
        activate: (ctx) =>
          ctx.defer(() => {
            closed.push(2)
          })
      })
    ])
    expect(scope.getSnapshot().fibers[0].generation).toBe(2)
    expect(lease.snapshot.fibers[0].generation).toBe(1)
    expect(oldContext?.isCurrent()).toBe(false)
    expect(oldContext?.signal.aborted).toBe(false)
    expect(closed).toEqual([])
    await lease.release()
    await lease.release()
    expect(closed).toEqual([1])
    expect(oldContext?.signal.aborted).toBe(true)
    await expect(Promise.resolve().then(() => lease.snapshot)).rejects.toThrow('closed')
    await host.close()
    expect(closed).toEqual([1, 2])
  })

  it('does not expose prepared callbacks or start tasks before complete publication', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const bus = new globalThis.EventTarget()
    const events: string[] = []
    await host.setPlugins([
      plugin({
        activate: (ctx) => {
          ctx.listen(bus, 'change', () => {
            events.push('visible')
          })
          ctx.spawn(async () => {
            events.push('task')
          })
          bus.dispatchEvent(new globalThis.Event('change'))
        }
      })
    ])
    await workspace(host)
    bus.dispatchEvent(new globalThis.Event('change'))
    await host.close()
    expect(events).toEqual(['task', 'visible'])
  })

  it('cancels child tasks before cleaning parent resources and invalidates held leases', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const events: string[] = []
    await host.setPlugins([
      plugin({
        scopes: ['workspace', 'session'],
        activate: (ctx) => {
          ctx.defer(() => {
            events.push(`close:${ctx.scopeId.split('/').at(-1)}`)
          })
          ctx.spawn(
            (signal) =>
              new Promise<void>((resolve) => {
                if (signal.aborted) resolve()
                else
                  signal.addEventListener(
                    'abort',
                    () => {
                      events.push('cancelled')
                      resolve()
                    },
                    { once: true }
                  )
              })
          )
        }
      })
    ])
    const parent = await workspace(host)
    const child = await host.openScope({ id: 'chat', kind: 'session', parent })
    const lease = child.pin()
    const report = await parent.close()
    expect(report).toEqual({ failed: [], timedOut: [] })
    expect(events.slice(-2)).toEqual(['close:chat', 'close:alpha'])
    expect(() => lease.snapshot).toThrow('closed')
    await lease.release()
    await host.close()
  })

  it('attempts older cleanups after a stuck or failing effect and reports both', async () => {
    const host = new WorkbenchPluginHost('tenant', 20)
    const cleaned: string[] = []
    await host.setPlugins([
      plugin({
        activate: (ctx) => {
          ctx.defer(() => {
            cleaned.push('oldest')
          })
          ctx.defer(() => {
            throw new Error('release failed')
          })
          ctx.defer(() => new Promise<void>(() => undefined))
        }
      })
    ])
    const scope = await workspace(host)
    const report = await scope.close()
    expect(cleaned).toEqual(['oldest'])
    expect(report.failed.some((error) => error.includes('release failed'))).toBe(true)
    expect(report.timedOut).toContain('effect:2')
    await host.close()
  })

  it('bounds an unresponsive activation and denies late resource registration', async () => {
    const host = new WorkbenchPluginHost('tenant', 15)
    let context: PluginActivationContext | undefined
    await host.setPlugins([
      plugin({
        activate: async (ctx) => {
          context = ctx
          await new Promise<void>(() => undefined)
        }
      })
    ])
    await expect(workspace(host)).rejects.toThrow('activation timed out')
    expect(context?.signal.aborted).toBe(true)
    expect(() => context?.defer(() => undefined)).toThrow('closed')
    await host.close()
  })

  it('rejects foreign parents, workspace rebinding and a wider child ceiling', async () => {
    const host = new WorkbenchPluginHost('tenant')
    const foreign = new WorkbenchPluginHost('other')
    const root = await host.openScope({ id: 'main', kind: 'window', plugins: ['review'] })
    await expect(foreign.openScope({ id: 'workspace', kind: 'workspace', parent: root })).rejects.toThrow('parent')
    await expect(
      host.openScope({ id: 'workspace', kind: 'workspace', parent: root, plugins: ['admin'] })
    ).rejects.toThrow('ceiling')
    const scope = await host.openScope({
      id: 'workspace',
      kind: 'workspace',
      parent: root,
      services: { workspace: { id: 'a', root: '/a' } }
    })
    await expect(
      host.openScope({ id: 'chat', kind: 'session', parent: scope, services: { workspace: { id: 'a', root: '/b' } } })
    ).rejects.toThrow('workspace')
    await host.close()
    await foreign.close()
  })
})
