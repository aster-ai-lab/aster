import { loggerService } from '@logger'
import type {
  PluginCloseReport,
  PluginScopeKind,
  PluginScopeSnapshot,
  PluginServiceSnapshot,
  WorkbenchPlugin
} from '@renderer/types/workbenchPlugin'
import { SIDEBAR_FAVORITES } from '@shared/data/preference/preferenceTypes'
import { studioExecutionObservationSchema } from '@shared/types/studioExecution'

import { PluginEffects } from './pluginEffects'
import { compareContributionKeys, validateWorkbenchPlugin, workbenchExtensionPoint } from './workbenchExtensionPoints'

const EMPTY: PluginScopeSnapshot = Object.freeze({ generation: 0, fibers: [], contributions: [] })
const logger = loggerService.withContext('WorkbenchPluginHost')
const runServiceSchema = studioExecutionObservationSchema.pick({ coreSessionId: true, runId: true, promptId: true })
const CHILD_KINDS: Record<PluginScopeKind, readonly PluginScopeKind[]> = {
  window: ['workspace', 'view'],
  workspace: ['session', 'view'],
  view: [],
  session: ['run', 'view'],
  run: ['turn', 'subtask', 'view'],
  turn: ['subtask', 'view'],
  subtask: ['turn', 'subtask', 'view']
}

function sameDefinition(left: WorkbenchPlugin, right: WorkbenchPlugin): boolean {
  if (left.activate !== right.activate || left.contributions.length !== right.contributions.length) return false
  const metadata = (plugin: WorkbenchPlugin) =>
    JSON.stringify({
      protocolVersion: plugin.protocolVersion ?? 1,
      scopes: [...plugin.scopes].sort(),
      requires: [...(plugin.requires ?? [])].sort(),
      services: [...(plugin.services ?? [])].sort(),
      contributions: plugin.contributions.map((item) => ({
        id: item.id,
        slot: item.slot,
        labelKey: item.labelKey,
        title: item.title,
        order: item.order,
        point: item.point,
        contractVersion: item.contractVersion,
        mode: item.mode ?? 'append',
        scopes: item.scopes && [...item.scopes].sort(),
        services: item.services && [...item.services].sort(),
        when: item.when && {
          ...item.when,
          ...(item.when.all ? { all: [...item.when.all].sort() } : {}),
          ...(item.when.none ? { none: [...item.when.none].sort() } : {})
        },
        ...(item.slot === 'activity' || item.slot === 'applet'
          ? {
              pageId: item.pageId,
              sidebarId: item.sidebarId,
              applicationId: item.applicationId,
              description: item.description
            }
          : {}),
        ...(item.slot === 'harness' ? { components: item.components } : {})
      }))
    })
  if (metadata(left) !== metadata(right)) return false
  return left.contributions.every((item, index) => {
    const other = right.contributions[index]
    if (item.slot === 'composer') return other.slot === 'composer' && item.invoke === other.invoke
    if (item.slot === 'activity' || item.slot === 'applet') return other.slot === item.slot && item.icon === other.icon
    if ('component' in item) return 'component' in other && item.component === other.component
    return true
  })
}

function orderedPlugins(plugins: readonly WorkbenchPlugin[]): readonly WorkbenchPlugin[] {
  if (plugins.length > 256) throw new Error('Plugin catalog limit exceeded')
  const byId = new Map<string, WorkbenchPlugin>()
  const applications = new Set<string>()
  for (const plugin of plugins) {
    validateWorkbenchPlugin(plugin)
    if (
      !/^[a-z][a-z0-9_.-]{0,127}$/.test(plugin.id) ||
      !Number.isSafeInteger(plugin.generation) ||
      plugin.generation < 1
    ) {
      throw new Error('Invalid plugin identity or generation')
    }
    if (byId.has(plugin.id)) throw new Error(`Duplicate plugin: ${plugin.id}`)
    const ids = new Set<string>()
    for (const contribution of plugin.contributions) {
      if (contribution.slot === 'activity' && contribution.applicationId !== undefined) {
        const id = contribution.applicationId
        if (!SIDEBAR_FAVORITES.includes(id) || plugin.id !== `studio.${id}` || applications.has(id))
          throw new Error(`Invalid or duplicate built-in activity identity: ${id}`)
        applications.add(id)
      }
      if (!/^[a-z][a-z0-9_.-]{0,127}$/.test(contribution.id) || ids.has(contribution.id)) {
        throw new Error(`Duplicate or invalid contribution: ${plugin.id}/${contribution.id}`)
      }
      ids.add(contribution.id)
    }
    for (const contribution of plugin.contributions) {
      if (
        (contribution.slot === 'activity' || contribution.slot === 'applet') &&
        !plugin.contributions.some((item) => item.slot === 'page' && item.id === contribution.pageId)
      ) {
        throw new Error(`Plugin activity requires a local page: ${plugin.id}/${contribution.id}`)
      }
      if (
        contribution.slot === 'activity' &&
        contribution.sidebarId !== undefined &&
        !plugin.contributions.some((item) => item.slot === 'sidebar' && item.id === contribution.sidebarId)
      ) {
        throw new Error(`Plugin activity requires an owned sidebar: ${plugin.id}/${contribution.id}`)
      }
    }
    if (plugin.contributions.length > 256) throw new Error('Plugin contribution limit exceeded')
    byId.set(plugin.id, plugin)
  }
  const ordered: WorkbenchPlugin[] = []
  const visiting = new Set<string>()
  const visited = new Set<string>()
  const visit = (plugin: WorkbenchPlugin) => {
    if (visiting.has(plugin.id)) throw new Error(`Plugin dependency cycle: ${plugin.id}`)
    if (visited.has(plugin.id)) return
    visiting.add(plugin.id)
    for (const id of [...(plugin.requires ?? [])].sort()) {
      const dependency = byId.get(id)
      if (dependency) visit(dependency)
    }
    visiting.delete(plugin.id)
    visited.add(plugin.id)
    ordered.push(plugin)
  }
  for (const plugin of [...byId.values()].sort((a, b) => compareContributionKeys(a.id, b.id))) visit(plugin)
  return ordered.map((plugin) =>
    Object.freeze({
      ...plugin,
      requires: Object.freeze([...(plugin.requires ?? [])]),
      services: Object.freeze([...(plugin.services ?? [])]),
      scopes: Object.freeze([...plugin.scopes]),
      contributions: Object.freeze(
        plugin.contributions.map((item) =>
          Object.freeze({
            ...item,
            ...(item.slot === 'harness' ? { components: Object.freeze([...item.components]) } : {}),
            ...(item.scopes ? { scopes: Object.freeze([...item.scopes]) } : {}),
            ...(item.services ? { services: Object.freeze([...item.services]) } : {}),
            ...(item.when
              ? {
                  when: Object.freeze({
                    ...item.when,
                    ...(item.when.all ? { all: Object.freeze([...item.when.all]) } : {}),
                    ...(item.when.none ? { none: Object.freeze([...item.when.none]) } : {})
                  })
                }
              : {})
          })
        )
      )
    })
  )
}

function mergeServices(parent: PluginServiceSnapshot, local: PluginServiceSnapshot): PluginServiceSnapshot {
  const conversation = local.conversation ?? parent.conversation
  if (conversation && (typeof conversation.sessionId !== 'string' || !conversation.sessionId.trim()))
    throw new Error('Plugin conversation identity is required')
  if (
    parent.workspace &&
    local.workspace &&
    (parent.workspace.id !== local.workspace.id || parent.workspace.root !== local.workspace.root)
  ) {
    throw new Error('Child scope cannot replace the parent workspace')
  }
  if (parent.conversation && local.conversation && parent.conversation.sessionId !== local.conversation.sessionId) {
    throw new Error('Child scope cannot replace the parent conversation')
  }
  const run = local.run ?? parent.run
  const turn = local.turn ?? parent.turn
  if (run && !runServiceSchema.safeParse(run).success) throw new Error('Plugin Run identity is required')
  if (turn && (!Number.isSafeInteger(turn.modelTurn) || turn.modelTurn < 1))
    throw new Error('Plugin model turn requires a positive ordinal')
  if (
    parent.run &&
    local.run &&
    (parent.run.runId !== local.run.runId ||
      parent.run.coreSessionId !== local.run.coreSessionId ||
      parent.run.promptId !== local.run.promptId)
  )
    throw new Error('Child scope cannot replace the parent Run')
  if (parent.turn && local.turn && parent.turn.modelTurn !== local.turn.modelTurn)
    throw new Error('Child scope cannot replace the parent model turn')
  return Object.freeze(
    Object.fromEntries(
      Object.entries({
        ...parent,
        ...Object.fromEntries(Object.entries(local).filter(([, value]) => value !== undefined))
      })
        .filter(([, value]) => value !== undefined)
        .map(([key, value]) => [
          key,
          Object.freeze({
            ...value,
            ...('components' in value ? { components: Object.freeze([...value.components]) } : {})
          })
        ])
    )
  )
}

interface PluginActivation {
  readonly plugin: WorkbenchPlugin
  readonly context: PluginEffects
  readonly dependencies: readonly PluginActivation[]
  references: number
}

class PublishedGeneration {
  leases = 0
  retired = false
  revoked = false
  closing?: Promise<PluginCloseReport>

  constructor(
    readonly snapshot: PluginScopeSnapshot,
    readonly activations: readonly PluginActivation[],
    private readonly timeoutMs: number,
    private readonly onClosed: (report: PluginCloseReport) => void
  ) {
    for (const activation of activations) activation.references++
  }

  close(deadline = Date.now() + this.timeoutMs): Promise<PluginCloseReport> {
    if (!this.closing) {
      this.cancel()
      this.closing = (async () => {
        const failed: string[] = []
        const timedOut: string[] = []
        for (const activation of [...this.activations].reverse()) {
          activation.references--
          if (activation.references !== 0) continue
          const report = await activation.context.close(deadline)
          failed.push(...report.failed)
          timedOut.push(...report.timedOut)
        }
        const report = Object.freeze({ failed: Object.freeze(failed), timedOut: Object.freeze(timedOut) })
        this.onClosed(report)
        return report
      })()
    }
    return this.closing
  }

  cancel(force = false): void {
    this.revoked = true
    for (const activation of this.activations) {
      if (force || activation.references === 1) activation.context.cancel()
    }
  }

  retire(): Promise<PluginCloseReport> | undefined {
    this.retired = true
    if (this.leases === 0) return this.close()
    return undefined
  }
}

interface PreparedPluginScope {
  canPublish(): boolean
  publish(): void
  announce(): void
  retire(): Promise<PluginCloseReport> | undefined
  abort(): Promise<PluginCloseReport>
}

/** A UI lease pins contributions, not Code execution authority or a Use route. */
export class PluginContributionLease {
  private released = false

  constructor(
    private readonly generation: PublishedGeneration,
    private readonly ownerActive: () => boolean
  ) {
    if (generation.leases >= 256) throw new Error('Plugin contribution lease limit exceeded')
    generation.leases++
  }

  get snapshot(): PluginScopeSnapshot {
    if (this.released || this.generation.revoked || this.generation.closing || !this.ownerActive())
      throw new Error('Plugin contribution lease is closed')
    return this.generation.snapshot
  }

  async release(): Promise<void> {
    if (this.released) return
    this.released = true
    this.generation.leases--
    if (this.generation.retired && this.generation.leases === 0) await this.generation.close()
  }
}

export interface OpenPluginScope {
  readonly id: string
  readonly kind: PluginScopeKind
  readonly parent?: WorkbenchPluginScope
  readonly services?: PluginServiceSnapshot
  readonly plugins?: readonly string[]
  /** Cancels opening only; the caller owns closing the returned scope. */
  readonly signal?: globalThis.AbortSignal
}

export class WorkbenchPluginScope {
  readonly children = new Set<WorkbenchPluginScope>()
  readonly services: PluginServiceSnapshot
  private readonly listeners = new Set<() => void>()
  private readonly generations = new Set<PublishedGeneration>()
  private readonly preparing = new Set<PluginEffects>()
  private generation?: PublishedGeneration
  private serial: Promise<void> = Promise.resolve()
  private closing?: Promise<PluginCloseReport>
  private stopped = false
  private revision = 0
  private readonly reports: PluginCloseReport[] = []

  constructor(
    readonly host: WorkbenchPluginHost,
    readonly options: OpenPluginScope,
    readonly id: string,
    private readonly timeoutMs: number
  ) {
    this.services = mergeServices(options.parent?.services ?? {}, options.services ?? {})
    if (this.services.turn && !this.services.run) throw new Error('Plugin model turn requires an admitted Run')
  }

  isOpen(): boolean {
    return !this.stopped && (!this.options.parent || this.options.parent.isOpen())
  }

  allows(plugin: string): boolean {
    return (
      (!this.options.plugins || this.options.plugins.includes(plugin)) &&
      (!this.options.parent || this.options.parent.allows(plugin))
    )
  }

  readonly getSnapshot = (): PluginScopeSnapshot => (this.isOpen() ? (this.generation?.snapshot ?? EMPTY) : EMPTY)
  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  pin(): PluginContributionLease {
    if (!this.isOpen() || !this.generation) throw new Error('Plugin scope is not active')
    return new PluginContributionLease(this.generation, () => this.isOpen())
  }

  prepareReplacement(plugins: readonly WorkbenchPlugin[]): Promise<PreparedPluginScope> {
    const update = this.serial.then(() => this.prepare(plugins))
    this.serial = update.then(
      () => undefined,
      () => undefined
    )
    return update
  }

  private async prepare(plugins: readonly WorkbenchPlugin[]): Promise<PreparedPluginScope> {
    if (this.stopped) throw new Error('Plugin scope is closed')
    if (this.reports.length >= 256) throw new Error('Plugin cleanup failure budget exhausted')
    if (this.generations.size >= 32) throw new Error('Retained plugin generation limit exceeded')
    const effects: PluginEffects[] = []
    const fibers: PluginScopeSnapshot['fibers'][number][] = []
    const contributions: PluginScopeSnapshot['contributions'][number][] = []
    const diagnostics: NonNullable<PluginScopeSnapshot['diagnostics']>[number][] = []
    const active = new Map<string, PluginActivation>()
    const activations: PluginActivation[] = []
    let candidate: PublishedGeneration | undefined
    try {
      for (const plugin of plugins) {
        if (!this.allows(plugin.id) || !plugin.scopes.includes(this.options.kind)) continue
        const missing = [
          ...(plugin.requires ?? []).filter((id) => !active.has(id)).map((id) => `plugin:${id}`),
          ...(plugin.services ?? []).filter((name) => !this.services[name]).map((name) => `service:${name}`)
        ]
        fibers.push(
          Object.freeze({
            id: plugin.id,
            generation: plugin.generation,
            state: missing.length ? 'blocked' : 'active',
            missing: Object.freeze(missing)
          })
        )
        if (missing.length) continue
        const dependencies = (plugin.requires ?? []).map((id) => active.get(id)!)
        const previous = this.generation?.activations.find((item) => item.plugin.id === plugin.id)
        let activation: PluginActivation
        if (
          previous &&
          previous.plugin.generation === plugin.generation &&
          sameDefinition(previous.plugin, plugin) &&
          previous.dependencies.length === dependencies.length &&
          previous.dependencies.every((item, index) => item === dependencies[index])
        ) {
          activation = previous
        } else {
          const context = new PluginEffects(
            this.id,
            this.services,
            () => this.isOpen() && this.generation?.activations.includes(activation) === true,
            this.timeoutMs
          )
          activation = { plugin, context, dependencies, references: 0 }
          effects.push(context)
          this.preparing.add(context)
          await context.activate(plugin.activate)
        }
        if (this.stopped) throw new Error('Plugin scope closed during activation')
        const context = activation.context
        activations.push(activation)
        active.set(plugin.id, activation)
        for (const contribution of plugin.contributions) {
          if (contribution.scopes && !contribution.scopes.includes(this.options.kind)) continue
          const point = workbenchExtensionPoint(contribution.slot)
          const missing = (contribution.services ?? []).filter((name) => !this.services[name])
          if (missing.length) {
            diagnostics.push(
              Object.freeze({
                key: `${plugin.id}/${contribution.id}`,
                point: point.id,
                reason: 'service-unavailable',
                missing: Object.freeze(missing)
              })
            )
            continue
          }
          contributions.push(
            Object.freeze({
              key: `${plugin.id}/${contribution.id}`,
              pluginId: plugin.id,
              pluginGeneration: plugin.generation,
              point: point.id,
              contractVersion: point.contractVersion,
              contribution,
              context
            })
          )
        }
      }
      contributions.sort(
        (a, b) => (a.contribution.order ?? 0) - (b.contribution.order ?? 0) || compareContributionKeys(a.key, b.key)
      )
      const snapshot = Object.freeze({
        generation: this.revision + 1,
        fibers: Object.freeze(fibers),
        diagnostics: Object.freeze(diagnostics),
        contributions: Object.freeze(contributions)
      })
      candidate = new PublishedGeneration(snapshot, activations, this.timeoutMs, (report) => {
        if (report.failed.length || report.timedOut.length) this.reports.push(report)
        if (candidate) this.generations.delete(candidate)
      })
      this.generations.add(candidate)
      const previous = this.generation
      for (const context of effects) this.preparing.delete(context)
      const prepared = candidate
      return {
        canPublish: () => this.isOpen() && !prepared.revoked && !prepared.closing,
        publish: () => {
          this.generation = prepared
          this.revision = snapshot.generation
        },
        announce: () => {
          for (const context of effects) context.start()
          this.notify()
        },
        retire: () => previous?.retire(),
        abort: () => prepared.close()
      }
    } catch (error) {
      const deadline = Date.now() + this.timeoutMs
      for (const context of effects) context.cancel()
      for (const context of [...effects].reverse()) {
        this.preparing.delete(context)
        const report = await context.close(deadline)
        if (report.failed.length || report.timedOut.length) this.reports.push(report)
      }
      throw error
    }
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener()
      } catch (error) {
        logger.error('Plugin contribution observer failed', error instanceof Error ? error : new Error(String(error)))
      }
    }
  }

  close(deadline = Date.now() + this.timeoutMs): Promise<PluginCloseReport> {
    if (!this.closing) {
      this.closing = Promise.resolve().then(() => this.cleanup(deadline))
      this.cancel()
    }
    return this.closing
  }

  private cancel(): void {
    if (this.stopped) return
    this.stopped = true
    for (const effect of this.preparing) effect.cancel()
    for (const generation of this.generations) generation.cancel(true)
    for (const child of this.children) child.cancel()
    this.notify()
  }

  private async cleanup(deadline: number): Promise<PluginCloseReport> {
    await this.serial
    for (const child of [...this.children].reverse()) this.reports.push(await child.close(deadline))
    for (const generation of [...this.generations].reverse()) await generation.close(deadline)
    this.generation = undefined
    this.notify()
    this.listeners.clear()
    this.options.parent?.children.delete(this)
    this.host.release(this)
    return Object.freeze({
      failed: Object.freeze(this.reports.flatMap((report) => report.failed)),
      timedOut: Object.freeze(this.reports.flatMap((report) => report.timedOut))
    })
  }
}

export class WorkbenchPluginHost {
  private plugins: readonly WorkbenchPlugin[] = []
  private readonly history = new Map<string, WorkbenchPlugin>()
  private readonly scopes = new Map<string, WorkbenchPluginScope>()
  private stopped = false
  private serial: Promise<void> = Promise.resolve()
  private closing?: Promise<PluginCloseReport>

  constructor(
    readonly tenantId: string,
    private readonly timeoutMs = 2000
  ) {
    if (!tenantId || !Number.isFinite(timeoutMs) || timeoutMs <= 0) throw new Error('Invalid plugin host')
  }

  private enqueue<T>(work: () => Promise<T>): Promise<T> {
    const next = this.serial.then(() => {
      if (this.stopped) throw new Error('Plugin host is closed')
      return work()
    })
    this.serial = next.then(
      () => undefined,
      () => undefined
    )
    return next
  }

  async setPlugins(plugins: readonly WorkbenchPlugin[]): Promise<void> {
    const ordered = orderedPlugins(plugins)
    return this.enqueue(() => this.publishPlugins(ordered))
  }

  private async publishPlugins(ordered: readonly WorkbenchPlugin[]): Promise<void> {
    if (new Set([...this.history.keys(), ...ordered.map((plugin) => plugin.id)]).size > 1024)
      throw new Error('Plugin identity history limit exceeded')
    for (const plugin of ordered) {
      const previous = this.history.get(plugin.id)
      if (
        previous &&
        (plugin.generation < previous.generation ||
          (plugin.generation === previous.generation && !sameDefinition(previous, plugin)))
      ) {
        throw new Error(`Plugin generation is immutable: ${plugin.id}`)
      }
    }
    const results = await Promise.allSettled(
      [...this.scopes.values()].filter((scope) => scope.isOpen()).map((scope) => scope.prepareReplacement(ordered))
    )
    const prepared = results.flatMap((result) => (result.status === 'fulfilled' ? [result.value] : []))
    const failed = results.find((result) => result.status === 'rejected')
    if (failed || this.stopped || prepared.some((item) => !item.canPublish())) {
      for (const item of [...prepared].reverse()) await item.abort()
      if (failed?.status === 'rejected') throw failed.reason
      throw new Error('Plugin publication cancelled')
    }
    // No callbacks or awaits between swaps: every observer sees one complete catalog.
    this.plugins = ordered
    for (const plugin of ordered) this.history.set(plugin.id, plugin)
    for (const item of prepared) item.publish()
    for (const item of prepared) item.announce()
    await Promise.all(
      prepared.map(async (item) => {
        await item.retire()
      })
    )
  }

  async openScope(options: OpenPluginScope): Promise<WorkbenchPluginScope> {
    const captured = Object.freeze({
      ...options,
      services: mergeServices({}, options.services ?? {}),
      plugins: options.plugins ? Object.freeze([...options.plugins]) : undefined
    })
    return this.enqueue(() => this.activateScope(captured))
  }

  private async activateScope(options: OpenPluginScope): Promise<WorkbenchPluginScope> {
    if (options.signal?.aborted) throw new Error('Plugin scope activation cancelled')
    if (this.scopes.size >= 256) throw new Error('Plugin scope limit exceeded')
    let depth = 0
    for (let ancestor = options.parent; ancestor; ancestor = ancestor.options.parent) depth++
    if (depth >= 32) throw new Error('Plugin scope depth limit exceeded')
    if (!/^[a-zA-Z0-9_.-]{1,128}$/.test(options.id)) throw new Error('Invalid plugin scope identity')
    const parent = options.parent
    if (
      parent &&
      (parent.host !== this || !parent.isOpen() || !CHILD_KINDS[parent.options.kind].includes(options.kind))
    ) {
      throw new Error('Invalid plugin scope parent')
    }
    if (!parent && options.kind !== 'window') throw new Error('Plugin root must be a window')
    if (options.plugins?.some((id) => parent && !parent.allows(id)))
      throw new Error('Child plugin ceiling exceeds parent')
    const id = `${parent?.id ?? this.tenantId}/${options.kind}/${options.id}`
    if (this.scopes.has(id)) throw new Error('Duplicate plugin scope')
    const scope = new WorkbenchPluginScope(
      this,
      Object.freeze({ ...options, plugins: options.plugins ? Object.freeze([...options.plugins]) : undefined }),
      id,
      this.timeoutMs
    )
    this.scopes.set(id, scope)
    parent?.children.add(scope)
    const cancel = () => {
      void scope.close()
    }
    options.signal?.addEventListener('abort', cancel, { once: true })
    try {
      const prepared = await scope.prepareReplacement(this.plugins)
      if (this.stopped || !prepared.canPublish()) {
        await prepared.abort()
        throw new Error('Plugin scope activation cancelled')
      }
      prepared.publish()
      prepared.announce()
      await prepared.retire()
      return scope
    } catch (error) {
      await scope.close()
      throw error
    } finally {
      options.signal?.removeEventListener('abort', cancel)
    }
  }

  release(scope: WorkbenchPluginScope): void {
    if (this.scopes.get(scope.id) === scope) this.scopes.delete(scope.id)
  }

  close(): Promise<PluginCloseReport> {
    if (this.closing) return this.closing
    this.stopped = true
    const pending = [...this.scopes.values()].filter((scope) => !scope.options.parent).map((scope) => scope.close())
    this.closing = (async () => {
      await this.serial
      const reports = await Promise.all(pending)
      return {
        failed: reports.flatMap((report) => report.failed),
        timedOut: reports.flatMap((report) => report.timedOut)
      }
    })()
    return this.closing
  }
}
