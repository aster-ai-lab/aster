/** Homepage composer behavior ported from the Work task composer. Chat composers do not use this. */

import type { WorkbenchTurnPolicy } from '@shared/types/workbenchProof'

export const HOME_COMPOSER_PINNED_TOOL_IDS = ['composer:new-session', 'skills'] as const

const RESEARCH_LINE = '深度研究：只检索和综合，不要修改工作区。'

export interface HomeComposerFlags {
  reviewMode: boolean
  artifactTrace: boolean
  researchMode: boolean
}

export const EMPTY_HOME_COMPOSER_FLAGS: HomeComposerFlags = {
  reviewMode: false,
  artifactTrace: false,
  researchMode: false
}

const OPTIONS_KEY = 'workbench.composer.options.v1'
export const HOME_COMPOSER_OPTIONS_CHANGED = 'a3s:home-composer-options-changed'

export function readHomeComposerOptions(
  storage: Pick<Storage, 'getItem'> | null = defaultStorage()
): HomeComposerFlags {
  if (!storage) return { ...EMPTY_HOME_COMPOSER_FLAGS }
  try {
    const raw = storage.getItem(OPTIONS_KEY)
    if (!raw) return { ...EMPTY_HOME_COMPOSER_FLAGS }
    const parsed = JSON.parse(raw) as Partial<HomeComposerFlags>
    return {
      reviewMode: parsed.reviewMode === true,
      artifactTrace: parsed.artifactTrace === true,
      researchMode: parsed.researchMode === true
    }
  } catch {
    return { ...EMPTY_HOME_COMPOSER_FLAGS }
  }
}

export function writeHomeComposerOptions(
  flags: HomeComposerFlags,
  storage: Pick<Storage, 'setItem'> | null = defaultStorage()
): boolean {
  if (!storage) return false
  try {
    storage.setItem(OPTIONS_KEY, JSON.stringify(flags))
    globalThis.window?.dispatchEvent(new Event(HOME_COMPOSER_OPTIONS_CHANGED))
    return true
  } catch {
    return false
  }
}

function defaultStorage(): Storage | null {
  try {
    return globalThis.localStorage ?? null
  } catch {
    return null
  }
}

/** Per-turn constraints. The agent path sends these beside the user text. */
export function homeTurnDirectives(flags: HomeComposerFlags): string[] {
  return flags.researchMode ? [RESEARCH_LINE] : []
}

export function homeTurnContext(flags: HomeComposerFlags): {
  directives: string[]
  research: boolean
  workbench: WorkbenchTurnPolicy
} {
  return {
    directives: homeTurnDirectives(flags),
    research: flags.researchMode,
    workbench: { reviewMode: flags.reviewMode, artifactTrace: flags.artifactTrace }
  }
}

export function researchBlocksSend(flags: HomeComposerFlags, fileCount: number) {
  return flags.researchMode && fileCount > 0
}

const GOAL_LINE = /(?:^|\n)\/goal(?:[ \t]+([^\n]*))?/

export function composerGoalSummary(text: string): string | null {
  const match = text.match(GOAL_LINE)
  if (!match) return null
  return (match[1] ?? '').trim()
}

const GOAL_VERBS = new Set(['status', 'pause', 'resume', 'clear'])

/** Objective typed after `/goal`. Verbs such as `clear` are not an objective. */
export function composerGoalObjective(text: string): string | null {
  const summary = composerGoalSummary(text)
  if (summary === null) return null
  if (GOAL_VERBS.has(summary.toLowerCase())) return null
  return summary
}

export interface DurableGoalRecord {
  objective: string
  paused: boolean
}

/** `{workspace}/.a3s/durable-goal.json`, the file the agent writes for `/goal`. */
export function parseDurableGoal(raw: string): DurableGoalRecord | null {
  try {
    const value = JSON.parse(raw) as { objective?: unknown; paused?: unknown }
    if (typeof value.objective !== 'string') return null
    const objective = value.objective.trim()
    if (!objective) return null
    return { objective, paused: value.paused === true }
  } catch {
    return null
  }
}

export function durableGoalFile(workspacePath: string): string {
  return `${workspacePath.replace(/[/\\]+$/, '')}/.a3s/durable-goal.json`
}

export interface TaskGoalPresentation {
  text: string
  paused: boolean
  /** A draft line can be removed from the composer. A stored goal is cleared with `/goal clear`. */
  clearable: boolean
}

/**
 * A `/goal` command is the whole user message, matching the agent parser.
 * `texts` are chronological. Later `clear` removes the objective.
 */
export function sessionGoalFromUserTexts(texts: readonly string[]): DurableGoalRecord | null {
  let goal: DurableGoalRecord | null = null
  for (const raw of texts) {
    const text = raw.trim()
    if (!text.startsWith('/goal')) continue
    const rest = text.slice('/goal'.length)
    if (rest.length > 0 && !/^\s/.test(rest)) continue
    const args = rest.trim()
    const verb = args.toLowerCase()
    if (verb === '' || verb === 'status') continue
    if (verb === 'pause') {
      if (goal) goal = { objective: goal.objective, paused: true }
      continue
    }
    if (verb === 'resume') {
      if (goal) goal = { objective: goal.objective, paused: false }
      continue
    }
    if (verb === 'clear') {
      goal = null
      continue
    }
    goal = { objective: args, paused: false }
  }
  return goal
}

export function userTextsFromSessionMessages(
  items: readonly { role?: string; searchableText?: string }[] | null | undefined
): string[] {
  if (!items) return []
  const texts: string[] = []
  for (const item of items) {
    if (item.role !== 'user') continue
    const text = item.searchableText?.trim()
    if (text) texts.push(text)
  }
  return texts
}

/** The draft the user is typing, otherwise the goal this conversation already set. */
export function taskGoalPresentation(
  draft: string | null,
  sessionGoal: DurableGoalRecord | null
): TaskGoalPresentation | null {
  const typed = draft?.trim() ?? ''
  if (typed) return { text: typed, paused: false, clearable: true }
  if (!sessionGoal) return null
  return { text: sessionGoal.objective, paused: sessionGoal.paused, clearable: false }
}

export function withoutComposerGoal(text: string): string {
  return text
    .replace(/(?:^|\n)\/goal(?:[ \t]+[^\n]*)?/, '')
    .replace(/^\n+/, '')
    .trim()
}

export interface HomePluginCandidate {
  id: string
  name: string
  description?: string
  isActive: boolean
}

export function activePluginCandidates(servers: readonly HomePluginCandidate[], query: string) {
  const needle = query.trim().toLowerCase()
  return servers.filter((server) => {
    if (!server.isActive) return false
    if (!needle) return true
    return (
      server.name.toLowerCase().includes(needle) ||
      server.id.toLowerCase().includes(needle) ||
      (server.description ?? '').toLowerCase().includes(needle)
    )
  })
}

export function pluginInsertionText(server: Pick<HomePluginCandidate, 'id' | 'name'>) {
  return `Use the MCP server "${server.name}" (id: ${server.id}).`
}

export interface SpeechRecognitionResultLike {
  readonly isFinal: boolean
  readonly [index: number]: { transcript: string } | undefined
}

export function recognitionEventTranscripts(
  results: {
    readonly length: number
    readonly [index: number]: SpeechRecognitionResultLike | undefined
  },
  resultIndex: number
) {
  let finalTranscript = ''
  let previewTranscript = ''
  for (let index = 0; index < results.length; index += 1) {
    const result = results[index]
    const transcript = result?.[0]?.transcript ?? ''
    previewTranscript += transcript
    if (index >= resultIndex && result?.isFinal) finalTranscript += transcript
  }
  return {
    finalTranscript: finalTranscript.trim(),
    previewTranscript: previewTranscript.trim()
  }
}

export function insertVoiceTranscript(
  value: string,
  transcript: string,
  selectionStart: number,
  selectionEnd: number
): { cursor: number; value: string } {
  const spoken = transcript.trim()
  if (!spoken) return { cursor: selectionStart, value }
  const start = Math.max(0, Math.min(selectionStart, value.length))
  const end = Math.max(start, Math.min(selectionEnd, value.length))
  const before = value.slice(0, start)
  const after = value.slice(end)
  const needsLeadingSpace = /[\p{L}\p{N}]$/u.test(before) && /^[A-Za-z0-9]/u.test(spoken)
  const needsTrailingSpace = /^[A-Za-z0-9]/u.test(after) && /[A-Za-z0-9]$/u.test(spoken)
  const insertion = `${needsLeadingSpace ? ' ' : ''}${spoken}${needsTrailingSpace ? ' ' : ''}`
  return {
    cursor: before.length + insertion.length,
    value: `${before}${insertion}${after}`
  }
}

export interface ScreenshotPin {
  x: number
  y: number
  note: string
}

export function mergeScreenshotNotes(text: string, pins: readonly ScreenshotPin[]) {
  const notes = pins.map(
    (pin, index) => `${index + 1}. ${pin.note.trim() || `(${Math.round(pin.x * 100)}%, ${Math.round(pin.y * 100)}%)`}`
  )
  if (notes.length === 0) return text
  const block = `截图标注：\n${notes.join('\n')}`
  if (text.includes(block)) return text
  const trimmed = text.replace(/\s+$/, '')
  return trimmed ? `${trimmed}\n\n${block}` : block
}

export function screenshotFileName(capturedAt = new Date()) {
  const pad = (value: number) => value.toString().padStart(2, '0')
  return `screenshot-${capturedAt.getFullYear()}${pad(capturedAt.getMonth() + 1)}${pad(capturedAt.getDate())}-${pad(capturedAt.getHours())}${pad(capturedAt.getMinutes())}${pad(capturedAt.getSeconds())}.png`
}

export function sameToolOrder(left: readonly string[], right: readonly string[]) {
  return left.length === right.length && left.every((id, index) => id === right[index])
}
