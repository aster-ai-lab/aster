// The Studio view bridge: one MessagePort to the host, which runs this
// package's edge agent and keeps the view's state between openings.
const AGENT = "presenton";
const PROTOCOL = 1;

const REFUSALS: Record<string, string> = {
  standalone: "未在 Studio 中打开：生成需要本包的 agent。",
  busy: "代理正忙，请稍后再试。",
  capacity: "代理容量已满，请稍后再试。",
  not_found: "没有找到本包的 agent。",
  invalid_request: "请求被拒绝。",
  agent_failed: "代理运行失败。",
  closed: "视图已关闭。",
};

export class AgentError extends Error {
  readonly reason: string;

  constructor(message: string, reason: string) {
    super(message);
    this.reason = reason;
  }
}

let port: MessagePort | null = null;
let messageId = 0;
let restored = false;
let queuedSeed: unknown;
let hasQueuedSeed = false;
let queuedDeck: unknown[] = [];
let homeDelivery = 0;
/** Counts agent turns. A homepage seed must leave this at zero. */
export let agentModelRequests = 0;

export function resetAgentModelRequestsForTests() {
  agentModelRequests = 0;
}
const pending = new Map<number, { resolve: (text: string) => void; reject: (error: Error) => void }>();

function documentId(): string | null {
  const match = /(?:^|[#&])aix-document=([a-f0-9]{64})(?:&|$)/.exec(location.hash);
  return match ? match[1] : null;
}

type SeedResult = { applied?: boolean; notice?: string; seed?: unknown };

function postHome(body: Record<string, unknown>) {
  port?.postMessage({
    protocolVersion: PROTOCOL,
    type: "aix.view.home",
    delivery: homeDelivery,
    modelRequests: agentModelRequests,
    ...body,
  });
}

/** The view's own deck fact. The host reads this message, not this document's global. */
export function reportHomeDeck(holdsWork: boolean) {
  postHome({ holdsWork });
}

/** Compact deck text for the aster read tool. The renderer posts it to the plane. */
export function reportDeckSnapshot() {
  const read = (globalThis as typeof globalThis & { __asterPresentonReadDeck?: () => unknown }).__asterPresentonReadDeck;
  if (typeof read !== "function" || !port) return;
  const text = read();
  if (typeof text !== "string") return;
  port.postMessage({ protocolVersion: PROTOCOL, type: "aix.view.deck-snapshot", text });
}

function applySeed(body: unknown) {
  const accept = (globalThis as typeof globalThis & { __asterPresentonAcceptHomeSeed?: (value: unknown) => SeedResult })
    .__asterPresentonAcceptHomeSeed;
  if (typeof accept !== "function") {
    postHome({ holdsWork: false, applied: false });
    return;
  }
  const result = accept(body);
  if (!result?.applied) {
    postHome({ holdsWork: Boolean(result?.notice), applied: false, notice: result?.notice ?? null });
    return;
  }
  postHome({ holdsWork: false, applied: true, seed: result.seed });
}

function receiveSeed(body: unknown, delivery: number) {
  homeDelivery = delivery;
  if (!restored) {
    queuedSeed = body;
    hasQueuedSeed = true;
    return;
  }
  applySeed(body);
}

function applyDeck(part: unknown) {
  const apply = (globalThis as typeof globalThis & { __asterPresentonApplyDeckToolPart?: (value: unknown) => unknown })
    .__asterPresentonApplyDeckToolPart;
  const result = typeof apply === "function" ? apply(part) : { status: "idle" };
  reportDeckSnapshot();
  port?.postMessage({ protocolVersion: PROTOCOL, type: "aix.view.deck-result", result });
}

function receiveDeck(part: unknown) {
  if (!restored) {
    queuedDeck.push(part);
    return;
  }
  applyDeck(part);
}

/** Stored state is back. A seed that arrived during connect is applied now, then any queued deck calls. */
export function homeDocumentRestored() {
  restored = true;
  if (hasQueuedSeed) {
    hasQueuedSeed = false;
    const body = queuedSeed;
    queuedSeed = undefined;
    applySeed(body);
  }
  const parts = queuedDeck;
  queuedDeck = [];
  for (const part of parts) applyDeck(part);
}

/** Resolves with the stored state, or null when the view runs outside Studio. */
export function connect(): Promise<string | null> {
  const id = documentId();
  if (!id || window.parent === window) return Promise.resolve(null);
  return new Promise((resolve) => {
    const channel = new MessageChannel();
    port = channel.port1;
    port.onmessage = ({ data }) => {
      if (!data || data.protocolVersion !== PROTOCOL) return;
      if (data.type === "aix.view.connect") {
        resolve(typeof data.stateJSON === "string" ? data.stateJSON : null);
        return;
      }
      if (data.type === "aix.view.seed") {
        receiveSeed(data.seed, typeof data.delivery === "number" ? data.delivery : 0);
        return;
      }
      if (data.type === "aix.view.deck") {
        receiveDeck(data.part);
        return;
      }
      if (data.type !== "aix.agent.result" && data.type !== "aix.agent.refused") return;
      const waiter = pending.get(data.id);
      if (!waiter) return;
      pending.delete(data.id);
      if (data.type === "aix.agent.result") waiter.resolve(data.text);
      else waiter.reject(new AgentError(REFUSALS[data.reason] ?? String(data.reason), String(data.reason)));
    };
    port.start();
    window.parent.postMessage({ protocolVersion: PROTOCOL, type: "aix.document.ready", documentId: id }, "*", [channel.port2]);
  });
}

export function connected(): boolean {
  return port !== null;
}

/** One agent turn; resolves with the turn's output object. */
export async function invoke(parameters: Record<string, unknown> & { step: string }): Promise<any> {
  agentModelRequests += 1;
  if (!port) throw new AgentError(REFUSALS.standalone, "standalone");
  const channel = port;
  const id = ++messageId;
  const text = await new Promise<string>((resolve, reject) => {
    pending.set(id, { resolve, reject });
    channel.postMessage({ protocolVersion: PROTOCOL, type: "aix.agent.invoke", id, agent: AGENT, prompt: `演示文稿 ${parameters.step}`, parameters });
  });
  let result: any;
  try {
    result = JSON.parse(text);
  } catch {
    throw new AgentError("代理返回的不是 JSON。", "agent_failed");
  }
  if (result?.error) throw new AgentError(String(result.error), "agent_failed");
  return result;
}

let storeId = 0;
export function store(stateJSON: string) {
  port?.postMessage({ protocolVersion: PROTOCOL, type: "aix.view.store", id: ++storeId, stateJSON });
}
