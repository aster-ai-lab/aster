import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import { readStudioRuntime } from '@shared/studioRuntime'
import {
  AIX_AGENT_REFUSALS,
  AixAgentRefusedError,
  aixAgentTurnSchema,
  type AixAgentImage
} from '@shared/types/aixAgent'
import type { AixViewEnvironment } from '@shared/types/aixViewBridge'
import {
  aixWorkbenchCatalogueSchema,
  aixViewSessionSchema,
  aixViewCancellationSchema,
  AIX_VIEW_SESSION_PROTOCOL_VERSION,
  AIX_VIEW_ADMISSION_LIFETIME_MS,
  AIX_VIEW_CANCELLATION_TIMEOUT_MS,
  type AixViewAdmission,
  type AixWorkbenchCatalogue,
  type AixWorkbenchPackage,
  type AixWorkbenchView
} from '@shared/types/aixWorkbench'

import { AixAgentProvider } from './aixAgentProvider'
import { receiveAppletHomeDocument } from './aixAppletHome'
import { AixCommandProvider } from './aixCommandProvider'
import { AixComposerProvider } from './aixComposerProvider'
import { bindAixEditorRecovery } from './aixEditorRecovery'
import { AixResourceProvider } from './aixResourceProvider'
import { invokeAixResource } from './aixResourceTransport'
import { AixTreeProvider } from './aixTreeProvider'
import { AixViewBridge } from './aixViewBridge'
import {
  aixWorkbenchPackageIdentity,
  encodeAixWorkbenchIdentityRouteKey,
  type AixWorkbenchActivationIdentity,
  type AixWorkbenchInstanceIdentity
} from './aixWorkbenchIdentity'

async function request(path: string, method: string, body?: string, signal?: globalThis.AbortSignal): Promise<unknown> {
  const runtime = readStudioRuntime()
  if (!runtime) throw new Error('Native AIX host is unavailable')
  const response = await globalThis.fetch(`${runtime.apiBaseUrl}${path}`, {
    method,
    signal,
    headers: { Authorization: `Bearer ${runtime.token}`, 'Content-Type': 'application/json' },
    body
  })
  if (!response.ok) throw new Error('Native AIX view was refused')
  return response.json()
}

export async function readAixWorkbench(signal: globalThis.AbortSignal): Promise<AixWorkbenchCatalogue> {
  return aixWorkbenchCatalogueSchema.parse(await request('/api/aix/views', 'GET', undefined, signal))
}

export async function openAixView(
  item: AixWorkbenchPackage,
  view: AixWorkbenchView,
  signal: globalThis.AbortSignal,
  admission: AixViewAdmission
): Promise<{ token: string; url: string }> {
  const value = aixViewSessionSchema.parse(
    await request(
      '/api/aix/views/sessions',
      'POST',
      JSON.stringify({
        protocolVersion: AIX_VIEW_SESSION_PROTOCOL_VERSION,
        ...admission,
        slug: item.slug,
        surface: view.id,
        revision: item.revision
      }),
      signal
    )
  )
  if (
    value.requestId !== admission.requestId ||
    value.expiresAt !== admission.expiresAt ||
    !value.path.startsWith(`/aix-view/${value.token}/`) ||
    /[\\?#]/.test(value.path) ||
    [...value.path].some((character) => character.charCodeAt(0) < 32) ||
    value.path.split('/').some((part) => part === '..' || part === '.')
  ) {
    throw new Error('Invalid native AIX asset session')
  }
  const runtime = readStudioRuntime()
  if (!runtime) throw new Error('Native AIX host is unavailable')
  return { token: value.token, url: `${runtime.apiBaseUrl}${value.path}` }
}

/** One turn of an edge agent the live view's own package declares. The native host binds and runs it. */
export async function invokeAixViewAgent(
  token: string,
  agent: string,
  prompt: string,
  parameters: Record<string, unknown> | undefined,
  images: readonly AixAgentImage[] | undefined,
  signal: globalThis.AbortSignal
): Promise<string> {
  const runtime = readStudioRuntime()
  if (!runtime || !/^[a-f0-9]{64}$/.test(token)) throw new AixAgentRefusedError('closed')
  const response = await globalThis.fetch(
    `${runtime.apiBaseUrl}/api/aix/views/sessions/${token}/agents/${encodeURIComponent(agent)}/turns`,
    {
      method: 'POST',
      signal,
      headers: { Authorization: `Bearer ${runtime.token}`, 'Content-Type': 'application/json' },
      body: JSON.stringify({ prompt, ...(parameters ? { parameters } : {}), ...(images ? { images } : {}) })
    }
  )
  const body: unknown = await response.json().catch(() => null)
  if (!response.ok) {
    const error = (body as { error?: unknown } | null)?.error
    const reason = AIX_AGENT_REFUSALS.find((known) => known === error) ?? 'agent_failed'
    throw new AixAgentRefusedError(reason)
  }
  return aixAgentTurnSchema.parse(body).text
}

export async function closeAixView(token: string): Promise<void> {
  if (!/^[a-f0-9]{64}$/.test(token)) return
  await request(`/api/aix/views/sessions/${token}`, 'DELETE')
}

async function cancelAixView(admission: AixViewAdmission): Promise<void> {
  const abort = new globalThis.AbortController()
  let timer: ReturnType<typeof globalThis.setTimeout> | undefined
  try {
    const value = aixViewCancellationSchema.parse(
      await Promise.race([
        request(
          `/api/aix/views/requests/${admission.requestId}`,
          'DELETE',
          JSON.stringify({
            protocolVersion: AIX_VIEW_SESSION_PROTOCOL_VERSION,
            activationId: admission.activationId,
            instanceId: admission.instanceId,
            expiresAt: admission.expiresAt
          }),
          abort.signal
        ),
        new Promise<never>((_resolve, reject) => {
          timer = globalThis.setTimeout(() => {
            reject(new Error('AIX cancellation outcome unknown: acknowledgment timed out'))
            abort.abort()
          }, AIX_VIEW_CANCELLATION_TIMEOUT_MS)
        })
      ])
    )
    if (
      value.requestId !== admission.requestId ||
      value.activationId !== admission.activationId ||
      value.instanceId !== admission.instanceId ||
      value.expiresAt !== admission.expiresAt
    )
      throw new Error('Invalid native AIX cancellation receipt')
  } finally {
    globalThis.clearTimeout(timer)
  }
}

function randomIdentity(): string {
  return Array.from(globalThis.crypto.getRandomValues(new Uint8Array(32)), (byte) =>
    byte.toString(16).padStart(2, '0')
  ).join('')
}

class AixViewSession {
  private readonly admission: AixViewAdmission
  private readonly abort = new globalThis.AbortController()
  private stopped = false
  private closing?: Promise<void>
  private admitted = false
  private connected = false
  private loaded = false
  private readonly bridge: AixViewBridge
  readonly documentId: string
  readonly tree?: AixTreeProvider
  readonly composer?: AixComposerProvider
  readonly commands?: AixCommandProvider
  readonly agents: AixAgentProvider
  readonly resources: AixResourceProvider
  private token?: string
  private releaseRecovery?: () => void
  readonly identity: AixWorkbenchInstanceIdentity
  readonly routeKey: string
  readonly ready: Promise<{ token: string; url: string }>

  constructor(
    item: AixWorkbenchPackage,
    view: AixWorkbenchView,
    context: PluginActivationContext,
    activation: AixWorkbenchActivationIdentity,
    documentId: string,
    admission: AixViewAdmission,
    state: { read(): string | null; write(value: string | null): boolean },
    private readonly released: () => void
  ) {
    this.documentId = documentId
    this.admission = admission
    this.identity = {
      activation,
      contribution: { package: activation.package, kind: 'view', localId: view.id },
      instanceId: documentId
    }
    this.routeKey = encodeAixWorkbenchIdentityRouteKey({ type: 'instance', instance: this.identity })
    if (view.commands?.length)
      this.commands = new AixCommandProvider(
        context,
        view.id,
        view.commands.map((command) => command.id),
        () => this.withdraw()
      )
    const closeCommands = () => this.commands?.close()
    this.bridge = new AixViewBridge(context, view.id, state, closeCommands)
    if (view.presentation === 'tree') this.tree = new AixTreeProvider(context, view.id, 5000, closeCommands)
    if (view.point === 'studio.composer.actions') this.composer = new AixComposerProvider(context, view.id)
    this.agents = new AixAgentProvider(
      context,
      (agent, prompt, parameters, images, signal) =>
        invokeAixViewAgent(this.token ?? '', agent, prompt, parameters, images, signal),
      () => this.withdraw()
    )
    this.resources = new AixResourceProvider(
      context,
      (operation, signal) => invokeAixResource(this.token ?? '', operation, signal),
      () => this.withdraw()
    )
    this.ready = openAixView(item, view, this.abort.signal, this.admission).then((session) => {
      if (this.stopped || !context.isCurrent()) throw new Error('AIX view owner closed')
      this.admitted = true
      this.token = session.token
      return { ...session, url: `${session.url}#aix-document=${this.documentId}` }
    })
  }

  connect(documentId: string, port: globalThis.MessagePort): boolean {
    if (!this.admitted || this.stopped || this.connected || documentId !== this.documentId) {
      port.close()
      return false
    }
    this.connected = true
    if (this.composer) this.composer.connect(port)
    else if (this.tree) this.tree.connect(port)
    else this.bridge.connect(port)
    if (this.commands) {
      const primary = port.onmessage
      const primaryError = port.onmessageerror
      port.onmessage = (event) => {
        if (typeof event.data?.type === 'string' && event.data.type.startsWith('aix.command.'))
          this.commands?.receive(event.data)
        else primary?.call(port, event)
      }
      port.onmessageerror = (event) => {
        primaryError?.call(port, event)
        this.withdraw()
      }
      this.commands.connect(port)
    }
    const beforeAgents = port.onmessage
    port.onmessage = (event) => {
      if (typeof event.data?.type === 'string' && event.data.type.startsWith('aix.agent.'))
        this.agents.receive(event.data)
      else beforeAgents?.call(port, event)
    }
    this.agents.connect(port)
    const beforeResources = port.onmessage
    port.onmessage = (event) => {
      if (typeof event.data?.type === 'string' && event.data.type.startsWith('aix.resource.'))
        this.resources.receive(event.data)
      else beforeResources?.call(port, event)
    }
    this.resources.connect(port)
    receiveAppletHomeDocument(this.identity.contribution.package.slug, this.identity.contribution.localId, port)
    this.releaseRecovery = bindAixEditorRecovery(
      this.identity.contribution.package.slug,
      this.identity.contribution.localId,
      port
    )
    return true
  }

  documentLoaded(): boolean {
    if (!this.admitted || this.stopped || this.loaded) return false
    this.loaded = true
    this.tree?.waitForDocument()
    this.commands?.waitForDocument()
    return true
  }

  setEnvironment(environment: AixViewEnvironment): void {
    if (!this.stopped && !this.tree && !this.composer) this.bridge.setEnvironment(environment)
  }

  withdraw(): void {
    this.stopped = true
    this.releaseRecovery?.()
    this.bridge.close()
    this.tree?.close()
    this.composer?.close()
    this.commands?.close()
    this.agents.close()
    this.resources.close()
    this.abort.abort()
  }

  close(): Promise<void> {
    if (!this.closing) {
      this.withdraw()
      this.closing = cancelAixView(this.admission).then(
        () => this.released(),
        (error) => {
          this.closing = undefined
          throw error
        }
      )
    }
    return this.closing
  }
}

/** One scoped cleanup owns all mounted asset sessions, including pending opens. */
export class AixViewSessions {
  private readonly sessions = new Map<string, AixViewSession>()
  private readonly operations = new Set<string>()
  private readonly states = new Map<string, string>()
  private stopped = false
  private closing?: Promise<void>
  private readonly activation: AixWorkbenchActivationIdentity
  private readonly withdraw = () => {
    this.stopped = true
    this.states.clear()
    for (const session of this.sessions.values()) session.withdraw()
  }

  constructor(
    private readonly context: PluginActivationContext,
    item: AixWorkbenchPackage,
    activationId = randomIdentity()
  ) {
    this.activation = {
      package: {
        slug: item.slug,
        packageId: item.packageId,
        generation: item.generation,
        revision: item.revision
      },
      scopeId: context.scopeId,
      activationId
    }
    context.defer(() => this.close())
    context.signal.addEventListener('abort', this.withdraw, { once: true })
  }

  open(item: AixWorkbenchPackage, view: AixWorkbenchView): AixViewSession {
    if (this.stopped || !this.context.isCurrent()) throw new Error('AIX view owner closed')
    if (aixWorkbenchPackageIdentity(item) !== aixWorkbenchPackageIdentity(this.activation.package))
      throw new Error('AIX view package pin does not match its activation')
    if (this.sessions.size >= 64) throw new Error('AIX view session limit exceeded')
    const stateKey = JSON.stringify([aixWorkbenchPackageIdentity(item), view.id])
    const documentId = randomIdentity()
    const admission: AixViewAdmission = {
      requestId: randomIdentity(),
      activationId: this.activation.activationId,
      instanceId: documentId,
      expiresAt: Date.now() + AIX_VIEW_ADMISSION_LIFETIME_MS
    }
    const identity: AixWorkbenchInstanceIdentity = {
      activation: this.activation,
      contribution: { package: this.activation.package, kind: 'view', localId: view.id },
      instanceId: documentId
    }
    const instanceKey = encodeAixWorkbenchIdentityRouteKey({ type: 'instance', instance: identity })
    const operationKeys = (['admit', 'cancel'] as const).map((operation) =>
      encodeAixWorkbenchIdentityRouteKey({
        type: 'operation',
        operation: { instance: identity, operation, operationId: admission.requestId }
      })
    )
    if (this.sessions.has(instanceKey) || operationKeys.some((key) => this.operations.has(key)))
      throw new Error('Duplicate AIX view identity')
    for (const key of operationKeys) this.operations.add(key)
    let session!: AixViewSession
    try {
      session = new AixViewSession(
        item,
        view,
        this.context,
        this.activation,
        documentId,
        admission,
        {
          read: () => this.states.get(stateKey) ?? null,
          write: (value) => {
            if (this.stopped || !this.context.isCurrent()) return false
            if (value === null) this.states.delete(stateKey)
            else {
              if (!this.states.has(stateKey) && this.states.size >= 64) return false
              this.states.set(stateKey, value)
            }
            return true
          }
        },
        () => {
          this.sessions.delete(instanceKey)
          for (const key of operationKeys) this.operations.delete(key)
        }
      )
    } catch (error) {
      for (const key of operationKeys) this.operations.delete(key)
      throw error
    }
    if (session.routeKey !== instanceKey) throw new Error('AIX view identity changed during admission')
    this.sessions.set(instanceKey, session)
    return session
  }

  close(): Promise<void> {
    if (!this.closing) {
      this.context.signal.removeEventListener('abort', this.withdraw)
      this.withdraw()
      this.closing = Promise.allSettled([...this.sessions.values()].map((session) => session.close())).then(
        (results) => {
          const failed = results.filter((result): result is PromiseRejectedResult => result.status === 'rejected')
          if (failed.length) {
            this.closing = undefined
            throw new AggregateError(
              failed.map((result) => result.reason),
              'AIX asset cleanup failed'
            )
          }
        }
      )
    }
    return this.closing
  }
}
