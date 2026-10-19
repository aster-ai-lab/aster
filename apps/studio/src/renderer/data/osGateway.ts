import { ENDPOINT_TYPE, type EndpointType } from '@shared/data/types/model'
import { DEFAULT_PROVIDER_SETTINGS, type Provider } from '@shared/data/types/provider'

/** Studio provider id for the signed-in a3s OS gateway. The display name comes from the tenant. */
export const OS_GATEWAY_PROVIDER_ID = 'a3s'

/** Alias the gateway routes to the best available chat model. */
export const OS_GATEWAY_AUTO_MODEL_ID = 'auto'

const DEFAULT_ORIGIN = 'http://127.0.0.1:8080'
const ORIGIN = /^https?:\/\/[^\s/]+$/

const PROTOCOL_ENDPOINTS: Record<string, readonly EndpointType[]> = {
  openai: [
    ENDPOINT_TYPE.OPENAI_CHAT_COMPLETIONS,
    ENDPOINT_TYPE.OPENAI_RESPONSES,
    ENDPOINT_TYPE.OPENAI_TEXT_COMPLETIONS,
    ENDPOINT_TYPE.OPENAI_EMBEDDINGS,
    ENDPOINT_TYPE.OPENAI_IMAGE_GENERATION,
    ENDPOINT_TYPE.OPENAI_TEXT_TO_SPEECH
  ],
  anthropic: [ENDPOINT_TYPE.ANTHROPIC_MESSAGES],
  google: [ENDPOINT_TYPE.GOOGLE_GENERATE_CONTENT],
  ollama: [ENDPOINT_TYPE.OLLAMA_CHAT, ENDPOINT_TYPE.OLLAMA_GENERATE],
  jina: [ENDPOINT_TYPE.JINA_RERANK]
}

/** Gateway origin from the shell, or the local a3s-gateway default. */
export function osGatewayOrigin(host: typeof globalThis = globalThis): string {
  const raw = (host as typeof host & { __A3S_GATEWAY_ORIGIN__?: unknown }).__A3S_GATEWAY_ORIGIN__
  if (typeof raw !== 'string') return DEFAULT_ORIGIN
  const value = raw.trim().replace(/\/+$/, '')
  return ORIGIN.test(value) ? value : DEFAULT_ORIGIN
}

/** Native endpoint adapters. The alias is the model; the path selects the protocol. */
export function osGatewayEndpoints(origin: string): Provider['endpointConfigs'] {
  const root = origin.replace(/\/+$/, '')
  const v1 = `${root}/v1`
  const v1beta = `${root}/v1beta`
  return {
    [ENDPOINT_TYPE.OPENAI_CHAT_COMPLETIONS]: { adapterFamily: 'openai-compatible', baseUrl: v1 },
    [ENDPOINT_TYPE.OPENAI_RESPONSES]: { adapterFamily: 'openai', baseUrl: v1 },
    [ENDPOINT_TYPE.OPENAI_TEXT_COMPLETIONS]: { adapterFamily: 'openai-compatible', baseUrl: v1 },
    [ENDPOINT_TYPE.OPENAI_EMBEDDINGS]: { adapterFamily: 'openai-compatible', baseUrl: v1 },
    [ENDPOINT_TYPE.OPENAI_IMAGE_GENERATION]: { adapterFamily: 'openai-compatible', baseUrl: v1 },
    [ENDPOINT_TYPE.OPENAI_TEXT_TO_SPEECH]: { adapterFamily: 'openai-compatible', baseUrl: v1 },
    [ENDPOINT_TYPE.ANTHROPIC_MESSAGES]: { adapterFamily: 'anthropic', baseUrl: v1 },
    [ENDPOINT_TYPE.GOOGLE_GENERATE_CONTENT]: { adapterFamily: 'google', baseUrl: v1beta },
    [ENDPOINT_TYPE.OLLAMA_CHAT]: { adapterFamily: 'ollama', baseUrl: root },
    [ENDPOINT_TYPE.OLLAMA_GENERATE]: { adapterFamily: 'ollama', baseUrl: root },
    [ENDPOINT_TYPE.JINA_RERANK]: { adapterFamily: 'jina-rerank', baseUrl: root }
  }
}

/** Protocols the organization catalog names, in catalog order, with unknown values dropped. */
export function protocolsToEndpointTypes(protocols: readonly string[] | undefined): EndpointType[] {
  const types: EndpointType[] = []
  for (const protocol of protocols ?? []) {
    for (const type of PROTOCOL_ENDPOINTS[protocol] ?? []) {
      if (!types.includes(type)) types.push(type)
    }
  }
  return types
}

export function osGatewayProvider(name: string, origin: string): Provider {
  return {
    id: OS_GATEWAY_PROVIDER_ID,
    presetProviderId: OS_GATEWAY_PROVIDER_ID,
    name: name.trim() || 'A3S',
    endpointConfigs: osGatewayEndpoints(origin),
    defaultChatEndpoint: ENDPOINT_TYPE.OPENAI_CHAT_COMPLETIONS,
    authMethods: ['api-key'],
    authOptional: false,
    reportsActualCost: false,
    apiKeys: [{ id: `${OS_GATEWAY_PROVIDER_ID}:session`, isEnabled: true }],
    authType: 'api-key',
    settings: DEFAULT_PROVIDER_SETTINGS,
    isEnabled: true
  }
}
