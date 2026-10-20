import { getToolName, isToolUIPart } from 'ai'

import { mapCitationMarksToTags } from '@renderer/utils/citation'
import {
  resolveCitationMarkerParts,
  resolveMessageCitations,
  stripCitationMarkers
} from '@renderer/utils/message/citations'
import type { CherryMessagePart } from '@shared/data/types/message'

import { extractTaskCodeDiff, isMutation, type TaskCodeDiff } from './taskDiff'
import { taskPlanSteps, type TaskPlanStep } from './taskPlanProjection'

export type TaskToolState = 'pending' | 'running' | 'success' | 'error'

export type TaskTurnItem =
  | { kind: 'prose'; id: string; text: string; streaming: boolean }
  | {
      kind: 'tool'
      id: string
      name: string
      state: TaskToolState
      partIndex?: number
      input?: Record<string, unknown>
    }
  | { kind: 'plan'; id: string; steps: TaskPlanStep[] }
  | { kind: 'note'; id: string; text: string }
  | { kind: 'block'; id: string; index: number; text?: string; quiet?: boolean }

/** Transport parts the classic transcript also skips. `source-url` still feeds citation resolution. */
const HIDDEN_TURN_PARTS = new Set([
  'step-start',
  'source-url',
  'source-document',
  'data-citation',
  'data-agent-task-event',
  'data-task-plan',
  'data-knowledge-scope',
  'data-review-source',
  'data-review-progress',
  'data-clear'
])

const BLOCK_TURN_PARTS = new Set([
  'reasoning',
  'file',
  'data-translation',
  'data-video',
  'data-form',
  'data-compact',
  'data-compaction-anchor',
  'data-conversation-reset',
  'data-agent-session-fork',
  'data-retry'
])

interface LeakedCall {
  name: string
  input: Record<string, unknown>
}

type LeakedSegment = { kind: 'prose'; text: string } | { kind: 'call'; call: LeakedCall; closed: boolean }

const OPEN_TAG = '<tool_call>'
const CLOSE_TAG = '</tool_call>'

const MERMAID_DIRECTIVES = [
  'architecture-beta',
  'sequenceDiagram',
  'classDiagram',
  'stateDiagram-v2',
  'stateDiagram',
  'requirementDiagram',
  'quadrantChart',
  'xychart-beta',
  'block-beta',
  'sankey-beta',
  'packet-beta',
  'C4Deployment',
  'C4Component',
  'C4Container',
  'C4Dynamic',
  'C4Context',
  'erDiagram',
  'gitGraph',
  'flowchart',
  'mindmap',
  'timeline',
  'journey',
  'kanban',
  'gantt',
  'graph',
  'pie'
].sort((left, right) => right.length - left.length || left.localeCompare(right))

/**
 * Split a model answer into prose and tool rows in the order the text arrived.
 * A call that cannot be parsed stays in the prose. Calls inside a code fence stay prose.
 * DeepSeek writes the same call as `<｜DSML｜invoke name="…">`.
 */
export function splitLeakedToolCallSegments(text: string): LeakedSegment[] {
  const segments: LeakedSegment[] = []
  let prose = ''
  let rest = text

  while (rest.length > 0) {
    const toolAt = rest.indexOf(OPEN_TAG)
    const dsml = findDsmlInvoke(rest)
    const toolStart = toolAt >= 0 ? toolAt : -1
    const dsmlStart = dsml?.index ?? -1
    if (toolStart < 0 && dsmlStart < 0) break

    const useDsml = dsmlStart >= 0 && (toolStart < 0 || dsmlStart < toolStart)
    const start = useDsml ? dsmlStart : toolStart
    const absolute = text.length - rest.length + start
    if (fenceIsOpen(text, absolute)) {
      const skip = useDsml ? (dsml?.openLength ?? 1) : OPEN_TAG.length
      prose += rest.slice(0, start + skip)
      rest = rest.slice(start + skip)
      continue
    }

    prose += rest.slice(0, start)
    if (!useDsml) {
      const after = rest.slice(start + OPEN_TAG.length)
      const taken = takeCallBody(after)
      const call = parseCallBody(taken.body)
      if (call) {
        const cleaned = cleanProse(prose)
        if (cleaned) segments.push({ kind: 'prose', text: cleaned })
        prose = ''
        segments.push({ kind: 'call', call, closed: taken.closed })
      } else {
        prose += `${OPEN_TAG}${taken.body}`
      }
      rest = after.slice(taken.consumed)
      continue
    }

    const taken = takeDsmlInvoke(rest.slice(start))
    if (taken.call) {
      const cleaned = cleanProse(prose)
      if (cleaned) segments.push({ kind: 'prose', text: cleaned })
      prose = ''
      segments.push({ kind: 'call', call: taken.call, closed: taken.closed })
    } else if (!taken.hide) {
      prose += rest.slice(start, start + taken.consumed)
    }
    rest = rest.slice(start + Math.max(taken.consumed, 1))
  }

  prose += rest
  const cleaned = cleanProse(prose)
  if (cleaned) segments.push({ kind: 'prose', text: cleaned })
  return segments
}

/**
 * Project one assistant turn into arrival order.
 * Structured tool parts and a leaked `<tool_call>` or DeepSeek DSML payload share one row shape.
 * A leaked call is dropped only when a structured tool has the same name and target.
 * Reasoning, files, translation, video, and compaction stay in place for the existing blocks.
 * A tool call whose form is already a form card in the turn has no separate row.
 */
export function projectTaskConversationTurn(parts: readonly CherryMessagePart[], streaming: boolean): TaskTurnItem[] {
  const structuredCalls = structuredCallIdentities(parts)
  const formSpecs = shownFormSpecs(parts)
  const items: TaskTurnItem[] = []
  let leaked = 0
  let proseKey = 0

  const pushLeaked = (call: LeakedCall, closed: boolean) => {
    if (leakedDuplicatesStructured(call, structuredCalls)) return
    leaked += 1
    const input = call.input
    items.push({
      kind: 'tool',
      id: `leaked-tool-${leaked}`,
      name: call.name,
      state: streaming && !closed ? 'running' : 'success',
      ...(Object.keys(input).length > 0 ? { input } : {})
    })
  }

  for (const [index, part] of parts.entries()) {
    if (part.type === 'text') {
      for (const segment of splitLeakedToolCallSegments(part.text ?? '')) {
        if (segment.kind === 'prose') {
          pushProse(items, segment.text, () => `prose-${proseKey++}`)
          continue
        }
        pushLeaked(segment.call, segment.closed)
      }
      continue
    }

    if (part.type === 'reasoning') {
      const raw = part.text ?? ''
      if (!raw.includes('DSML') && !raw.includes(OPEN_TAG)) {
        if (isBlankReasoning(part)) continue
        items.push({ kind: 'block', id: `block-${index}`, index })
        continue
      }
      let pending = ''
      let proseCount = 0
      const flushThought = () => {
        if (!pending.trim()) return
        proseCount += 1
        items.push({ kind: 'block', id: `block-${index}-${proseCount}`, index, text: pending })
        pending = ''
      }
      for (const segment of splitLeakedToolCallSegments(raw)) {
        if (segment.kind === 'prose') {
          pending = pending ? `${pending}\n\n${segment.text}` : segment.text
          continue
        }
        flushThought()
        pushLeaked(segment.call, segment.closed)
      }
      flushThought()
      continue
    }

    const fenced = readDataCodeFence(part)
    if (fenced) {
      pushProse(items, fenced, () => `prose-${proseKey++}`)
      continue
    }

    const note = readErrorNote(part)
    if (note) {
      items.push({ kind: 'note', id: `error-${index}`, text: note })
      continue
    }

    if (isToolUIPart(part) && part.state === 'output-available') {
      const steps = taskPlanSteps(part.input)
      if (steps) {
        items.push({ kind: 'plan', id: `plan-${index}`, steps })
        continue
      }
    }
    const tool = readStructuredTool(part, index)
    if (tool) {
      if (!asksShownForm(part, formSpecs)) items.push(tool)
      continue
    }

    if (isToolUIPart(part) || HIDDEN_TURN_PARTS.has(part.type) || isBlankReasoning(part)) continue
    if (!BLOCK_TURN_PARTS.has(part.type)) continue
    items.push({
      kind: 'block',
      id: `block-${index}`,
      index,
      ...(part.type === 'data-form' ? { quiet: true } : {})
    })
  }

  presentProseCitations(items, parts)
  if (!streaming) {
    for (const item of items) {
      if (item.kind === 'prose') item.text = shortenPresentedArtifactPaths(item.text)
    }
  }

  if (streaming) {
    for (let index = items.length - 1; index >= 0; index -= 1) {
      const item = items[index]
      if (item?.kind !== 'prose') continue
      items[index] = { ...item, streaming: true }
      break
    }
  }

  return items
}

/**
 * Repair a glued mermaid opener and a closed mermaid body.
 * An open fence is never given a closing fence.
 */
export function normalizeTaskConversationMarkdown(source: string): string {
  let result = ''
  let cursor = 0

  while (cursor < source.length) {
    const open = source.indexOf('```', cursor)
    if (open < 0) {
      result += source.slice(cursor)
      break
    }

    result += source.slice(cursor, open)
    const lineEnd = source.indexOf('\n', open)
    const infoEnd = lineEnd < 0 ? source.length : lineEnd
    const info = source.slice(open + 3, infoEnd)
    const repair = repairMermaidOpener(info)
    const language = fenceLanguage(repair ? 'mermaid' : info)
    const closer = lineEnd < 0 ? -1 : source.indexOf('```', lineEnd + 1)

    if (repair) {
      result += '```mermaid'
      if (closer < 0) {
        result += `\n${repair}`
        if (lineEnd >= 0) result += source.slice(lineEnd)
        break
      }
      const body = source.slice(lineEnd + 1, closer)
      result += `\n${prepareMermaidDiagram(`${repair}\n${body}`)}\n\`\`\``
      cursor = closer + 3
      continue
    }

    if (language === 'mermaid' && closer >= 0 && lineEnd >= 0) {
      const body = source.slice(lineEnd + 1, closer)
      result += `${source.slice(open, lineEnd + 1)}${prepareMermaidDiagram(body)}\n\`\`\``
      cursor = closer + 3
      continue
    }

    if (closer < 0) {
      result += source.slice(open)
      break
    }

    result += source.slice(open, closer + 3)
    cursor = closer + 3
  }

  return result
}

/**
 * The markdown parser treats a fence with no closer as a finished block.
 * Peel that tail off so a settled turn still shows it as source.
 */
export function splitTrailingOpenFence(source: string): {
  closed: string
  open?: { language: string; code: string }
} {
  let cursor = 0
  while (cursor < source.length) {
    const open = source.indexOf('```', cursor)
    if (open < 0) break
    const lineEnd = source.indexOf('\n', open)
    const infoEnd = lineEnd < 0 ? source.length : lineEnd
    const language = fenceLanguage(source.slice(open + 3, infoEnd))
    const closer = lineEnd < 0 ? -1 : source.indexOf('```', lineEnd + 1)
    if (closer < 0) {
      return {
        closed: source.slice(0, open).replace(/\s+$/u, ''),
        open: {
          language,
          code: lineEnd < 0 ? '' : source.slice(lineEnd + 1)
        }
      }
    }
    cursor = closer + 3
  }
  return { closed: source }
}

/** Clean a closed mermaid body so the diagram type sits on its own line. */
export function prepareMermaidDiagram(code: string): string {
  const cleaned = code
    .replace(/^[ \t]*%%\{init:[^%\n]*%%[ \t]*\n?/gim, '')
    .replace(/\bend[ \t]+subgraph\b/gi, 'end')
    .replace(/\*\*/g, '')
    .replace(/^\n+/u, '')
    .replace(/\s+$/u, '')
  return splitMermaidHeader(cleaned)
}

function presentProseCitations(items: TaskTurnItem[], parts: readonly CherryMessagePart[]) {
  const prose = items.filter((item): item is Extract<TaskTurnItem, { kind: 'prose' }> => item.kind === 'prose')
  if (prose.length === 0) return
  const resolved = resolveCitationMarkerParts(
    prose.map((item) => item.text),
    resolveMessageCitations(parts)
  )
  for (const [index, item] of prose.entries()) {
    const projection = resolved[index]
    const tagged = projection ? mapCitationMarksToTags(projection.content, projection.byMarker) : item.text
    item.text = stripCitationMarkers(tagged)
  }
}

function isBlankReasoning(part: CherryMessagePart): boolean {
  return part.type === 'reasoning' && !(part.text ?? '').trim()
}

function pushProse(items: TaskTurnItem[], text: string, nextId: () => string) {
  const last = items.at(-1)
  if (last?.kind === 'prose') {
    last.text = `${last.text}\n\n${text}`
    return
  }
  items.push({ kind: 'prose', id: nextId(), text, streaming: false })
}

function readDataCodeFence(part: CherryMessagePart): string | undefined {
  if (part.type !== 'data-code' || !('data' in part) || !part.data || typeof part.data !== 'object') return undefined
  const data = part.data as { content?: string; language?: string }
  if (!data.content?.trim()) return undefined
  return `\`\`\`${data.language ?? ''}\n${data.content}\n\`\`\``
}

function readErrorNote(part: CherryMessagePart): string | undefined {
  if (part.type !== 'data-error' || !('data' in part) || !part.data || typeof part.data !== 'object') return undefined
  const message = (part.data as { message?: string | null }).message?.trim()
  return message || undefined
}

export type TaskWorkspaceProductKind = 'page' | 'image' | 'document' | 'code' | 'file'

export interface TaskWorkspaceProduct {
  path: string
  name: string
  kind: TaskWorkspaceProductKind
}

const PRODUCT_LIMIT = 12

/**
 * Files a finished turn wrote or edited, plus local pages announced in the reply.
 * Hidden while streaming. A failed tool, and a read tool's path, are not products.
 */
export function workspaceProductsForTurn(
  parts: readonly CherryMessagePart[],
  streaming: boolean
): TaskWorkspaceProduct[] {
  if (streaming) return []
  const products: TaskWorkspaceProduct[] = []
  const seen = new Set<string>()
  const push = (path: string) => {
    if (products.length >= PRODUCT_LIMIT || seen.has(path)) return
    const name = fileNameOf(path)
    if (!name) return
    seen.add(path)
    products.push({ path, name, kind: productKind(name) })
  }
  for (const part of parts) {
    if (part.type === 'text') {
      for (const path of presentedArtifactPaths(part.text ?? '')) push(path)
      continue
    }
    if (!isToolUIPart(part) || structuredToolState(part) !== 'success') continue
    for (const path of filePathsOnTool(part)) push(path)
    if (!announcesFiles(getToolName(part).trim().toLowerCase())) continue
    for (const text of shellTexts(part)) {
      for (const path of presentedArtifactPaths(text)) push(path)
    }
  }
  return products
}

export interface TurnFileEdit {
  path: string
  diff: TaskCodeDiff
}

/** Successful file edits in a finished turn. The last edit of a path wins. Reads and failures stay out. */
export function turnFileEdits(parts: readonly CherryMessagePart[], streaming: boolean): TurnFileEdit[] {
  if (streaming) return []
  const edits: TurnFileEdit[] = []
  const indexByPath = new Map<string, number>()
  const put = (path: string, diff: TaskCodeDiff) => {
    const existing = indexByPath.get(path)
    if (existing === undefined) {
      indexByPath.set(path, edits.length)
      edits.push({ path, diff })
    } else {
      edits[existing] = { path, diff }
    }
  }
  for (const part of parts) {
    if (!isToolUIPart(part) || structuredToolState(part) !== 'success') continue
    const name = getToolName(part).trim()
    if (!name || !isMutation(name.toLowerCase())) continue
    const diff = extractTaskCodeDiff(name, part)
    const paths = filePathsOnTool(part)
    const diffPath = diff?.path?.trim()
    if (diffPath && !paths.includes(diffPath)) paths.unshift(diffPath)
    for (const path of paths) {
      const trimmed = path.trim()
      if (!trimmed) continue
      put(trimmed, diff && diffPath === trimmed ? diff : pathOnlyEdit(trimmed))
    }
  }
  return edits
}

function pathOnlyEdit(path: string): TaskCodeDiff {
  return {
    path,
    operation: 'edit',
    lines: [],
    additions: 0,
    deletions: 0,
    comparisonComplete: false,
    truncated: false
  }
}

export interface TurnToolEvidence {
  name: string
  ok: boolean
  paths: string[]
  detail: string
  error: string
}

/** Settled tools in this turn. A running call is not evidence yet. */
export function turnToolEvidence(parts: readonly CherryMessagePart[]): TurnToolEvidence[] {
  const rows: TurnToolEvidence[] = []
  for (const part of parts) {
    if (!isToolUIPart(part)) continue
    const name = getToolName(part).trim()
    if (!name) continue
    const state = structuredToolState(part)
    if (state === 'running' || state === 'pending') continue
    const paths = filePathsOnTool(part)
    if (announcesFiles(name.toLowerCase())) {
      for (const text of shellTexts(part)) {
        for (const path of presentedArtifactPaths(text)) {
          if (!paths.includes(path)) paths.push(path)
        }
      }
    }
    rows.push({
      name,
      ok: state === 'success',
      paths,
      detail: toolDetail(part, paths),
      error: state === 'error' ? toolErrorText(part) : ''
    })
  }
  return rows
}

function toolDetail(part: CherryMessagePart, paths: string[]): string {
  const input = 'input' in part ? part.input : undefined
  if (input && typeof input === 'object' && !Array.isArray(input)) {
    const record = input as Record<string, unknown>
    for (const key of ['command', 'cmd', 'file_path', 'filePath', 'path']) {
      const value = record[key]
      if (typeof value === 'string' && value.trim()) return value.trim().slice(0, 180)
    }
  }
  return paths[0] ?? ''
}

function toolErrorText(part: CherryMessagePart): string {
  if ('errorText' in part && typeof part.errorText === 'string' && part.errorText.trim()) {
    return part.errorText.trim().slice(0, 180)
  }
  const output = 'output' in part ? part.output : undefined
  if (typeof output === 'string' && output.trim()) return output.trim().slice(0, 180)
  return ''
}

const PAGE_EXTENSIONS = new Set(['html', 'htm'])
const IMAGE_EXTENSIONS = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'bmp'])
const DOCUMENT_EXTENSIONS = new Set([
  'md',
  'markdown',
  'pdf',
  'txt',
  'csv',
  'docx',
  'pptx',
  'xlsx',
  'xls',
  'ppt',
  'doc'
])
const CODE_EXTENSIONS = new Set([
  'ts',
  'tsx',
  'js',
  'jsx',
  'py',
  'rs',
  'go',
  'css',
  'scss',
  'json',
  'yml',
  'yaml',
  'toml',
  'sh'
])
const PRESENTED_EXTENSIONS = new Set([
  ...PAGE_EXTENSIONS,
  ...IMAGE_EXTENSIONS,
  ...DOCUMENT_EXTENSIONS,
  ...CODE_EXTENSIONS
])

function productKind(name: string): TaskWorkspaceProductKind {
  const extension = extensionOf(name)
  if (!extension) return 'file'
  if (PAGE_EXTENSIONS.has(extension)) return 'page'
  if (IMAGE_EXTENSIONS.has(extension)) return 'image'
  if (DOCUMENT_EXTENSIONS.has(extension)) return 'document'
  if (CODE_EXTENSIONS.has(extension)) return 'code'
  return 'file'
}

function announcesFiles(name: string): boolean {
  return /^(bash|shell|exec|execute|run_terminal_cmd|run_command|terminal)($|[_-])/.test(name)
}

function shellTexts(part: CherryMessagePart): string[] {
  const record = part as { input?: unknown; output?: unknown }
  const texts: string[] = []
  collectShortStrings(record.input, texts)
  collectShortStrings(record.output, texts)
  return texts
}

function collectShortStrings(value: unknown, texts: string[]) {
  if (typeof value === 'string') {
    if (value.length > 0 && value.length <= 20_000) texts.push(value)
    return
  }
  if (Array.isArray(value)) {
    for (const item of value) collectShortStrings(item, texts)
    return
  }
  if (!value || typeof value !== 'object') return
  for (const item of Object.values(value as Record<string, unknown>)) collectShortStrings(item, texts)
}

const FILE_URL_PATTERN = /file:\/\/[^\s<>"'`]+/gi
const POSIX_PATH_PATTERN = /(?:^|[\s`'"(（[【])(\/[^\s`'")）】]+)/gu
const WINDOWS_PATH_PATTERN = /(?:^|[\s`'"(（[【])([A-Za-z]:\\(?:[^\\/:*?"<>|\r\n]+\\)*[^\\/:*?"<>|\r\n]+)/g
const TRAILING_PUNCTUATION = /[.,;:!?，。；、)）\]】>]+$/u

/** Local files named in a finished reply. Paths inside a code fence stay put. */
export function presentedArtifactPaths(text: string): string[] {
  const paths: string[] = []
  visitOutsideFences(text, (slice, fenced) => {
    if (!fenced) collectPresentedPaths(slice, paths)
  })
  return paths
}

/** Replace an announced local path with its file name. Code fences stay intact. */
export function shortenPresentedArtifactPaths(text: string): string {
  let result = ''
  visitOutsideFences(text, (slice, fenced) => {
    result += fenced ? slice : replacePresentedPaths(slice)
  })
  return result
}

function visitOutsideFences(source: string, visit: (slice: string, fenced: boolean) => void) {
  let cursor = 0
  while (cursor < source.length) {
    const open = source.indexOf('```', cursor)
    if (open < 0) {
      visit(source.slice(cursor), false)
      return
    }
    visit(source.slice(cursor, open), false)
    const lineEnd = source.indexOf('\n', open)
    const closer = lineEnd < 0 ? -1 : source.indexOf('```', lineEnd + 1)
    if (closer < 0) {
      visit(source.slice(open), true)
      return
    }
    visit(source.slice(open, closer + 3), true)
    cursor = closer + 3
  }
}

function collectPresentedPaths(slice: string, paths: string[]) {
  for (const span of presentedSpans(slice)) pushPath(paths, span.path)
}

function replacePresentedPaths(slice: string): string {
  const spans = presentedSpans(slice).sort((left, right) => right.start - left.start)
  let result = slice
  for (const span of spans) {
    result = `${result.slice(0, span.start)}${span.name}${result.slice(span.end)}`
  }
  return result
}

interface PresentedSpan {
  start: number
  end: number
  path: string
  name: string
}

function presentedSpans(slice: string): PresentedSpan[] {
  const spans: PresentedSpan[] = []
  const occupied: Array<[number, number]> = []
  for (const match of slice.matchAll(FILE_URL_PATTERN)) {
    const raw = match[0].replace(TRAILING_PUNCTUATION, '')
    const path = fileUrlToArtifactPath(raw)
    const name = path ? fileNameOf(path) : ''
    if (!path || !name || !PRESENTED_EXTENSIONS.has(extensionOf(name) ?? '')) continue
    const start = match.index ?? 0
    const end = start + raw.length
    spans.push({ start, end, path, name })
    occupied.push([start, end])
  }
  collectBarePaths(slice, POSIX_PATH_PATTERN, spans, occupied)
  collectBarePaths(slice, WINDOWS_PATH_PATTERN, spans, occupied)
  return spans
}

function collectBarePaths(slice: string, pattern: RegExp, spans: PresentedSpan[], occupied: Array<[number, number]>) {
  for (const match of slice.matchAll(pattern)) {
    const raw = match[1]
    if (!raw) continue
    const path = raw.replace(TRAILING_PUNCTUATION, '')
    const name = fileNameOf(path)
    if (!name || !PRESENTED_EXTENSIONS.has(extensionOf(name) ?? '')) continue
    const start = (match.index ?? 0) + match[0].length - raw.length
    const end = start + path.length
    if (occupied.some(([from, to]) => start < to && end > from)) continue
    spans.push({ start, end, path, name })
    occupied.push([start, end])
  }
}

function fileUrlToArtifactPath(value: string): string | undefined {
  try {
    const url = new URL(value)
    if (url.protocol !== 'file:') return undefined
    let path = decodeURIComponent(url.pathname)
    if (/^\/[A-Za-z]:\//.test(path)) path = path.slice(1)
    return path
  } catch {
    return undefined
  }
}

const MUTATION_PATH_KEYS = ['file_path', 'filePath'] as const

function filePathsOnTool(part: CherryMessagePart): string[] {
  const record = part as { files?: unknown; content?: unknown; output?: unknown; input?: unknown }
  const paths: string[] = []
  collectFileList(paths, record.files)
  collectDiffItems(paths, record.content)
  collectOutputFiles(paths, record.output)
  if (isMutation(getToolName(part).trim().toLowerCase())) {
    collectNamedPaths(paths, record.input)
    collectNamedPaths(paths, record.output)
  }
  return paths
}

function collectNamedPaths(paths: string[], value: unknown) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return
  const record = value as Record<string, unknown>
  for (const key of MUTATION_PATH_KEYS) pushPath(paths, record[key])
}

function collectFileList(paths: string[], value: unknown) {
  if (!Array.isArray(value)) return
  for (const item of value) {
    if (typeof item === 'string') pushPath(paths, item)
  }
}

function collectDiffItems(paths: string[], value: unknown) {
  if (!Array.isArray(value)) return
  for (const item of value) {
    if (!item || typeof item !== 'object') continue
    const entry = item as { type?: unknown; path?: unknown }
    if (entry.type === 'diff') pushPath(paths, entry.path)
  }
}

function collectOutputFiles(paths: string[], value: unknown) {
  if (!value || typeof value !== 'object') return
  const output = value as { type?: unknown; path?: unknown; files?: unknown; content?: unknown }
  if (output.type === 'diff') pushPath(paths, output.path)
  collectFileList(paths, output.files)
  collectDiffItems(paths, output.content)
}

function pushPath(paths: string[], value: unknown) {
  if (typeof value !== 'string') return
  const path = value.trim()
  if (!path || paths.includes(path)) return
  paths.push(path)
}

function fileNameOf(path: string): string {
  const normalized = path.replace(/\\/g, '/').replace(/\/+$/u, '')
  return normalized.slice(normalized.lastIndexOf('/') + 1)
}

function extensionOf(name: string): string | undefined {
  const dot = name.lastIndexOf('.')
  if (dot <= 0 || dot === name.length - 1) return undefined
  return name.slice(dot + 1).toLowerCase()
}

const TARGET_KEYS = ['path', 'file_path', 'filePath', 'filename', 'file', 'command', 'cmd', 'query', 'pattern', 'q']

function structuredCallIdentities(parts: readonly CherryMessagePart[]): Array<{ name: string; target: string }> {
  const identities: Array<{ name: string; target: string }> = []
  for (const part of parts) {
    if (!isToolUIPart(part)) continue
    const name = getToolName(part).trim().toLowerCase()
    if (!name) continue
    identities.push({ name, target: callTarget(part) })
  }
  return identities
}

/** Same tool and same path or command. A different leaked call stays in the turn. */
function leakedDuplicatesStructured(
  call: LeakedCall,
  structured: readonly { name: string; target: string }[]
): boolean {
  const name = call.name.trim().toLowerCase()
  const target = callTarget(call.input)
  return structured.some((tool) => tool.name === name && tool.target === target)
}

function callTarget(source: unknown): string {
  if (!source || typeof source !== 'object' || Array.isArray(source)) return ''
  const record = source as Record<string, unknown>
  const direct = targetField(record)
  if (direct) return direct
  const input = record.input
  if (input && typeof input === 'object' && !Array.isArray(input)) return targetField(input as Record<string, unknown>)
  return ''
}

function targetField(record: Record<string, unknown>): string {
  for (const key of TARGET_KEYS) {
    const value = record[key]
    if (typeof value === 'string' && value.trim()) return value.trim()
  }
  return ''
}

function shownFormSpecs(parts: readonly CherryMessagePart[]): Set<string> {
  const specs = new Set<string>()
  for (const part of parts) {
    if (part.type !== 'data-form') continue
    const spec = (part.data as { spec?: unknown } | undefined)?.spec
    if (typeof spec === 'string' && spec) specs.add(spec)
  }
  return specs
}

function asksShownForm(part: CherryMessagePart, specs: ReadonlySet<string>): boolean {
  if (specs.size === 0 || !('input' in part)) return false
  const input = part.input
  if (typeof input !== 'object' || input === null || !('form' in input)) return false
  return typeof input.form === 'string' && specs.has(input.form)
}

function readStructuredTool(part: CherryMessagePart, index: number): TaskTurnItem | undefined {
  if (!isToolUIPart(part)) return undefined
  const name = getToolName(part).trim()
  if (!name) return undefined
  const id =
    'toolCallId' in part && typeof part.toolCallId === 'string' && part.toolCallId ? part.toolCallId : `tool-${index}`
  return { kind: 'tool', id, name, state: structuredToolState(part), partIndex: index }
}

function structuredToolState(part: CherryMessagePart): TaskToolState {
  const state = 'state' in part ? String(part.state) : ''
  if (state === 'approval-requested') return 'pending'
  if (state === 'output-error' || state === 'output-denied' || state === 'cancelled') return 'error'
  if (state === 'output-available') {
    if ('errorText' in part && part.errorText) return 'error'
    const output = 'output' in part ? part.output : undefined
    if (isErrorOutput(output)) return 'error'
    return 'success'
  }
  return 'running'
}

function isErrorOutput(output: unknown): boolean {
  return (
    typeof output === 'object' &&
    output !== null &&
    !Array.isArray(output) &&
    'isError' in output &&
    output.isError === true
  )
}

function cleanProse(value: string): string {
  let visible = ''
  visitOutsideFences(value, (slice, fenced) => {
    visible += fenced ? slice : scrubDsmlResidue(slice)
  })
  return visible
    .split('\n')
    .map((line) => line.trimEnd())
    .join('\n')
    .trim()
}

/** `<｜DSML｜invoke>`, `<｜｜DSML｜｜ invoke>`, ASCII `||DSML||`, and `<|DSML||invoke>`. */
const DSML_BAR = '(?:\\uFF5C|\\|){1,2}'
const DSML_INVOKE_OPEN = new RegExp(`<${DSML_BAR}DSML${DSML_BAR}\\s*invoke(?=[\\s>/])`, 'u')
const DSML_INVOKE_CLOSE = new RegExp(`</${DSML_BAR}DSML${DSML_BAR}\\s*invoke>`, 'u')
const DSML_PARAMETER = new RegExp(
  `<${DSML_BAR}DSML${DSML_BAR}\\s*parameter\\b([^>]*)>([\\s\\S]*?)</${DSML_BAR}DSML${DSML_BAR}\\s*parameter>`,
  'gu'
)
const DSML_PARAMETER_PARTIAL = new RegExp(`<${DSML_BAR}DSML${DSML_BAR}\\s*parameter\\b([^>]*)>([\\s\\S]*)$`, 'u')
const DSML_WRAPPER = new RegExp(`</?${DSML_BAR}DSML${DSML_BAR}\\s*(?:tool_calls|function_calls|calls)>`, 'gu')
const DSML_PARTIAL_OPEN = new RegExp(`<${DSML_BAR}DSML${DSML_BAR}\\s*[a-z_]*$`, 'u')
const TOOL_NAME = /^[A-Za-z_][A-Za-z0-9_-]*$/u

function findDsmlInvoke(text: string): { index: number; openLength: number } | undefined {
  const match = DSML_INVOKE_OPEN.exec(text)
  if (!match) return undefined
  return { index: match.index, openLength: match[0].length }
}

function takeDsmlInvoke(at: string): { call?: LeakedCall; consumed: number; closed: boolean; hide: boolean } {
  const headerEnd = at.indexOf('>')
  if (headerEnd < 0) return { consumed: at.length, closed: false, hide: true }
  const header = at.slice(0, headerEnd + 1)
  const name = xmlAttr(header, 'name')
  if (!name || !TOOL_NAME.test(name)) return { consumed: headerEnd + 1, closed: false, hide: false }
  const bodyStart = headerEnd + 1
  const close = DSML_INVOKE_CLOSE.exec(at.slice(bodyStart))
  if (!close) {
    return {
      call: { name, input: parseDsmlParameters(at.slice(bodyStart)) },
      consumed: at.length,
      closed: false,
      hide: false
    }
  }
  return {
    call: { name, input: parseDsmlParameters(at.slice(bodyStart, bodyStart + close.index)) },
    consumed: bodyStart + close.index + close[0].length,
    closed: true,
    hide: false
  }
}

function parseDsmlParameters(body: string): Record<string, unknown> {
  const input: Record<string, unknown> = {}
  for (const match of body.matchAll(DSML_PARAMETER)) {
    const name = xmlAttr(match[1] ?? '', 'name')
    if (!name) continue
    input[name] = dsmlParameterValue(match[1] ?? '', match[2] ?? '')
  }
  if (Object.keys(input).length > 0) return input
  const partial = DSML_PARAMETER_PARTIAL.exec(body)
  if (partial) {
    const name = xmlAttr(partial[1] ?? '', 'name')
    if (name) input[name] = dsmlParameterValue(partial[1] ?? '', partial[2] ?? '')
    return input
  }
  const trimmed = body.trim()
  if (!trimmed.startsWith('{')) return input
  try {
    const parsed = JSON.parse(trimmed) as unknown
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) return parsed as Record<string, unknown>
  } catch {
    return input
  }
  return input
}

function dsmlParameterValue(header: string, rawValue: string): unknown {
  const raw = decodeXml(rawValue.trim())
  if ((xmlAttr(header, 'string') ?? '').toLowerCase() === 'true') return raw
  return coerceParameter(raw)
}

function xmlAttr(source: string, key: string): string | undefined {
  const match = new RegExp(`(?:^|\\s)${key}\\s*=\\s*(["'])([\\s\\S]*?)\\1`, 'u').exec(source)
  return match?.[2]
}

function decodeXml(value: string): string {
  return value
    .replace(/&quot;/gu, '"')
    .replace(/&apos;/gu, "'")
    .replace(/&lt;/gu, '<')
    .replace(/&gt;/gu, '>')
    .replace(/&amp;/gu, '&')
}

function coerceParameter(raw: string): unknown {
  if (raw === 'true') return true
  if (raw === 'false') return false
  if (raw === 'null') return null
  if (raw.startsWith('{') || raw.startsWith('[')) {
    try {
      return JSON.parse(raw) as unknown
    } catch {
      return raw
    }
  }
  if (/^-?\d+$/u.test(raw)) return Number(raw)
  if (/^-?\d+\.\d+$/u.test(raw)) return Number(raw)
  return raw
}

function scrubDsmlResidue(value: string): string {
  return value.replace(DSML_WRAPPER, '').replace(DSML_PARTIAL_OPEN, '')
}

function fenceIsOpen(source: string, index: number): boolean {
  let open = false
  let cursor = 0
  while (cursor < index) {
    const next = source.indexOf('```', cursor)
    if (next < 0 || next >= index) break
    open = !open
    cursor = next + 3
  }
  return open
}

function takeCallBody(afterOpen: string): { body: string; consumed: number; closed: boolean } {
  const closed = afterOpen.indexOf(CLOSE_TAG)
  if (closed >= 0) {
    return {
      body: afterOpen.slice(0, closed).trim().replace(/\/$/u, '').trim(),
      consumed: closed + CLOSE_TAG.length,
      closed: true
    }
  }
  const selfClosing = afterOpen.indexOf('/>')
  if (selfClosing >= 0) {
    return { body: afterOpen.slice(0, selfClosing).trim(), consumed: selfClosing + 2, closed: true }
  }
  const next = afterOpen.indexOf(OPEN_TAG)
  if (next >= 0) return { body: afterOpen.slice(0, next).trim(), consumed: next, closed: false }
  return { body: afterOpen.trim(), consumed: afterOpen.length, closed: false }
}

function parseCallBody(body: string): LeakedCall | undefined {
  const trimmed = body.trim().replace(/\/$/u, '').trim()
  const name = trimmed.split(/\s+/u)[0] ?? ''
  if (!/^[A-Za-z_][A-Za-z0-9_-]*$/u.test(name)) return undefined
  return { name, input: parseAttributes(trimmed.slice(name.length).trim()) }
}

function parseAttributes(source: string): Record<string, unknown> {
  const input: Record<string, unknown> = {}
  let index = 0
  while (index < source.length) {
    while (index < source.length && /\s/u.test(source[index] ?? '')) index += 1
    const keyStart = index
    while (index < source.length && /[A-Za-z0-9_-]/u.test(source[index] ?? '')) index += 1
    if (keyStart === index || source[index] !== '=') break
    const key = source.slice(keyStart, index)
    index += 1
    const parsed = parseAttributeValue(source, index)
    input[key] = parsed.value
    index = parsed.next
  }
  return input
}

function parseAttributeValue(source: string, start: number): { value: unknown; next: number } {
  const quote = source[start]
  if (quote === '"' || quote === "'") {
    let index = start + 1
    let value = ''
    while (index < source.length) {
      const ch = source[index] ?? ''
      if (ch === '\\') {
        value += source[index + 1] ?? ''
        index += 2
        continue
      }
      if (ch === quote) return { value, next: index + 1 }
      value += ch
      index += 1
    }
    return { value, next: source.length }
  }
  if (quote === '[' || quote === '{') {
    const end = matchingCloser(source, start)
    if (end !== undefined) {
      const raw = source.slice(start, end + 1)
      try {
        return { value: JSON.parse(raw) as unknown, next: end + 1 }
      } catch {
        return { value: raw, next: end + 1 }
      }
    }
  }
  let index = start
  while (index < source.length && !/\s/u.test(source[index] ?? '')) index += 1
  return { value: bareValue(source.slice(start, index)), next: index }
}

function matchingCloser(source: string, start: number): number | undefined {
  const open = source[start]
  const close = open === '[' ? ']' : '}'
  let depth = 0
  let inString = false
  let escaped = false
  for (let index = start; index < source.length; index += 1) {
    const ch = source[index]
    if (inString) {
      if (escaped) escaped = false
      else if (ch === '\\') escaped = true
      else if (ch === '"') inString = false
      continue
    }
    if (ch === '"') inString = true
    else if (ch === open) depth += 1
    else if (ch === close) {
      depth -= 1
      if (depth === 0) return index
    }
  }
  return undefined
}

function bareValue(raw: string): unknown {
  if (raw === 'true') return true
  if (raw === 'false') return false
  if (raw === 'null') return null
  if (/^-?\d+$/u.test(raw)) return Number(raw)
  if (/^-?\d+\.\d+$/u.test(raw)) return Number(raw)
  return raw
}

function fenceLanguage(info: string): string {
  return info.trim().split(/[\s{]/u)[0]?.toLowerCase() ?? ''
}

function repairMermaidOpener(info: string): string | undefined {
  const trimmed = info.trim()
  if (!/^mermaid/i.test(trimmed)) return undefined
  const rest = trimmed.slice('mermaid'.length).trim()
  if (!rest) return undefined
  return mermaidDirectiveLine(rest)
}

function mermaidDirectiveLine(info: string): string | undefined {
  const trimmed = info.trim()
  if (!trimmed) return undefined
  const lower = trimmed.toLowerCase()
  for (const keyword of MERMAID_DIRECTIVES) {
    const key = keyword.toLowerCase()
    if (lower === key || lower.startsWith(`${key} `) || lower.startsWith(`${key}\t`)) return trimmed
    if (!lower.startsWith(key)) continue
    const rest = trimmed.slice(keyword.length)
    if (!rest || /^[\s{[(-]/u.test(rest)) continue
    if (key === 'flowchart' || key === 'graph') {
      const direction = /^(TD|TB|BT|RL|LR)\b/i.exec(rest)
      if (direction) return `${keyword} ${direction[1].toUpperCase()}${rest.slice(direction[1].length)}`.trim()
    }
    return `${keyword} ${rest}`.trim()
  }
  return undefined
}

function splitMermaidHeader(code: string): string {
  const newline = code.indexOf('\n')
  const first = (newline === -1 ? code : code.slice(0, newline)).trim()
  const rest = newline === -1 ? '' : code.slice(newline + 1)
  const split = splitMermaidHeaderLine(first)
  if (!split) return code
  return rest ? `${split}\n${rest}` : split
}

function splitMermaidHeaderLine(line: string): string | undefined {
  const trimmed = line.trim()
  if (!trimmed) return undefined
  const lower = trimmed.toLowerCase()
  for (const keyword of MERMAID_DIRECTIVES) {
    const key = keyword.toLowerCase()
    if (!lower.startsWith(key)) continue
    const after = trimmed.slice(keyword.length)
    if (!after.trim()) return undefined
    if (key === 'flowchart' || key === 'graph') {
      const direction = /^[ \t]+(TD|TB|BT|RL|LR)\b(.*)$/i.exec(after)
      if (!direction) return `${keyword}\n${after.trim()}`
      const tail = direction[2].trim()
      if (!tail) return undefined
      return `${keyword} ${direction[1].toUpperCase()}\n${tail}`
    }
    return `${keyword}\n${after.trim()}`
  }
  return undefined
}
