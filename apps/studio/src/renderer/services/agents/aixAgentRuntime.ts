/** One installed AIX agent run. Edge, cloud, and remote share this lifecycle. */

export type AixRuntimeMode = 'edge' | 'cloud' | 'remote'
export type AixHarness = 'builtin' | 'package'

export type AixAgentDeclaration = {
  runtime: AixRuntimeMode
  harness?: AixHarness
  entry?: string
  card?: string
  resources?: { cpuMillis: number; memoryBytes: number }
}

export type AixRunEvent =
  | { sequence: number; type: 'text_delta'; text: string }
  | { sequence: number; type: 'output'; text: string }
  | { sequence: number; type: 'completed' }
  | { sequence: number; type: 'failed'; message: string }

export type AixCloudBinding = {
  packageId: string
  digest: string
  agentAssetId: string
  agentAssetReleaseId: string
}

export type AixRunRequest = {
  declaration: AixAgentDeclaration
  text: string
  parameters?: Record<string, string | number | boolean>
  /** Package identity bound to a cloud asset release. The install slug is not an asset id. */
  cloudBinding?: AixCloudBinding
  conversationId?: string
  installSlug?: string
  /** Inference route model alias for the cloud agent run; the Cloud binds it to the workload before dispatch. */
  modelAlias?: string
}

export interface AixRuntimeTransport {
  start(request: AixRunRequest): Promise<{ runId: string }>
  page(runId: string, after: number): Promise<AixRunEvent[]>
  stop(runId: string): Promise<void>
  cancel?(runId: string): Promise<void>
}

export type AixRunOutcome = {
  delivery: 'stream' | 'once'
  text: string
  destroyed: true
}

const CARD_URL = /^https:\/\/[^\s/?#]+(?:\/[^\s?#]*)?$/

export function aixDeclarationFromUnknown(value: unknown): AixAgentDeclaration | null {
  if (!value || typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  if (record.runtime !== 'edge' && record.runtime !== 'cloud' && record.runtime !== 'remote') return null
  const declaration: AixAgentDeclaration = { runtime: record.runtime }
  if (record.harness === 'builtin' || record.harness === 'package') declaration.harness = record.harness
  if (typeof record.entry === 'string' && record.entry.length > 0) declaration.entry = record.entry
  if (typeof record.card === 'string' && record.card.length > 0) declaration.card = record.card
  const resources = record.resources
  if (resources && typeof resources === 'object') {
    const cpuMillis = (resources as { cpuMillis?: unknown }).cpuMillis
    const memoryBytes = (resources as { memoryBytes?: unknown }).memoryBytes
    if (typeof cpuMillis === 'number' && typeof memoryBytes === 'number') {
      declaration.resources = { cpuMillis, memoryBytes }
    }
  }
  return declarationError(declaration) ? null : declaration
}

export function declarationError(declaration: AixAgentDeclaration): string | null {
  if (declaration.runtime === 'remote') {
    if (declaration.harness || declaration.entry || declaration.resources) {
      return 'A remote agent cannot declare a harness, entry, or resources.'
    }
    if (!declaration.card || !CARD_URL.test(declaration.card)) {
      return 'A remote agent requires a credential-free HTTPS agent card URL.'
    }
    return null
  }
  if (declaration.card) return 'An edge or cloud agent cannot declare an agent card.'
  if (declaration.harness !== 'builtin' && declaration.harness !== 'package') {
    return 'An edge or cloud agent requires a harness.'
  }
  if (declaration.harness === 'builtin' && declaration.entry) {
    return 'A builtin harness cannot declare an entry.'
  }
  if (declaration.harness === 'package' && !declaration.entry) {
    return 'A package harness requires an entry.'
  }
  const resources = declaration.resources
  if (!resources || !Number.isInteger(resources.cpuMillis) || resources.cpuMillis <= 0) {
    return 'An edge or cloud agent requires cpuMillis.'
  }
  if (!Number.isInteger(resources.memoryBytes) || resources.memoryBytes <= 0) {
    return 'An edge or cloud agent requires memoryBytes.'
  }
  return null
}

/**
 * The renderer does not execute an AIX agent. Edge, cloud, and remote all
 * stay on the host, so this returns no text and does not call the transport.
 */
export async function rendererSuppliesAgentOutput(
  _request: AixRunRequest,
  _transport: AixRuntimeTransport
): Promise<string | undefined> {
  return undefined
}

/**
 * Start one run, read events until a terminal page, then destroy it.
 * A refusal happens before `start`. `stop` still runs after a page failure.
 */
export async function runAixAgent(
  request: AixRunRequest,
  transport: AixRuntimeTransport,
  signal?: AbortSignal
): Promise<AixRunOutcome> {
  const problem = declarationError(request.declaration)
  if (problem) throw new Error(problem)
  if (signal?.aborted) throw new Error('The agent run was cancelled.')
  const started = await transport.start(request)
  let after = 0
  let text = ''
  let deltas = 0
  try {
    for (;;) {
      if (signal?.aborted) throw new Error('The agent run was cancelled.')
      const events = await transport.page(started.runId, after)
      if (events.length === 0) throw new Error('The agent run ended without a terminal event.')
      for (const event of events) {
        if (event.sequence <= after) throw new Error('Agent events must advance by sequence.')
        after = event.sequence
        if (event.type === 'text_delta') {
          deltas += 1
          text += event.text
        } else if (event.type === 'output') {
          text = event.text
        } else if (event.type === 'failed') {
          throw new Error(event.message)
        } else if (event.type === 'completed') {
          return { delivery: deltas > 0 ? 'stream' : 'once', text, destroyed: true }
        }
      }
    }
  } catch (error) {
    if (signal?.aborted) await transport.cancel?.(started.runId)
    throw error
  } finally {
    await transport.stop(started.runId)
  }
}

export function a2aTaskSend(request: AixRunRequest, runId: string) {
  return {
    jsonrpc: '2.0',
    id: runId,
    method: 'tasks/send',
    params: {
      id: runId,
      message: {
        role: 'user',
        parts: [
          { type: 'text', text: request.text },
          ...(request.parameters ? [{ type: 'data', data: request.parameters }] : [])
        ]
      }
    }
  }
}

const CLOUD_PACKAGE_ID = /^use\/[a-z0-9][a-z0-9-]*\/[a-z0-9][a-z0-9-]*$/
const CLOUD_DIGEST = /^sha256:[0-9a-f]{64}$/
const CLOUD_ASSET_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

export function cloudBindingFromUnknown(value: unknown): AixCloudBinding | null {
  if (!value || typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  const binding = {
    packageId: record.packageId,
    digest: record.digest,
    agentAssetId: record.agentAssetId,
    agentAssetReleaseId: record.agentAssetReleaseId
  }
  if (
    typeof binding.packageId !== 'string' ||
    typeof binding.digest !== 'string' ||
    typeof binding.agentAssetId !== 'string' ||
    typeof binding.agentAssetReleaseId !== 'string' ||
    !CLOUD_PACKAGE_ID.test(binding.packageId) ||
    !CLOUD_DIGEST.test(binding.digest) ||
    !CLOUD_ASSET_ID.test(binding.agentAssetId) ||
    !CLOUD_ASSET_ID.test(binding.agentAssetReleaseId)
  ) {
    return null
  }
  return binding
}

/** Create body for the existing cloud Agent Runtime. An install slug is refused as an asset id. */
export function cloudCreateBody(request: AixRunRequest) {
  const binding = request.cloudBinding
  if (
    !binding ||
    !CLOUD_PACKAGE_ID.test(binding.packageId) ||
    !CLOUD_DIGEST.test(binding.digest) ||
    !CLOUD_ASSET_ID.test(binding.agentAssetId) ||
    !CLOUD_ASSET_ID.test(binding.agentAssetReleaseId)
  ) {
    throw new Error('The cloud agent is not bound to an asset release.')
  }
  if (binding.agentAssetId === request.installSlug || binding.agentAssetId === binding.packageId) {
    throw new Error('The install name is not an agent asset id.')
  }
  return {
    agentAssetId: binding.agentAssetId,
    agentAssetReleaseId: binding.agentAssetReleaseId,
    input: {
      packageId: binding.packageId,
      digest: binding.digest,
      prompt: request.text,
      ...(request.parameters ? { parameters: request.parameters } : {}),
      ...(request.modelAlias ? { modelAlias: request.modelAlias } : {})
    }
  }
}

export function a2aTaskCancel(runId: string) {
  return { jsonrpc: '2.0', id: runId, method: 'tasks/cancel', params: { id: runId } }
}

export function agentProtocolStart(request: AixRunRequest, runId: string) {
  return {
    schema: 'a3s.code.agent-run-start.v1',
    request_id: runId,
    identity: {
      schema: 'a3s.code.agent-run-identity.v1',
      protocol: 'a3s.code.agent.v1',
      agent_release_identity: `sha256:${'ab'.padStart(64, '0')}`,
      session_id: runId,
      run_id: runId
    },
    prompt: request.text
  }
}

type FetchLike = (
  url: string,
  init: { method: string; body?: string }
) => Promise<{ ok: boolean; json: () => Promise<unknown> }>

function cloudEvents(body: unknown): AixRunEvent[] {
  if (body && typeof body === 'object' && Array.isArray((body as { events?: unknown }).events)) {
    return (body as { events: AixRunEvent[] }).events
  }
  const text = body && typeof body === 'object' ? (body as { text?: unknown }).text : undefined
  if (typeof text !== 'string') throw new Error('The cloud agent runtime returned no events.')
  return [
    { sequence: 1, type: 'output', text },
    { sequence: 2, type: 'completed' }
  ]
}

/** Host transport. Edge speaks Agent Protocol. Cloud uses a bound asset release. Remote uses the card. */
export function httpAixTransport(
  endpoints: { edge?: string; cloud?: string },
  fetchImpl: FetchLike
): AixRuntimeTransport {
  const runs = new Map<string, AixRunEvent[]>()
  const remoteCards = new Map<string, string>()
  const cloudRuns = new Map<string, string>()
  const cloudBase = endpoints.cloud?.replace(/\/$/, '')
  return {
    async start(request) {
      const mode = request.declaration.runtime
      const runId = crypto.randomUUID()
      if (mode === 'remote') {
        const card = request.declaration.card
        if (!card) throw new Error('A remote agent requires a credential-free HTTPS agent card URL.')
        const response = await fetchImpl(card, { method: 'POST', body: JSON.stringify(a2aTaskSend(request, runId)) })
        if (!response.ok) throw new Error('The remote agent refused the task.')
        const body = (await response.json()) as { result?: { text?: string } }
        const text = body.result?.text
        if (typeof text !== 'string') throw new Error('The remote agent returned no text.')
        remoteCards.set(runId, card)
        runs.set(runId, [
          { sequence: 1, type: 'output', text },
          { sequence: 2, type: 'completed' }
        ])
        return { runId }
      }
      if (mode === 'cloud') {
        const create = cloudCreateBody(request)
        if (!cloudBase || !request.conversationId) {
          throw new Error('The cloud agent runtime endpoint is not configured.')
        }
        const response = await fetchImpl(`${cloudBase}/agent-conversations/${request.conversationId}/agent-runtimes`, {
          method: 'POST',
          body: JSON.stringify(create)
        })
        if (!response.ok) throw new Error('The cloud agent runtime refused the run.')
        const body = (await response.json()) as { projection?: { agentExecutionId?: string } }
        const executionId = body.projection?.agentExecutionId
        if (!executionId) throw new Error('The cloud agent runtime returned no run.')
        cloudRuns.set(executionId, cloudBase)
        return { runId: executionId }
      }
      const endpoint = endpoints.edge
      if (!endpoint) throw new Error('The edge agent runtime endpoint is not configured.')
      const response = await fetchImpl(`${endpoint.replace(/\/$/, '')}/v1/agent/commands`, {
        method: 'POST',
        body: JSON.stringify(agentProtocolStart(request, runId))
      })
      if (!response.ok) throw new Error('The agent runtime refused the command.')
      const body = (await response.json()) as { events?: AixRunEvent[] }
      if (!Array.isArray(body.events) || body.events.length === 0) {
        throw new Error('The agent runtime returned no events.')
      }
      runs.set(runId, body.events)
      return { runId }
    },
    async page(runId, after) {
      const base = cloudRuns.get(runId)
      if (base) {
        const response = await fetchImpl(`${base}/agent-runtimes/${runId}/logs`, { method: 'GET' })
        if (!response.ok) throw new Error('The cloud agent runtime returned no events.')
        return cloudEvents(await response.json()).filter((event) => event.sequence > after)
      }
      const events = runs.get(runId) ?? []
      return events.filter((event) => event.sequence > after)
    },
    async cancel(runId) {
      const card = remoteCards.get(runId)
      if (!card) return
      await fetchImpl(card, { method: 'POST', body: JSON.stringify(a2aTaskCancel(runId)) })
    },
    async stop(runId) {
      const base = cloudRuns.get(runId)
      cloudRuns.delete(runId)
      remoteCards.delete(runId)
      runs.delete(runId)
      if (base) {
        await fetchImpl(`${base}/agent-runtimes/${runId}/stop`, { method: 'POST', body: '{}' })
      }
    }
  }
}

export type AixCloudRunSummary = {
  agentExecutionId: string
  conversationId: string
  status: string
  fullyBound: boolean
  harnessInvocationProfileDigest?: string
  egressPolicyDigest?: string
  workspaceMaterializationDigest?: string
}

/** List the cloud agent runs of one conversation (run history). */
export async function cloudListRuns(
  endpoints: { cloud?: string },
  fetchImpl: (
    url: string,
    init: { method: string; headers?: Record<string, string> }
  ) => Promise<{ ok: boolean; json: () => Promise<unknown> }>,
  conversationId: string,
  bearer?: string
): Promise<AixCloudRunSummary[]> {
  const cloudBase = endpoints.cloud?.replace(/\/$/, '')
  if (!cloudBase || !conversationId) return []
  const headers: Record<string, string> = { 'content-type': 'application/json' }
  if (bearer) headers.authorization = `Bearer ${bearer}`
  const response = await fetchImpl(`${cloudBase}/agent-conversations/${conversationId}/agent-runtimes?limit=20`, {
    method: 'GET',
    headers
  })
  if (!response.ok) return []
  const body = (await response.json()) as { data?: AixCloudRunSummary[] }
  return Array.isArray(body.data) ? body.data : []
}
