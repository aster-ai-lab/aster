import { preferenceService } from '@data/PreferenceService'
import type { HarnessAssemblePreference } from '@shared/data/preference/preferenceTypes'

export const HARNESS_ASSEMBLE_KEY = 'app.harness.assemble'

export const HARNESS_ASSEMBLE_IDS = ['default', 'lean', 'host'] as const

export type HarnessAssembleId = HarnessAssemblePreference

const COMPONENTS: Record<HarnessAssembleId, readonly string[]> = {
  default: ['system', 'tools', 'budget', 'compact', 'infer'],
  lean: ['system', 'tools', 'budget', 'infer'],
  host: ['system', 'tools', 'host:intent_stamp', 'budget', 'compact', 'infer']
}

export function harnessComponents(id: HarnessAssembleId): readonly string[] {
  return COMPONENTS[id]
}

export function readHarnessAssembleId(): HarnessAssembleId {
  const stored = preferenceService.getCachedValue(HARNESS_ASSEMBLE_KEY)
  return isHarnessAssembleId(stored) ? stored : 'default'
}

export async function writeHarnessAssembleId(id: HarnessAssembleId): Promise<void> {
  await preferenceService.set(HARNESS_ASSEMBLE_KEY, id, { optimistic: false })
}

export function subscribeHarnessAssemble(listener: () => void): () => void {
  void preferenceService.get(HARNESS_ASSEMBLE_KEY)
  return preferenceService.subscribeChange(HARNESS_ASSEMBLE_KEY)(listener)
}

export async function withHarnessComponents<T extends object>(
  body: T & { harnessComponents?: string[] }
): Promise<T & { harnessComponents: string[] }> {
  if (body.harnessComponents !== undefined) return { ...body, harnessComponents: [...body.harnessComponents] }
  const id = await preferenceService.get(HARNESS_ASSEMBLE_KEY)
  if (!isHarnessAssembleId(id) || preferenceService.getCachedValue(HARNESS_ASSEMBLE_KEY) === undefined)
    throw new Error('Harness preference is unavailable')
  return { ...body, harnessComponents: [...harnessComponents(id)] }
}

export function isHarnessAssembleId(value: unknown): value is HarnessAssembleId {
  return HARNESS_ASSEMBLE_IDS.some((id) => id === value)
}
