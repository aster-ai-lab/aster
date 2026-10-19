import { createContext, type ReactNode, use, useMemo, useSyncExternalStore } from 'react'

import { matchesWorkbenchCondition, type WorkbenchPluginScope } from '@renderer/services/plugins'
import type {
  PluginContribution,
  PluginScopeSnapshot,
  PublishedPluginContribution
} from '@renderer/types/workbenchPlugin'

import { usePluginContributionContext } from './PluginContributionContext'

const ScopeContext = createContext<WorkbenchPluginScope | null>(null)
const EMPTY: PluginScopeSnapshot = Object.freeze({ generation: 0, fibers: [], contributions: [] })
const emptySnapshot = () => EMPTY
const noSubscription = () => () => undefined
const EMPTY_CONTRIBUTIONS: readonly PublishedPluginContribution[] = Object.freeze([])
const emptyContributions = () => EMPTY_CONTRIBUTIONS

export function PluginScopeProvider({ scope, children }: { scope: WorkbenchPluginScope | null; children: ReactNode }) {
  return <ScopeContext value={scope}>{children}</ScopeContext>
}

export function usePluginScope(): WorkbenchPluginScope | null {
  return use(ScopeContext)
}

export function usePluginScopeSnapshot(): PluginScopeSnapshot {
  const scope = usePluginScope()
  return useSyncExternalStore(scope?.subscribe ?? noSubscription, scope?.getSnapshot ?? emptySnapshot, emptySnapshot)
}

/** Ancestor composition keeps the original owner and respects the nearest scope's plugin ceiling. */
export function usePluginContributions(slot: PluginContribution['slot'], inherit = false) {
  const scope = usePluginScope()
  const store = useMemo(() => {
    const owners: WorkbenchPluginScope[] = []
    for (let owner = scope; owner; owner = inherit ? (owner.options.parent ?? null) : null) owners.push(owner)
    let snapshots: readonly PluginScopeSnapshot[] = []
    let contributions = EMPTY_CONTRIBUTIONS
    return {
      subscribe: (listener: () => void) => {
        const unsubscribe = owners.map((owner) => owner.subscribe(listener))
        return () => unsubscribe.forEach((release) => release())
      },
      getSnapshot: () => {
        const next = owners.map((owner) => owner.getSnapshot())
        if (next.length === snapshots.length && next.every((value, index) => value === snapshots[index]))
          return contributions
        snapshots = next
        const selected = new Map<string, PublishedPluginContribution>()
        for (const snapshot of next)
          for (const item of snapshot.contributions)
            if (
              scope?.isOpen() &&
              item.contribution.slot === slot &&
              scope.allows(item.pluginId) &&
              !selected.has(item.key)
            )
              selected.set(item.key, item)
        contributions = Object.freeze(
          [...selected.values()].sort(
            (left, right) =>
              (left.contribution.order ?? 100) - (right.contribution.order ?? 100) ||
              (left.key < right.key ? -1 : left.key > right.key ? 1 : 0)
          )
        )
        return contributions
      }
    }
  }, [scope, slot, inherit])
  const contributions = useSyncExternalStore(store.subscribe, store.getSnapshot, emptyContributions)
  const context = usePluginContributionContext(scope)
  return useMemo(
    () => Object.freeze(contributions.filter((item) => matchesWorkbenchCondition(item.contribution.when, context))),
    [contributions, context]
  )
}
