import { commandInText, isProseToolLabel } from '@renderer/utils/permissionPrompt'

export type TaskDiffKind = 'context' | 'addition' | 'deletion' | 'hunk'

export interface TaskDiffLine {
  kind: TaskDiffKind
  text: string
  oldLine?: number
  newLine?: number
}

export type TaskDiffOperation = 'write' | 'edit' | 'patch' | 'delete'

export interface TaskCodeDiff {
  path?: string
  operation: TaskDiffOperation
  lines: TaskDiffLine[]
  additions: number
  deletions: number
  comparisonComplete: boolean
  truncated: boolean
}

export interface TaskToolCopy {
  titleKey?: string
  titleValues?: Record<string, string>
  titleText?: string
  summaryKey?: string
  summaryText?: string
  statusKey?: string
}

export interface WikiSearchHit {
  name: string
  path: string
  passage: string
}

export type WikiSearchView = { phase: 'preparing' } | { phase: 'empty' } | { phase: 'hits'; hits: WikiSearchHit[] }

export interface TaskToolPayload {
  input?: string
  output?: string
  error?: string
}

const MAX_DIFF_LINES = 400
const PATH_KEYS = ['file_path', 'filePath', 'path', 'filename', 'file']
const DIFF_KEYS = [
  'unified_diff',
  'unifiedDiff',
  'unified',
  'diff_text',
  'diffText',
  'patch_text',
  'patchText',
  'patch',
  'diff'
]
const BEFORE_KEYS = [
  'before',
  'old_content',
  'oldContent',
  'old_text',
  'oldText',
  'old_string',
  'oldString',
  'previous'
]
const AFTER_KEYS = [
  'after',
  'new_content',
  'newContent',
  'new_text',
  'newText',
  'new_string',
  'newString',
  'replacement'
]
const WRITE_BODY_KEYS = ['content', 'contents', 'file_text', 'fileText']

/** File change carried by a tool part, or undefined when the call is not an edit. */
export function extractTaskCodeDiff(toolName: string, part: unknown): TaskCodeDiff | undefined {
  const name = toolName.trim().toLowerCase()
  const bags = collectBags(part)
  const path = firstPath(bags)
  const unified = firstUnified(bags)
  if (unified) {
    const parsed = parseUnified(unified, path)
    if (parsed && (parsed.lines.length > 0 || parsed.path || path)) {
      return finish(name, parsed.path ?? path, parsed.lines, true, parsed.truncated)
    }
  }
  const pair = firstPair(bags)
  if (pair) {
    const diffed = diffLines(pair.before, pair.after)
    return finish(name, pair.path ?? path, diffed.lines, true, diffed.truncated)
  }
  const body = writeBody(name, bags)
  if (body !== undefined) {
    const source = splitKeep(body)
    const lines = source.slice(0, MAX_DIFF_LINES).map((text, index) => ({
      kind: 'addition' as const,
      text,
      newLine: index + 1
    }))
    return finish(name, path, lines, false, source.length > lines.length)
  }
  const declared = declaredDiffPath(bags)
  if (declared || (isMutation(name) && path)) {
    return finish(name, declared ?? path, [], false, false)
  }
  return undefined
}

/** Match a tool-name token. `search_skills` must not count as `ls`. */
function hasToolToken(name: string, needles: readonly string[]): boolean {
  const tokens = name
    .toLowerCase()
    .split(/[^a-z0-9]+/u)
    .filter(Boolean)
  return needles.some((needle) => tokens.includes(needle))
}

/** Verb and target for the collapsed tool chip. Raw name when nothing stronger exists. */
export function taskToolCopy(toolName: string, part: unknown, diff?: TaskCodeDiff): TaskToolCopy {
  const name = toolName.trim()
  const file = diff?.path ? baseName(diff.path) : citedPath(part)
  if (diff) {
    const summary = diffSummary(diff)
    if (!file) return { titleKey: 'workbench.diff.change', ...summary }
    if (diff.operation === 'write') return { titleKey: 'workbench.tool.write', titleValues: { name: file }, ...summary }
    if (diff.operation === 'delete')
      return { titleKey: 'workbench.tool.delete', titleValues: { name: file }, ...summary }
    if (diff.operation === 'patch') return { titleKey: 'workbench.tool.patch', titleValues: { name: file }, ...summary }
    return { titleKey: 'workbench.tool.edit', titleValues: { name: file }, ...summary }
  }

  const lowered = name.toLowerCase()
  if (lowered === 'search_wiki') return wikiToolCopy(part)
  if (lowered === 'ask_user_form') return { titleKey: 'workbench.tool.ask_form', ...settledPreview(part) }
  if (/shell|bash|zsh|exec|run_command|run_terminal/.test(lowered)) {
    const command = citedString(part, ['command', 'cmd'])
    const preview = settledPreview(part)
    if (command) return { titleKey: 'workbench.tool.run', titleValues: { command: compact(command, 48) }, ...preview }
    return { titleKey: 'workbench.tool.run_command', ...preview }
  }
  if (/read|view_file|open_file|cat/.test(lowered) && file) {
    return { titleKey: 'workbench.tool.read', titleValues: { name: file }, ...settledPreview(part) }
  }
  if (hasToolToken(lowered, ['list', 'ls', 'dir', 'glob'])) {
    if (file) return { titleKey: 'workbench.tool.list', titleValues: { name: file }, ...settledPreview(part) }
    return { titleKey: 'workbench.tool.list_files', ...settledPreview(part) }
  }
  if (/grep|search|find/.test(lowered)) {
    const query = citedString(part, ['query', 'pattern', 'q'])
    if (query)
      return { titleKey: 'workbench.tool.search', titleValues: { query: compact(query, 48) }, ...settledPreview(part) }
    return { titleKey: 'workbench.tool.search_plain', ...settledPreview(part) }
  }
  if (/delete|remove|unlink/.test(lowered) && file) {
    return { titleKey: 'workbench.tool.delete', titleValues: { name: baseName(file) } }
  }
  if (isProseToolLabel(name)) {
    const command = citedString(part, ['command', 'cmd']) ?? commandInText(name)
    if (command) return { titleKey: 'workbench.tool.run', titleValues: { command: compact(command, 48) } }
    return { titleKey: 'workbench.tool.confirm' }
  }
  return { titleText: name, ...settledPreview(part) }
}

/** Bounded input, output, and error text for the expanded tool disclosure. */
export function taskToolPayload(part: unknown): TaskToolPayload {
  if (!part || typeof part !== 'object') return {}
  const record = part as Record<string, unknown>
  const error = typeof record.errorText === 'string' ? record.errorText.trim() : ''
  return {
    input: meaningful(record.input),
    output: meaningful(record.output),
    error: error || undefined
  }
}

export interface DiffToken {
  text: string
  changed: boolean
}

const DIFF_TOKEN = /\s+|\p{L}+|\p{N}+|[^\s\p{L}\p{N}]/gu
const DIFF_TOKEN_LIMIT = 120
const DIFF_CHAR_LIMIT = 800

/** Changed spans inside one replaced line. A fully rewritten line stays unmarked. */
export function intraLineSpans(before: string, after: string): { before: DiffToken[]; after: DiffToken[] } | null {
  if (before === after || before.length > DIFF_CHAR_LIMIT || after.length > DIFF_CHAR_LIMIT) return null
  const left = before.match(DIFF_TOKEN) ?? []
  const right = after.match(DIFF_TOKEN) ?? []
  if (left.length === 0 || right.length === 0 || left.length > DIFF_TOKEN_LIMIT || right.length > DIFF_TOKEN_LIMIT) {
    return null
  }
  const shared = tokenLcs(left, right)
  if (shared.length === 0) return null
  const beforeSpans = mergeTokens(left.map((text, index) => ({ text, changed: !shared.left.has(index) })))
  const afterSpans = mergeTokens(right.map((text, index) => ({ text, changed: !shared.right.has(index) })))
  if (!beforeSpans.some((span) => span.changed) || !afterSpans.some((span) => !span.changed && span.text.trim())) {
    return null
  }
  if (!afterSpans.some((span) => span.changed)) return null
  return { before: beforeSpans, after: afterSpans }
}

/** Intra-line spans aligned to `lines`. Unpaired or fully rewritten lines are empty. */
export function pairLineSpans(lines: readonly TaskDiffLine[]): DiffToken[][] {
  const spans = lines.map(() => [] as DiffToken[])
  let index = 0
  while (index < lines.length) {
    if (lines[index]?.kind !== 'deletion') {
      index += 1
      continue
    }
    const deletions: number[] = []
    while (lines[index]?.kind === 'deletion') {
      deletions.push(index)
      index += 1
    }
    const additions: number[] = []
    while (lines[index]?.kind === 'addition') {
      additions.push(index)
      index += 1
    }
    const count = Math.min(deletions.length, additions.length)
    for (let offset = 0; offset < count; offset += 1) {
      const pair = intraLineSpans(lines[deletions[offset]]?.text ?? '', lines[additions[offset]]?.text ?? '')
      if (!pair) continue
      spans[deletions[offset]] = pair.before
      spans[additions[offset]] = pair.after
    }
  }
  return spans
}

function tokenLcs(left: string[], right: string[]): { left: Set<number>; right: Set<number> } {
  const rows = left.length + 1
  const cols = right.length + 1
  const scores = Array.from({ length: rows }, () => new Uint16Array(cols))
  for (let i = left.length - 1; i >= 0; i -= 1) {
    for (let j = right.length - 1; j >= 0; j -= 1) {
      scores[i][j] = left[i] === right[j] ? scores[i + 1][j + 1] + 1 : Math.max(scores[i + 1][j], scores[i][j + 1])
    }
  }
  const matchedLeft = new Set<number>()
  const matchedRight = new Set<number>()
  let i = 0
  let j = 0
  while (i < left.length && j < right.length) {
    if (left[i] === right[j]) {
      matchedLeft.add(i)
      matchedRight.add(j)
      i += 1
      j += 1
    } else if (scores[i + 1][j] >= scores[i][j + 1]) {
      i += 1
    } else {
      j += 1
    }
  }
  return { left: matchedLeft, right: matchedRight }
}

function mergeTokens(spans: DiffToken[]): DiffToken[] {
  const merged: DiffToken[] = []
  for (const span of spans) {
    const last = merged[merged.length - 1]
    if (last && last.changed === span.changed) last.text += span.text
    else merged.push({ text: span.text, changed: span.changed })
  }
  return merged
}

export function taskDiffPatchText(diff: TaskCodeDiff): string {
  const header = diff.path ? `--- a/${diff.path}\n+++ b/${diff.path}\n` : ''
  const body = diff.lines
    .map((line) => {
      if (line.kind === 'hunk') return line.text.startsWith('@@') ? line.text : `@@ ${line.text}`
      const mark = line.kind === 'addition' ? '+' : line.kind === 'deletion' ? '-' : ' '
      return `${mark}${line.text}`
    })
    .join('\n')
  return `${header}${body}`
}

export function baseName(path: string): string {
  const normalized = path.replace(/\\/g, '/').replace(/\/+$/u, '')
  const name = normalized.slice(normalized.lastIndexOf('/') + 1)
  return name || normalized
}

function finish(
  toolName: string,
  path: string | undefined,
  lines: TaskDiffLine[],
  comparisonComplete: boolean,
  truncated: boolean
): TaskCodeDiff {
  let additions = 0
  let deletions = 0
  for (const line of lines) {
    if (line.kind === 'addition') additions += 1
    if (line.kind === 'deletion') deletions += 1
  }
  return {
    path,
    operation: operationOf(toolName),
    lines,
    additions,
    deletions,
    comparisonComplete: comparisonComplete && lines.length > 0,
    truncated
  }
}

function diffSummary(diff: TaskCodeDiff): Pick<TaskToolCopy, 'summaryKey' | 'summaryText'> {
  if (diff.lines.length === 0) return { summaryKey: 'workbench.tool.diff_pending' }
  const stats = `+${diff.additions} · −${diff.deletions}`
  if (!diff.comparisonComplete) return { summaryKey: 'workbench.tool.preview', summaryText: stats }
  return { summaryText: stats }
}

/** Published wiki search text. Phase tokens stay out of the chip preview. */
export function wikiSearchView(toolName: string, part: unknown): WikiSearchView | null {
  if (toolName.trim().toLowerCase() !== 'search_wiki') return null
  const text = taskToolResultText(part)
  if (!text) return null
  if (text === 'not-ready') return { phase: 'preparing' }
  if (text === 'no-match') return { phase: 'empty' }
  const hits = parseWikiHits(text)
  if (!hits) {
    const name =
      text
        .split('\n')
        .find((line) => line.trim())
        ?.trim() || text
    return { phase: 'hits', hits: [{ name, path: '', passage: text }] }
  }
  return { phase: 'hits', hits }
}

function wikiToolCopy(part: unknown): TaskToolCopy {
  const query = citedString(part, ['query'])
  const title: TaskToolCopy = query
    ? { titleKey: 'workbench.tool.wiki_search', titleValues: { query: compact(query, 48) } }
    : { titleKey: 'workbench.tool.wiki_search_plain' }
  const view = wikiSearchView('search_wiki', part)
  if (!view) return title
  if (view.phase === 'preparing') {
    return {
      ...title,
      summaryKey: 'workbench.tool.wiki_preparing',
      statusKey: 'workbench.tool.wiki_preparing_status'
    }
  }
  if (view.phase === 'empty') return { ...title, summaryKey: 'workbench.tool.wiki_empty' }
  const names = view.hits.map((hit) => hit.name).filter(Boolean)
  return names.length > 0 ? { ...title, summaryText: compact(names.join('、'), 72) } : title
}

function parseWikiHits(text: string): WikiSearchHit[] | null {
  const blocks = text.split(/\n\n/)
  const hits: WikiSearchHit[] = []
  for (const block of blocks) {
    const lines = block.split('\n')
    const name = lines[0]?.trim() ?? ''
    const path = lines[1]?.trim() ?? ''
    if (!name || !looksLikeWikiPath(path)) return null
    hits.push({ name, path, passage: lines.slice(2).join('\n').trim() })
  }
  return hits.length > 0 ? hits : null
}

function looksLikeWikiPath(path: string): boolean {
  return path.length > 0 && !/\s/u.test(path) && (path.includes('/') || path.endsWith('.md'))
}

function settledPreview(part: unknown): Pick<TaskToolCopy, 'summaryText'> {
  const text = taskToolResultText(part)
  if (!text) return {}
  const first =
    text
      .split(/\r?\n/u)
      .find((line) => line.trim())
      ?.trim() ?? ''
  if (!first || first.startsWith('{') || first.startsWith('[')) return {}
  return { summaryText: compact(first, 72) }
}

export interface DirectoryListingEntry {
  kind: 'dir' | 'file' | 'link' | 'unknown'
  name: string
  size: string
}

export interface DirectoryListing {
  path: string
  entries: DirectoryListingEntry[]
  empty: boolean
  footer?: string
  more?: string
}

const DIRECTORY_ENTRY = /^(dir|file|link|unknown)\s+(\S+)\s{2}(.+)$/u

/** `ls` text from the workspace tool, or undefined when the text is not that listing. */
export function parseDirectoryListing(text: string): DirectoryListing | undefined {
  const lines = text.replace(/\r\n/gu, '\n').replace(/\n+$/u, '').split('\n')
  const header = lines[0]?.match(/^Directory:\s+(\S.*)$/u)
  if (!header || lines[1] !== '') return undefined
  const path = header[1].trim()
  if (lines.length === 3 && lines[2] === '(empty directory)') {
    return { path, entries: [], empty: true }
  }
  const entries: DirectoryListingEntry[] = []
  let index = 2
  for (; index < lines.length; index += 1) {
    const line = lines[index] ?? ''
    if (line === '') break
    const match = DIRECTORY_ENTRY.exec(line)
    if (!match) return undefined
    const kind = match[1] as DirectoryListingEntry['kind']
    let name = match[3] ?? ''
    if (kind === 'dir' && name.endsWith('/')) name = name.slice(0, -1)
    if (!name) return undefined
    entries.push({ kind, name, size: match[2] ?? '' })
  }
  if (entries.length === 0 || lines[index] !== '') return undefined
  index += 1
  const footer = lines[index]
  if (!footer || !/^\d+ of \d+ entries$/u.test(footer)) return undefined
  index += 1
  let more: string | undefined
  if (index < lines.length) {
    more = lines[index]
    if (!more || !/^More entries available; continue with cursor=\S+$/u.test(more)) return undefined
    index += 1
  }
  if (index !== lines.length) return undefined
  return { path, entries, empty: false, footer, more }
}

/** Plain command or tool text. Structured JSON stays in the raw payload. */
export function taskToolResultText(part: unknown): string | undefined {
  if (!part || typeof part !== 'object') return undefined
  const record = part as Record<string, unknown>
  const text = readResultText(record.output) ?? readResultText(record.content)
  if (!text) return undefined
  return text.length > 8000 ? `${text.slice(0, 8000)}\n…` : text
}

function readResultText(value: unknown): string | undefined {
  if (typeof value === 'string') {
    const trimmed = value.trim()
    if (!trimmed || trimmed === '{}' || trimmed === '[]' || trimmed === 'null') return undefined
    if (looksUnified(trimmed)) return undefined
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
      try {
        return readResultText(JSON.parse(trimmed) as unknown)
      } catch {
        return undefined
      }
    }
    return trimmed
  }
  if (Array.isArray(value)) {
    const bytes = byteText(value)
    if (bytes) return bytes
    const parts = value.map((item) => readResultText(item)).filter((item): item is string => Boolean(item))
    return parts.length > 0 ? parts.join('\n') : undefined
  }
  if (!value || typeof value !== 'object') return undefined
  const record = value as Record<string, unknown>
  if (record.type === 'diff') return undefined
  if (typeof record.stdout === 'string' && record.stdout.trim()) return record.stdout.trim()
  if (record.type === 'text' && typeof record.text === 'string' && record.text.trim()) return record.text.trim()
  if (Array.isArray(record.content)) return readResultText(record.content)
  if (record.output != null && record.output !== value) {
    const nested = readResultText(record.output)
    if (nested) return nested
  }
  if (typeof record.stderr === 'string' && record.stderr.trim()) return record.stderr.trim()
  return undefined
}

/** Command tools sometimes return stdout as a list of bytes. */
function byteText(value: readonly unknown[]): string | undefined {
  if (value.length === 0) return undefined
  const bytes: number[] = []
  for (const item of value) {
    if (typeof item !== 'number' || !Number.isInteger(item) || item < 0 || item > 255) return undefined
    bytes.push(item)
  }
  const text = new TextDecoder('utf-8', { fatal: false })
    .decode(Uint8Array.from(bytes))
    .replace(/\u0000+$/u, '')
    .trim()
  return text || undefined
}

function operationOf(name: string): TaskDiffOperation {
  if (/delete|remove|unlink/.test(name)) return 'delete'
  if (/patch|apply/.test(name)) return 'patch'
  if (/write|create/.test(name) && !/replace|edit|update/.test(name)) return 'write'
  return 'edit'
}

export function isMutation(name: string): boolean {
  if (/search_replace|str_replace|strreplace|apply_patch|edit_file|write_file/.test(name)) return true
  if (/^(read|grep|glob|ls|list|bash|shell|exec)/.test(name)) return false
  return /(^|_)(write|edit|patch|replace|create|update|delete|remove)($|_)/.test(name)
}

function isWrite(name: string): boolean {
  return /write|create/.test(name) && !/replace|edit|update|patch|delete|remove/.test(name)
}

function collectBags(part: unknown): unknown[] {
  if (!part || typeof part !== 'object') return []
  const record = part as Record<string, unknown>
  const bags: unknown[] = []
  if (Array.isArray(record.files)) bags.push(...record.files)
  for (const value of [record.input, record.output, record.content]) {
    if (value == null) continue
    bags.push(value)
    if (Array.isArray(value)) {
      bags.push(...value)
      continue
    }
    if (typeof value === 'object') {
      const nested = value as Record<string, unknown>
      if (Array.isArray(nested.content)) bags.push(...nested.content)
    }
  }
  return bags
}

function firstPath(bags: readonly unknown[]): string | undefined {
  for (const bag of bags) {
    if (typeof bag === 'string' && looksLikePath(bag)) return bag.trim()
    const path = stringField(bag, PATH_KEYS)
    if (path) return path
  }
  return undefined
}

function declaredDiffPath(bags: readonly unknown[]): string | undefined {
  for (const bag of bags) {
    if (!bag || typeof bag !== 'object' || Array.isArray(bag)) continue
    const record = bag as Record<string, unknown>
    if (record.type !== 'diff') continue
    const path = stringField(record, PATH_KEYS)
    if (path) return path
  }
  return undefined
}

function firstUnified(bags: readonly unknown[]): string | undefined {
  for (const bag of bags) {
    if (typeof bag === 'string' && looksUnified(bag)) return bag
    if (!bag || typeof bag !== 'object' || Array.isArray(bag)) continue
    const record = bag as Record<string, unknown>
    const text = stringField(record, DIFF_KEYS)
    if (text && looksUnified(text)) return text
  }
  return undefined
}

function firstPair(bags: readonly unknown[]): { before: string; after: string; path?: string } | undefined {
  for (const bag of bags) {
    if (!bag || typeof bag !== 'object' || Array.isArray(bag)) continue
    const record = bag as Record<string, unknown>
    const before = stringField(record, BEFORE_KEYS)
    const after = stringField(record, AFTER_KEYS)
    if (before === undefined || after === undefined) continue
    return { before, after, path: stringField(record, PATH_KEYS) }
  }
  return undefined
}

function writeBody(name: string, bags: readonly unknown[]): string | undefined {
  if (!isWrite(name)) return undefined
  for (const bag of bags) {
    if (!bag || typeof bag !== 'object' || Array.isArray(bag)) continue
    const text = stringField(bag, WRITE_BODY_KEYS)
    if (text !== undefined && !looksUnified(text)) return text
  }
  return undefined
}

function citedPath(part: unknown): string | undefined {
  return firstPath(collectBags(part))
}

function citedString(part: unknown, keys: readonly string[]): string | undefined {
  for (const bag of collectBags(part)) {
    const value = stringField(bag, keys)
    if (value) return value
  }
  return undefined
}

function stringField(value: unknown, keys: readonly string[]): string | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return undefined
  const record = value as Record<string, unknown>
  for (const key of keys) {
    const field = record[key]
    if (typeof field === 'string' && field.trim()) return field
  }
  return undefined
}

function looksLikePath(value: string): boolean {
  return /[\\/]/.test(value) && !value.includes('\n') && value.length < 512
}

function looksUnified(text: string): boolean {
  return (
    text.includes('diff --git ') || text.includes('\n@@') || text.startsWith('@@') || text.includes('*** Begin Patch')
  )
}

function parseUnified(
  text: string,
  fallbackPath?: string
): { path?: string; lines: TaskDiffLine[]; truncated: boolean } | undefined {
  const rows = text.replace(/\n$/u, '').split('\n')
  const lines: TaskDiffLine[] = []
  let path = fallbackPath
  let oldLine = 0
  let newLine = 0
  let inHunk = false
  let truncated = false
  for (const row of rows) {
    if (lines.length >= MAX_DIFF_LINES) {
      truncated = true
      break
    }
    if (row.startsWith('diff --git ')) {
      path = gitPath(row) ?? path
      inHunk = false
      continue
    }
    if (row.startsWith('*** Update File:') || row.startsWith('*** Add File:') || row.startsWith('*** Delete File:')) {
      path = row.split(':').slice(1).join(':').trim() || path
      inHunk = false
      continue
    }
    if (row.startsWith('+++ ') || row.startsWith('--- ') || row.startsWith('*** ')) continue
    const hunk = /^@@\s+-(\d+)(?:,\d+)?\s+\+(\d+)/.exec(row)
    if (hunk) {
      oldLine = Number(hunk[1])
      newLine = Number(hunk[2])
      inHunk = true
      lines.push({ kind: 'hunk', text: row })
      continue
    }
    if (!inHunk) continue
    if (row.startsWith('+')) {
      lines.push({ kind: 'addition', text: row.slice(1), newLine })
      newLine += 1
      continue
    }
    if (row.startsWith('-')) {
      lines.push({ kind: 'deletion', text: row.slice(1), oldLine })
      oldLine += 1
      continue
    }
    if (row.startsWith('\\')) continue
    const textLine = row.startsWith(' ') ? row.slice(1) : row
    lines.push({ kind: 'context', text: textLine, oldLine, newLine })
    oldLine += 1
    newLine += 1
  }
  if (!lines.some((line) => line.kind === 'addition' || line.kind === 'deletion' || line.kind === 'context')) {
    return undefined
  }
  return { path, lines, truncated }
}

function gitPath(row: string): string | undefined {
  const match = /^diff --git a\/(.+) b\/(.+)$/.exec(row)
  return match?.[2] ?? match?.[1]
}

function diffLines(before: string, after: string): { lines: TaskDiffLine[]; truncated: boolean } {
  const left = splitKeep(before)
  const right = splitKeep(after)
  if (left.length > 140 || right.length > 140) {
    return {
      truncated: true,
      lines: [
        ...left.slice(0, 80).map((text, index) => ({ kind: 'deletion' as const, text, oldLine: index + 1 })),
        ...right.slice(0, 80).map((text, index) => ({ kind: 'addition' as const, text, newLine: index + 1 }))
      ]
    }
  }
  const width = right.length + 1
  const scores = new Uint32Array((left.length + 1) * width)
  for (let i = left.length - 1; i >= 0; i -= 1) {
    for (let j = right.length - 1; j >= 0; j -= 1) {
      const at = i * width + j
      scores[at] = left[i] === right[j] ? scores[at + width + 1] + 1 : Math.max(scores[at + width], scores[at + 1])
    }
  }
  const lines: TaskDiffLine[] = []
  let i = 0
  let j = 0
  let oldLine = 1
  let newLine = 1
  while (i < left.length && j < right.length) {
    if (left[i] === right[j]) {
      lines.push({ kind: 'context', text: left[i] ?? '', oldLine, newLine })
      i += 1
      j += 1
      oldLine += 1
      newLine += 1
      continue
    }
    if (scores[(i + 1) * width + j] >= scores[i * width + j + 1]) {
      lines.push({ kind: 'deletion', text: left[i] ?? '', oldLine })
      i += 1
      oldLine += 1
      continue
    }
    lines.push({ kind: 'addition', text: right[j] ?? '', newLine })
    j += 1
    newLine += 1
  }
  while (i < left.length) {
    lines.push({ kind: 'deletion', text: left[i] ?? '', oldLine })
    i += 1
    oldLine += 1
  }
  while (j < right.length) {
    lines.push({ kind: 'addition', text: right[j] ?? '', newLine })
    j += 1
    newLine += 1
  }
  if (lines.length <= MAX_DIFF_LINES) return { lines, truncated: false }
  return { lines: lines.slice(0, MAX_DIFF_LINES), truncated: true }
}

function splitKeep(value: string): string[] {
  if (!value) return []
  const normalized = value.endsWith('\n') ? value.slice(0, -1) : value
  return normalized.split('\n')
}

function meaningful(value: unknown): string | undefined {
  if (value == null) return undefined
  const text = typeof value === 'string' ? value : JSON.stringify(value, null, 2)
  if (!text) return undefined
  const trimmed = text.trim()
  if (!trimmed || trimmed === '{}' || trimmed === '[]' || trimmed === 'null') return undefined
  return trimmed.length > 8000 ? `${trimmed.slice(0, 8000)}\n…` : trimmed
}

function compact(value: string, max: number): string {
  const text = value.replace(/\s+/gu, ' ').trim()
  if (text.length <= max) return text
  return `${text.slice(0, Math.max(1, max - 1)).trimEnd()}…`
}
