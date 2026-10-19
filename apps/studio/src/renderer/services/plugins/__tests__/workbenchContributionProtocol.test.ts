import { describe, expect, it } from 'vitest'

import type { PluginContribution, WorkbenchPlugin } from '@renderer/types/workbenchPlugin'

import { WorkbenchPluginHost } from '../workbenchPluginHost'

const View = () => null
const page = { id: 'page', slot: 'page', labelKey: 'plugins.title', component: View } as const
function definition(overrides: Partial<WorkbenchPlugin> = {}): WorkbenchPlugin {
  return {
    id: 'proof',
    generation: 1,
    protocolVersion: 1,
    scopes: ['window', 'workspace'],
    contributions: [page],
    ...overrides
  }
}

describe('workbench contribution protocol', () => {
  // Catch caller mutations changing admitted conditions without a new immutable generation.
  it('retains admitted conditions independently of mutable factory inputs', async () => {
    const host = new WorkbenchPluginHost('condition-owner')
    const all: ('workspace' | 'run')[] = ['workspace']
    const when = { all, theme: 'dark' as const }
    await host.setPlugins([definition({ contributions: [{ ...page, when }] })])
    const scope = await host.openScope({ id: 'main', kind: 'window' })
    all[0] = 'run'
    expect(scope.getSnapshot().contributions[0].contribution.when?.all).toEqual(['workspace'])
    await expect(host.setPlugins([definition({ contributions: [{ ...page, when }] })])).rejects.toThrow('immutable')
    await host.close()
  })

  // Catch unsupported contracts activating extension code or replacing a valid publication.
  it('refuses malformed contracts before activation and preserves the current scope', async () => {
    const host = new WorkbenchPluginHost('protocol-owner')
    await host.setPlugins([definition()])
    const scope = await host.openScope({ id: 'main', kind: 'window' })
    const current = scope.getSnapshot()
    let activated = false
    const badContributions: readonly unknown[] = [
      { ...page, point: 'studio.workspace' },
      { ...page, contractVersion: 2 },
      { ...page, mode: 'replace' },
      { ...page, target: 'undeclared.target' },
      { ...page, slot: 'unknown' },
      { ...page, order: NaN },
      { ...page, order: Infinity },
      { ...page, order: 2147483648 },
      { ...page, scopes: ['run'] },
      { ...page, services: ['unknown'] },
      { ...page, labelKey: '' },
      { ...page, labelKey: 1 },
      { ...page, title: 1 },
      { ...page, title: '   ' },
      ...[
        {},
        { all: ['unknown'] },
        { all: ['run', 'run'] },
        { all: ['run'], none: ['run'] },
        { theme: 'system' },
        { locale: '../en' },
        { locale: '' },
        { theme: 'dark', command: 'execute' }
      ].map((when) => ({ ...page, when }))
    ]
    for (const bad of badContributions) {
      await expect(
        host.setPlugins([
          definition({
            generation: 2,
            activate: () => {
              activated = true
            },
            contributions: [bad] as PluginContribution[]
          })
        ])
      ).rejects.toThrow()
      expect(scope.getSnapshot()).toBe(current)
    }
    for (const overrides of [
      { protocolVersion: 2 },
      { scopes: [] },
      { scopes: ['window', 'window'] },
      { services: ['unknown'] },
      { requires: ['proof', 'proof'] }
    ]) {
      await expect(
        host.setPlugins([
          definition({
            ...overrides,
            generation: 2,
            activate: () => {
              activated = true
            }
          } as Partial<WorkbenchPlugin>)
        ])
      ).rejects.toThrow()
    }
    expect(activated).toBe(false)
    await host.close()
  })

  // Catch native workspace views appearing globally or executing without their required observations.
  it('publishes a contribution only in its declared scope with its required services', async () => {
    const host = new WorkbenchPluginHost('scope-owner')
    await host.setPlugins([
      definition({
        contributions: [
          page,
          {
            ...page,
            id: 'workspace',
            slot: 'workspace',
            point: 'studio.workspace',
            scopes: ['workspace'],
            services: ['workspace']
          }
        ]
      })
    ])
    const window = await host.openScope({ id: 'main', kind: 'window' })
    expect(window.getSnapshot().contributions.map((item) => item.key)).toEqual(['proof/page'])
    const unavailable = await host.openScope({ id: 'no-context', kind: 'workspace', parent: window })
    expect(unavailable.getSnapshot().diagnostics).toEqual([
      { key: 'proof/workspace', point: 'studio.workspace', reason: 'service-unavailable', missing: ['workspace'] }
    ])
    expect(unavailable.getSnapshot().contributions.map((item) => item.key)).toEqual(['proof/page'])
    const available = await host.openScope({
      id: 'project',
      kind: 'workspace',
      parent: window,
      services: { workspace: { id: 'project', root: '/project' } }
    })
    const view = available.getSnapshot().contributions.find((item) => item.key === 'proof/workspace')!
    expect(view.point).toBe('studio.workspace')
    expect(view.contractVersion).toBe(1)
    expect(view.context.services.workspace?.id).toBe('project')
    await host.close()
    expect(view.context.isCurrent()).toBe(false)
  })

  // Catch another package's page or a reserved built-in identity being exposed as a mini app.
  it('admits mini apps through their own point and requires an owned page', async () => {
    const host = new WorkbenchPluginHost('miniapp-owner')
    const launcher = { id: 'app', slot: 'applet', labelKey: 'plugins.title', pageId: 'page' } as const
    await expect(host.setPlugins([definition({ contributions: [launcher] })])).rejects.toThrow('local page')
    await expect(
      host.setPlugins([definition({ contributions: [page, { ...launcher, applicationId: 'plugins' }] })])
    ).rejects.toThrow('ActivityBar identity')
    await host.setPlugins([definition({ contributions: [page, launcher] })])
    const scope = await host.openScope({ id: 'main', kind: 'window' })
    expect(scope.getSnapshot().contributions.find((item) => item.key === 'proof/app')?.point).toBe('studio.applets')
    await host.close()
  })

  // Catch local-ID collisions being accepted, cross-package names colliding, or hidden packages leaking diagnostics.
  it('scopes local contribution identities by package and hides excluded package diagnostics', async () => {
    const host = new WorkbenchPluginHost('identity-owner')
    await expect(
      host.setPlugins([definition({ contributions: [page, { ...page, slot: 'panel' }] as PluginContribution[] })])
    ).rejects.toThrow('Duplicate or invalid contribution')

    await host.setPlugins([
      definition(),
      definition({
        id: 'secret.package',
        contributions: [{ ...page, slot: 'workspace', point: 'studio.workspace', services: ['workspace'] }]
      })
    ])
    const visible = await host.openScope({ id: 'visible-only', kind: 'window', plugins: ['proof'] })
    expect(visible.getSnapshot().contributions.map((item) => item.key)).toEqual(['proof/page'])
    expect(visible.getSnapshot().diagnostics).toEqual([])
    const bothPackages = await host.openScope({
      id: 'both-packages',
      kind: 'window',
      plugins: ['proof', 'secret.package']
    })
    const sameNames = await host.openScope({
      id: 'same-local-name',
      kind: 'workspace',
      parent: bothPackages,
      services: { workspace: { id: 'project', root: '/project' } }
    })
    expect(sameNames.getSnapshot().contributions.map((item) => item.key)).toEqual(['proof/page', 'secret.package/page'])
    await host.close()
  })

  // Catch equal-priority order changing with installation order or runtime locale.
  it('orders contributions deterministically and rejects same-generation scope mutations', async () => {
    const host = new WorkbenchPluginHost('order-owner')
    const plugin = definition({
      contributions: [
        { ...page, id: 'z', order: 1 },
        { ...page, id: 'a_', order: 0 },
        { ...page, id: 'a-', order: 0 }
      ]
    })
    await host.setPlugins([plugin])
    const scope = await host.openScope({ id: 'main', kind: 'window' })
    expect(scope.getSnapshot().contributions.map((item) => item.key)).toEqual(['proof/a-', 'proof/a_', 'proof/z'])
    await expect(
      host.setPlugins([
        { ...plugin, contributions: plugin.contributions.map((item) => ({ ...item, scopes: ['window'] })) }
      ])
    ).rejects.toThrow('immutable')
    expect(scope.getSnapshot().contributions[0].point).toBe('studio.editor')
    await host.close()
  })
})
