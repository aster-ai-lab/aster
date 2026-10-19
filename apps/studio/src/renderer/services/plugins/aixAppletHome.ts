// Homepage seed handoff for one AIX mini app view.
//
// The composer and the applet window do not share a JavaScript realm. The
// shell keeps the seed and the deck fact. This module only posts `aix.view.seed`
// on the view document's port and reports the document's `aix.view.home` reply.

const HOME_EVENT = 'a3s:aix-applet-home-seed'

export interface AppletHomeReceipt {
  delivery: number
  holdsWork: boolean
  applied: boolean | null
  seed: unknown
  notice: string | null
  modelRequests: number
}

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

interface StagedSeed {
  seed: unknown
  delivery: number
}

interface BoundPort {
  port: MessagePort
  delivery: number
}

const staged = new Map<string, StagedSeed>()
const ports = new Map<string, BoundPort>()
const pendingMessages = new Map<string, unknown[]>()
let localDelivery = 1

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

function key(slug: string, view: string) {
  return `${slug}\0${view}`
}

function postSeed(port: MessagePort, seed: unknown, delivery: number) {
  port.postMessage({ protocolVersion: 1, type: 'aix.view.seed', seed, delivery })
}

export function resetAppletHomeForTests() {
  staged.clear()
  ports.clear()
  pendingMessages.clear()
  localDelivery = 1
}

/** Drop a staged seed, a queued deck message, and a bound port for one uninstalled plugin. */
export function releaseAppletHome(slug: string) {
  const prefix = `${slug}\0`
  for (const id of staged.keys()) if (id.startsWith(prefix)) staged.delete(id)
  for (const id of pendingMessages.keys()) if (id.startsWith(prefix)) pendingMessages.delete(id)
  for (const id of ports.keys()) if (id.startsWith(prefix)) ports.delete(id)
}

/** Post a view message now, or after the port binds. A staged seed is posted first. */
export function postAppletViewMessage(slug: string, view: string, message: unknown) {
  const id = key(slug, view)
  const bound = ports.get(id)
  if (bound) {
    bound.port.postMessage(message)
    return
  }
  const queue = pendingMessages.get(id) ?? []
  queue.push(message)
  pendingMessages.set(id, queue)
}

/** Hand a homepage seed to the view that will open in this window. No native applet window is required. */
export function stageAppletHomeSeed(slug: string, view: string, seed: unknown) {
  const delivery = localDelivery
  localDelivery += 1
  stage(slug, view, seed, delivery)
}

export async function readAppletHomeReceipt(slug: string, view: string): Promise<AppletHomeReceipt | null> {
  const invoke = tauriInvoke()
  if (!invoke) return null
  const fact = await invoke('read_aix_applet_home_deck', { slug, view })
  if (!fact || typeof fact !== 'object') return null
  const record = fact as Partial<AppletHomeReceipt>
  if (typeof record.delivery !== 'number' || typeof record.holdsWork !== 'boolean') return null
  return {
    delivery: record.delivery,
    holdsWork: record.holdsWork,
    applied: record.applied === true ? true : record.applied === false ? false : null,
    seed: record.seed ?? null,
    notice: typeof record.notice === 'string' ? record.notice : null,
    modelRequests: typeof record.modelRequests === 'number' ? record.modelRequests : 0
  }
}

export async function readAppletHomeHoldsWork(slug: string, view: string): Promise<boolean> {
  const receipt = await readAppletHomeReceipt(slug, view)
  return receipt?.holdsWork === true
}

export async function waitAppletHomeDelivery(
  slug: string,
  view: string,
  delivery: number
): Promise<AppletHomeReceipt | null> {
  const deadline = Date.now() + 8_000
  while (Date.now() < deadline) {
    const receipt = await readAppletHomeReceipt(slug, view)
    if (receipt && receipt.delivery === delivery && receipt.applied !== null) return receipt
    await new Promise<void>((resolve) => globalThis.setTimeout(resolve, 15))
  }
  return null
}

function stage(slug: string, view: string, seed: unknown, delivery: number) {
  const id = key(slug, view)
  // A task confirm replaces the open document. Keep the seed until that next bind.
  staged.set(id, { seed, delivery })
  const bound = ports.get(id)
  if (!bound) return
  bound.delivery = delivery
  try {
    postSeed(bound.port, seed, delivery)
  } catch {
    // A closed port still leaves the seed for the document that replaces it.
  }
}

/** Take the shell's pending seed and post it once this view's port is bound. */
export async function takeAndStageAppletHomeSeed(slug: string, view: string): Promise<void> {
  const invoke = tauriInvoke()
  if (!invoke) return
  const taken = await invoke('take_aix_applet_home_seed', { slug, view })
  if (!taken || typeof taken !== 'object') return
  const delivery = (taken as { delivery?: unknown }).delivery
  const raw = (taken as { seed?: unknown }).seed
  if (typeof delivery !== 'number' || delivery <= 0 || typeof raw !== 'string') return
  let seed: unknown
  try {
    seed = JSON.parse(raw)
  } catch {
    return
  }
  if (!seed || typeof seed !== 'object') return
  stage(slug, view, seed, delivery)
}

function publish(slug: string, view: string, data: Record<string, unknown>) {
  const delivery = data.delivery
  if (typeof delivery !== 'number') return
  const invoke = tauriInvoke()
  if (!invoke) return
  const applied = data.applied === true ? true : data.applied === false ? false : null
  void invoke('report_aix_applet_home_deck', {
    slug,
    view,
    delivery,
    holdsWork: data.holdsWork === true,
    applied,
    seed: data.seed === undefined ? null : data.seed,
    notice: typeof data.notice === 'string' ? data.notice : null,
    modelRequests: typeof data.modelRequests === 'number' ? data.modelRequests : 0,
    hasApplied: applied !== null,
    hasSeed: data.seed !== undefined
  })
}

/** Bind the view document's port. A staged seed is posted on it. */
export function receiveAppletHomeDocument(slug: string, view: string, port: MessagePort) {
  const id = key(slug, view)
  const bound: BoundPort = { port, delivery: staged.get(id)?.delivery ?? 0 }
  ports.set(id, bound)
  port.addEventListener('message', (event: MessageEvent) => {
    const data = event.data as Record<string, unknown> | null
    if (!data || data.protocolVersion !== 1) return
    if (data.type === 'aix.view.home') {
      publish(slug, view, data)
      return
    }
    if (data.type !== 'aix.view.deck-snapshot' || typeof data.text !== 'string') return
    const text = data.text
    // shortcut: a plane miss leaves the view store as the deck. Upgrade when the tool call waits for this snapshot.
    void import('../../data/studioAgentPlane').then((plane) => plane.publishPresentonDeck(text)).catch(() => undefined)
  })
  const pending = staged.get(id)
  if (pending) {
    staged.delete(id)
    bound.delivery = pending.delivery
    postSeed(port, pending.seed, pending.delivery)
  }
  const queued = pendingMessages.get(id)
  if (!queued) return
  pendingMessages.delete(id)
  for (const message of queued) port.postMessage(message)
}

export function listenAppletHomeSeed(onSeed: () => void): () => void {
  const listener = () => onSeed()
  globalThis.window.addEventListener(HOME_EVENT, listener)
  return () => globalThis.window.removeEventListener(HOME_EVENT, listener)
}

export { HOME_EVENT }
