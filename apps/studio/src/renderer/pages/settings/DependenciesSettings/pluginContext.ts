/**
 * Plugin context shaped after Cordis (cordiverse/cordis).
 *
 * Spatial composability: one context holds services. A plugin declares the
 * services it injects. It stays inactive until every one of them is present.
 * A missing dependency is not a failure.
 *
 * Temporal composability: an active plugin's effects are registered in order.
 * Removing the plugin drops the service it provided and reverts those effects
 * from last to first. Dependents that injected the dropped service go inactive.
 */

export const HOST_SERVICE = 'workspace'

export interface PluginRecord {
  name: string
  description: string
  source: 'installed' | 'user'
}

export interface PluginContract {
  inject: readonly string[]
  provide: readonly string[]
  effects: readonly string[]
}

export interface FiberView {
  name: string
  description: string
  source: 'installed' | 'user'
  state: 'active' | 'inactive'
  inject: readonly string[]
  missing: readonly string[]
  provide: readonly string[]
  effects: readonly string[]
}

export interface ContextView {
  services: readonly string[]
  fibers: readonly FiberView[]
}

const CONTRACTS: Record<string, PluginContract> = {
  review: {
    inject: ['document', 'conversation'],
    provide: ['review'],
    effects: ['applet', 'command', 'findings', 'annotation']
  }
}

export function pluginContract(name: string): PluginContract {
  return CONTRACTS[name] ?? { inject: [], provide: [name], effects: [] }
}

/** Reconcile the installed set into one context. Session services come and go. */
export function reconcileContext(
  plugins: readonly PluginRecord[],
  session: readonly string[],
  contracts: Readonly<Record<string, PluginContract>> = {}
): ContextView {
  const contractFor = (name: string) => contracts[name] ?? pluginContract(name)
  const available = new Set<string>([HOST_SERVICE, ...session])
  const active = new Set<string>()
  let progressed = true
  while (progressed) {
    progressed = false
    for (const plugin of plugins) {
      if (active.has(plugin.name)) continue
      const contract = contractFor(plugin.name)
      if (contract.inject.some((key) => !available.has(key))) continue
      active.add(plugin.name)
      for (const key of contract.provide) available.add(key)
      progressed = true
    }
  }

  const fibers = plugins.map<FiberView>((plugin) => {
    const contract = contractFor(plugin.name)
    const state = active.has(plugin.name) ? 'active' : 'inactive'
    return {
      name: plugin.name,
      description: plugin.description,
      source: plugin.source,
      state,
      inject: contract.inject,
      missing: contract.inject.filter((key) => !available.has(key)),
      provide: contract.provide,
      effects: state === 'active' ? contract.effects : []
    }
  })

  return {
    services: [...available].sort(),
    fibers
  }
}

export function reviewFiberActive(context: ContextView): boolean {
  return context.fibers.some((fiber) => fiber.name === 'review' && fiber.state === 'active')
}
