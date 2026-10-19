import type { ComponentType } from 'react'

import type { SidebarFavorite } from '@shared/data/preference/preferenceTypes'
import type {
  StudioCompositionMode,
  StudioContributionCondition,
  StudioExtensionPointId
} from '@shared/types/studioContribution'

export type PluginScopeKind = 'window' | 'workspace' | 'view' | 'session' | 'run' | 'turn' | 'subtask'

export interface PluginServices {
  workspace: { readonly id: string; readonly root: string }
  document: { readonly uri: string; readonly version: number }
  conversation: { readonly sessionId: string }
  /** Readonly admitted Run observation, without resource authority. */
  run: { readonly coreSessionId: string; readonly runId: string; readonly promptId: string }
  /** Model-call ordinal; not the identity of a Core Tool Turn. */
  turn: { readonly modelTurn: number }
  harness: { readonly components: readonly string[] }
}

export type PluginServiceName = keyof PluginServices
export type PluginServiceSnapshot = Readonly<Partial<PluginServices>>
export type PluginCleanup = () => void | Promise<void>

export interface PluginActivationContext {
  readonly scopeId: string
  readonly services: PluginServiceSnapshot
  readonly signal: globalThis.AbortSignal
  isCurrent(): boolean
  defer(cleanup: PluginCleanup): void
  listen(target: globalThis.EventTarget, event: string, listener: (event: globalThis.Event) => void): void
  spawn(task: (signal: globalThis.AbortSignal) => Promise<void>): void
}

export interface PluginRenderProps {
  readonly context: PluginActivationContext
}

/** Immutable renderer draft observation; it grants no native resource access. */
export interface PluginComposerDraft {
  readonly id: string
  readonly revision: number
  readonly text: string
}

interface ContributionBase {
  readonly id: string
  readonly labelKey: string
  readonly title?: string
  readonly order?: number
  readonly point?: StudioExtensionPointId
  readonly contractVersion?: number
  readonly mode?: StudioCompositionMode
  readonly scopes?: readonly PluginScopeKind[]
  readonly services?: readonly PluginServiceName[]
  readonly when?: StudioContributionCondition
}

export type PluginContribution = ContributionBase &
  (
    | {
        readonly slot: 'activity' | 'applet'
        readonly pageId: string
        readonly description?: string
        readonly sidebarId?: string
        readonly applicationId?: SidebarFavorite
        readonly icon?: ComponentType<{ size: number; strokeWidth: number }>
      }
    | {
        readonly slot: 'page' | 'sidebar' | 'panel' | 'workspace'
        readonly component: ComponentType<PluginRenderProps>
      }
    | {
        readonly slot: 'composer'
        readonly invoke: (
          context: PluginActivationContext,
          text: string,
          draft: PluginComposerDraft,
          signal?: globalThis.AbortSignal
        ) => string | Promise<string>
      }
    | { readonly slot: 'harness'; readonly components: readonly string[] }
  )

/** Trusted UI factories only; external package authority stays in the native host. */
export interface WorkbenchPlugin {
  readonly id: string
  readonly generation: number
  readonly protocolVersion?: number
  readonly requires?: readonly string[]
  readonly services?: readonly PluginServiceName[]
  readonly scopes: readonly PluginScopeKind[]
  readonly contributions: readonly PluginContribution[]
  readonly activate?: (context: PluginActivationContext) => void | Promise<void>
}

export interface PluginCloseReport {
  readonly failed: readonly string[]
  readonly timedOut: readonly string[]
}

export interface PluginFiber {
  readonly id: string
  readonly generation: number
  readonly state: 'active' | 'blocked'
  readonly missing: readonly string[]
}

export interface PublishedPluginContribution {
  readonly key: string
  readonly pluginId: string
  readonly pluginGeneration: number
  readonly point: StudioExtensionPointId
  readonly contractVersion: number
  readonly contribution: PluginContribution
  readonly context: PluginActivationContext
}

export interface PluginContributionDiagnostic {
  readonly key: string
  readonly point: StudioExtensionPointId
  readonly reason: 'service-unavailable'
  readonly missing: readonly PluginServiceName[]
}

export interface PluginScopeSnapshot {
  readonly generation: number
  readonly diagnostics?: readonly PluginContributionDiagnostic[]
  readonly fibers: readonly PluginFiber[]
  readonly contributions: readonly PublishedPluginContribution[]
}
