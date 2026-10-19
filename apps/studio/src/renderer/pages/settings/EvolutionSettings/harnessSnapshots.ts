import { harnessComponents, type HarnessAssembleId } from '@renderer/services/harnessAssemble'

/** Admitted Meta Harness assemble recipes. The saved id is what the next run reads. */

export interface HarnessSnapshot {
  id: HarnessAssembleId
  titleKey: string
  noteKey: string
  scenarioKey: string
  components: readonly string[]
}

export const HARNESS_SNAPSHOTS: readonly HarnessSnapshot[] = [
  {
    id: 'default',
    titleKey: 'settings.evolution.versions.default.title',
    noteKey: 'settings.evolution.versions.default.note',
    scenarioKey: 'settings.evolution.versions.default.scenario',
    components: harnessComponents('default')
  },
  {
    id: 'lean',
    titleKey: 'settings.evolution.versions.lean.title',
    noteKey: 'settings.evolution.versions.lean.note',
    scenarioKey: 'settings.evolution.versions.lean.scenario',
    components: harnessComponents('lean')
  },
  {
    id: 'host',
    titleKey: 'settings.evolution.versions.host.title',
    noteKey: 'settings.evolution.versions.host.note',
    scenarioKey: 'settings.evolution.versions.host.scenario',
    components: harnessComponents('host')
  }
]

const STOCK_IDS = new Set(['system', 'tools', 'budget', 'compact', 'infer'])

export function snapshotById(id: string): HarnessSnapshot {
  return HARNESS_SNAPSHOTS.find((item) => item.id === id) ?? HARNESS_SNAPSHOTS[0]
}

/** Baseline for a Diff. The default assemble, unless that assemble is already on screen. */
export function defaultDiffBase(currentId: string): HarnessSnapshot['id'] {
  const first = HARNESS_SNAPSHOTS[0].id
  if (currentId !== first) return first
  return HARNESS_SNAPSHOTS[1].id
}

/** Stock slots and the builtin host mount report whether this snapshot includes them. */
export function componentMounted(snapshot: HarnessSnapshot, id: string): boolean | undefined {
  if (id === 'intent-stamp') return snapshot.components.includes('host:intent_stamp')
  if (!STOCK_IDS.has(id)) return undefined
  return snapshot.components.includes(id)
}

export function componentOrder(snapshot: HarnessSnapshot, id: string): number | undefined {
  const key = id === 'intent-stamp' ? 'host:intent_stamp' : id
  const index = snapshot.components.indexOf(key)
  return index < 0 ? undefined : index + 1
}
