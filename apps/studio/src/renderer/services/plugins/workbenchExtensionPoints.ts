import type {
  PluginContribution,
  PluginScopeKind,
  PluginServiceName,
  WorkbenchPlugin
} from '@renderer/types/workbenchPlugin'
import {
  STUDIO_CONTRIBUTION_PROTOCOL_VERSION,
  studioContributionConditionSchema
} from '@shared/types/studioContribution'
import { STUDIO_EXTENSION_POINT_CONTRACTS } from '@shared/types/studioExtensionPoints.generated'

const SCOPES: readonly PluginScopeKind[] = ['window', 'workspace', 'view', 'session', 'run', 'turn', 'subtask']
const SERVICES: readonly PluginServiceName[] = ['workspace', 'document', 'conversation', 'run', 'turn', 'harness']
const ID = /^[a-z][a-z0-9_.-]{0,127}$/

function extensionPoint<S extends PluginContribution['slot']>(slot: S) {
  return Object.freeze<(typeof STUDIO_EXTENSION_POINT_CONTRACTS)[S] & { readonly slot: S }>({
    ...STUDIO_EXTENSION_POINT_CONTRACTS[slot],
    slot
  })
}

export const WORKBENCH_EXTENSION_POINTS = Object.freeze({
  activity: extensionPoint('activity'),
  page: extensionPoint('page'),
  sidebar: extensionPoint('sidebar'),
  panel: extensionPoint('panel'),
  workspace: extensionPoint('workspace'),
  applet: extensionPoint('applet'),
  composer: extensionPoint('composer'),
  harness: extensionPoint('harness')
})

export function workbenchExtensionPoint(slot: PluginContribution['slot']) {
  if (!Object.hasOwn(WORKBENCH_EXTENSION_POINTS, slot)) throw new Error(`Unknown workbench extension point: ${slot}`)
  return WORKBENCH_EXTENSION_POINTS[slot]
}

function closedList<T extends string>(values: readonly T[], allowed: readonly T[]): boolean {
  return new Set(values).size === values.length && values.every((value) => allowed.includes(value))
}

export function validateWorkbenchPlugin(plugin: WorkbenchPlugin): void {
  if ((plugin.protocolVersion ?? STUDIO_CONTRIBUTION_PROTOCOL_VERSION) !== STUDIO_CONTRIBUTION_PROTOCOL_VERSION)
    throw new Error(`Unsupported plugin contribution protocol: ${plugin.id}`)
  if (
    !plugin.scopes.length ||
    !closedList(plugin.scopes, SCOPES) ||
    !closedList(plugin.services ?? [], SERVICES) ||
    new Set(plugin.requires ?? []).size !== (plugin.requires ?? []).length ||
    (plugin.requires ?? []).some((id) => !ID.test(id))
  )
    throw new Error(`Invalid plugin scope, service or dependency declaration: ${plugin.id}`)
  for (const contribution of plugin.contributions) {
    if (contribution.when !== undefined && !studioContributionConditionSchema.safeParse(contribution.when).success)
      throw new Error(`Invalid contribution condition: ${plugin.id}/${contribution.id}`)
    const point = workbenchExtensionPoint(contribution.slot)
    if (!point.modes.includes(contribution.mode ?? 'append') || 'target' in contribution)
      throw new Error(`Unsupported contribution composition: ${plugin.id}/${contribution.id}`)
    if (
      (contribution.point ?? point.id) !== point.id ||
      (contribution.contractVersion ?? point.contractVersion) !== point.contractVersion
    )
      throw new Error(`Incompatible extension point contract: ${plugin.id}/${contribution.id}`)
    if (
      contribution.order !== undefined &&
      (!Number.isInteger(contribution.order) || contribution.order < -2147483648 || contribution.order > 2147483647)
    )
      throw new Error(`Invalid contribution order: ${plugin.id}/${contribution.id}`)
    if (
      typeof contribution.labelKey !== 'string' ||
      !contribution.labelKey.trim() ||
      contribution.labelKey.length > 128 ||
      (contribution.title !== undefined &&
        (typeof contribution.title !== 'string' || !contribution.title.trim() || [...contribution.title].length > 64))
    )
      throw new Error(`Invalid contribution label: ${plugin.id}/${contribution.id}`)
    if (
      (contribution.scopes !== undefined &&
        (!contribution.scopes.length || !closedList(contribution.scopes, plugin.scopes))) ||
      !closedList(contribution.services ?? [], SERVICES)
    )
      throw new Error(`Invalid contribution scope or service: ${plugin.id}/${contribution.id}`)
    if (
      contribution.slot === 'applet' &&
      (contribution.applicationId !== undefined || contribution.sidebarId !== undefined)
    )
      throw new Error(`Mini app cannot claim an ActivityBar identity: ${plugin.id}/${contribution.id}`)
  }
}

export function compareContributionKeys(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0
}
