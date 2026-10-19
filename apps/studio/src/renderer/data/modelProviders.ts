import type { EndpointType } from '@shared/data/types/model'

/** One model row in the picker, keyed by the provider that names it. */
export type ModelEntry = {
  key: string
  providerId: string
  providerName: string
  id: string
  name: string
  description?: string
  family?: string
  reasoning: boolean
  toolCall: boolean
  attachment: boolean
  contextWindow: number | null
  outputLimit: number | null
  inputModalities: string[]
  outputModalities: string[]
  inputCost: number | null
  outputCost: number | null
  /** Prompt-cache hit, USD per million tokens. Absent when unpublished. */
  cacheReadCost?: number | null
  /** Prompt-cache write, USD per million tokens. Absent when unpublished. */
  cacheWriteCost?: number | null
  /** Billing coefficient on the published rate. 1 charges the list price. */
  priceMultiplier?: number | null
  /** Vendor icon id when it differs from the group. */
  logoId?: string
  /** Native gateway paths this alias can use. Absent means the provider default. */
  endpointTypes?: EndpointType[]
  /**
   * Published intensity levels. Absent means infer from the model id.
   * An empty list means the card has no intensity slider.
   */
  reasoningEfforts?: readonly string[] | null
}

/**
 * Studio provider registry ids for providers commonly known by another id (as in a
 * model's `provider/model` key). Every other id is looked up unchanged.
 */
const STUDIO_ALIASES: Record<string, string> = {
  zhipu: 'zhipuai',
  moonshot: 'moonshotai-cn',
  'moonshot-global': 'moonshotai',
  dashscope: 'alibaba-cn',
  gemini: 'google',
  vertexai: 'google-vertex',
  grok: 'xai',
  silicon: 'siliconflow-cn',
  together: 'togetherai',
  fireworks: 'fireworks-ai',
  doubao: 'volcengine',
  'aws-bedrock': 'amazon-bedrock',
  'azure-openai': 'azure',
  copilot: 'github-copilot',
  qiniu: 'qiniu-ai',
  minimax: 'minimax-cn',
  'minimax-global': 'minimax',
  mimo: 'xiaomi'
}

/** Model vendors' own platforms, in display order. */
export const OFFICIAL_PROVIDERS: readonly string[] = [
  'deepseek',
  'zhipuai',
  'moonshotai-cn',
  'alibaba-cn',
  'minimax-cn',
  'volcengine',
  'openai',
  'anthropic',
  'google',
  'xai'
]

/**
 * Brand icon keys for provider ids that differ from the icon registry.
 * A rule matches the id itself or any `<prefix>-…` variant (regional or plan editions).
 */
const LOGO_PREFIXES: readonly (readonly [string, string])[] = [
  ['zhipuai', 'zhipu'],
  ['zai', 'z-ai'],
  ['moonshotai', 'moonshot'],
  ['kimi', 'moonshot'],
  ['alibaba', 'bailian'],
  ['xai', 'grok'],
  ['xiaomi', 'xiaomimimo'],
  ['minimax', 'minimax'],
  ['siliconflow', 'silicon'],
  ['togetherai', 'together'],
  ['fireworks', 'fireworks'],
  ['amazon-bedrock', 'aws-bedrock'],
  ['azure', 'azureai'],
  ['google-vertex', 'vertexai'],
  ['volcengine', 'volcengine'],
  ['stepfun', 'step'],
  ['qiniu', 'qiniu'],
  ['cloudflare-workers-ai', 'workers-ai'],
  ['cloudflare', 'cloudflare'],
  ['tencent', 'tencent-cloud-ti'],
  ['perplexity', 'perplexity'],
  ['llama', 'meta'],
  ['ollama', 'ollama'],
  ['opencode', 'opencode'],
  ['nvidia', 'nvidia']
]

/** The icon registry key for a provider id; unmapped ids are returned unchanged. */
export function providerLogoId(providerId: string): string {
  const rule = LOGO_PREFIXES.find(([prefix]) => providerId === prefix || providerId.startsWith(`${prefix}-`))
  return rule ? rule[1] : providerId
}

const ALIAS_TO_STUDIO = new Map(Object.entries(STUDIO_ALIASES).map(([studio, alias]) => [alias, studio]))

/** The registry provider serving a provider id, if Studio knows it. */
export function studioProviderId(providerId: string, knownStudioIds: ReadonlySet<string>): string | undefined {
  const mapped = ALIAS_TO_STUDIO.get(providerId)
  if (mapped) return knownStudioIds.has(mapped) ? mapped : undefined
  if (STUDIO_ALIASES[providerId]) return undefined
  return knownStudioIds.has(providerId) ? providerId : undefined
}
