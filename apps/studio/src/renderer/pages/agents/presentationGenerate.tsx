import { getToolName, isToolUIPart } from 'ai'
import { useEffect, useSyncExternalStore } from 'react'

import {
  postAppletViewMessage,
  readAixCatalogue,
  releaseAppletHome,
  stageAppletHomeSeed,
  subscribeAixCatalogue
} from '@renderer/services/plugins'
import type { CherryMessagePart } from '@shared/data/types/message'

import { LANGUAGES } from '../../../../../use-registry/packages/presenton/agent/languages.mjs'
import { armPresentationDeck, clearPresentationDeck } from './presentationDeck'
import { PRESENTON_PACKAGE_ID, PRESENTON_SLUG, PRESENTON_TITLE } from './presentonHomepage'

/** a3s-code MCP tools. The server name is `presenton`. */
export const GENERATE_DECK_TOOL = 'mcp__presenton__generate_deck'
export const READ_DECK_TOOL = 'mcp__presenton__read_deck'
export const EDIT_OUTLINE_TOOL = 'mcp__presenton__edit_outline'
export const EDIT_SLIDE_TOOL = 'mcp__presenton__edit_slide'

const DECK_EDIT_TOOLS = new Set([READ_DECK_TOOL, EDIT_OUTLINE_TOOL, EDIT_SLIDE_TOOL])

const TONES = ['default', 'casual', 'professional', 'funny', 'educational', 'sales_pitch']
const VERBOSITIES = ['concise', 'standard', 'text-heavy']
const DEFAULT_LANGUAGE = 'Chinese (Simplified - 中文, 汉语)'
const MAX_CONTENT = 12_000
const MAX_INSTRUCTIONS = 4_000
const MAX_SLIDES = 20

export interface GenerateDeckSeed {
  content: string
  n_slides: number | null
  language: string
  tone: string
  verbosity: string
  include_title_slide: boolean
  instructions: string
}

const applied = new Set<string>()

export function resetGenerateDeckForTests() {
  applied.clear()
}

/** The installed catalogue still contains this package. */
export function presentonInstalled(): boolean {
  return readAixCatalogue().packages.some(
    (item) => item.slug === PRESENTON_SLUG && item.packageId === PRESENTON_PACKAGE_ID
  )
}

/** Uninstall drops the armed outline, the staged seed, and the remembered tool calls. */
export function releasePresentonControl() {
  applied.clear()
  clearPresentationDeck()
  releaseAppletHome(PRESENTON_SLUG)
}

function closed(value: unknown, options: readonly string[]): value is string {
  return typeof value === 'string' && options.includes(value)
}

/** The same closed sets `acceptHomeSeed` checks. A miss returns null and the canvas stays. */
export function normalizeGenerateDeckSeed(body: unknown): GenerateDeckSeed | null {
  if (!body || typeof body !== 'object' || Array.isArray(body)) return null
  const record = body as Record<string, unknown>
  const content = typeof record.content === 'string' ? record.content.trim() : ''
  if (!content || [...content].length > MAX_CONTENT) return null
  let nSlides: number | null = null
  if (record.n_slides !== undefined && record.n_slides !== null) {
    const n = record.n_slides
    if (typeof n !== 'number' || !Number.isInteger(n) || n < 1 || n > MAX_SLIDES) return null
    nSlides = n
  }
  const language = record.language === undefined ? DEFAULT_LANGUAGE : record.language
  const tone = record.tone === undefined ? 'default' : record.tone
  const verbosity = record.verbosity === undefined ? 'standard' : record.verbosity
  if (!closed(language, LANGUAGES) || !closed(tone, TONES) || !closed(verbosity, VERBOSITIES)) return null
  if (record.include_title_slide !== undefined && typeof record.include_title_slide !== 'boolean') return null
  const instructions = record.instructions === undefined ? '' : record.instructions
  if (typeof instructions !== 'string' || [...instructions].length > MAX_INSTRUCTIONS) return null
  return {
    content,
    n_slides: nSlides,
    language,
    tone,
    verbosity,
    include_title_slide: record.include_title_slide !== false,
    instructions
  }
}

function failedCall(part: CherryMessagePart): boolean {
  if ('errorText' in part && part.errorText) return true
  if (!('output' in part) || !part.output || typeof part.output !== 'object') return false
  return 'isError' in part.output && part.output.isError === true
}

function callId(part: CherryMessagePart): string {
  return 'toolCallId' in part && typeof part.toolCallId === 'string' ? part.toolCallId : ''
}

function readyTool(part: CherryMessagePart): string | null {
  if (!isToolUIPart(part) || !('state' in part) || part.state !== 'output-available' || failedCall(part)) return null
  const id = callId(part)
  if (!id) return null
  return getToolName(part)
}

/** One successful deck tool result. generate_deck seeds the canvas. Read and edit go to the open editor. */
export function applyGenerateDeckPart(sessionId: string, part: CherryMessagePart): 'applied' | 'idle' {
  if (!sessionId || !presentonInstalled()) return 'idle'
  const name = readyTool(part)
  const id = callId(part)
  if (!name || !id || applied.has(id)) return 'idle'
  if (name === GENERATE_DECK_TOOL) {
    const seed = normalizeGenerateDeckSeed('input' in part ? part.input : undefined)
    if (!seed) return 'idle'
    applied.add(id)
    stageAppletHomeSeed(PRESENTON_SLUG, 'workbench', seed)
    armPresentationDeck(sessionId, { title: PRESENTON_TITLE, topic: seed.content, lines: [] })
    return 'applied'
  }
  if (!DECK_EDIT_TOOLS.has(name)) return 'idle'
  applied.add(id)
  postAppletViewMessage(PRESENTON_SLUG, 'workbench', { protocolVersion: 1, type: 'aix.view.deck', part })
  return 'applied'
}

/** Watches the task turn for generate_deck and opens the canvas with that seed. */
export function PresentationGenerateBridge({
  sessionId,
  partsByMessageId
}: {
  sessionId: string
  partsByMessageId: Record<string, CherryMessagePart[]>
}) {
  const installed = useSyncExternalStore(subscribeAixCatalogue, presentonInstalled, presentonInstalled)
  useEffect(() => {
    if (!installed) {
      releasePresentonControl()
      return
    }
    for (const list of Object.values(partsByMessageId)) {
      for (const part of list) applyGenerateDeckPart(sessionId, part)
    }
  }, [installed, sessionId, partsByMessageId])
  return null
}
