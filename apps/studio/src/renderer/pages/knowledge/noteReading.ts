import { noteProperties, noteTags, openTodoLines } from './flowixNotes'
import { backlinkNotes, findNote, noteLinks, noteTitle } from './knowledgeLinks'

/** One indexed note stays within the visual markdown budget. */
export const NOTE_INDEX_BYTES = 256 * 1024

/** The page states this cap when the vault lists more files than it reads. */
export const NOTE_INDEX_LIMIT = 200

export interface ReadingNote {
  name: string
  path: string
  markdown: string | null
}

export interface ReadingTarget {
  label: string
  path: string
}

export interface UnresolvedTarget {
  label: string
  target: string
}

export interface BacklinkTarget extends ReadingTarget {
  line: string
}

export interface NoteHeading {
  level: number
  text: string
}

export interface BodyPassage {
  path: string
  name: string
  line: string
}

export interface NoteReading {
  outgoing: ReadingTarget[]
  unresolved: UnresolvedTarget[]
  backlinks: BacklinkTarget[]
  tags: string[]
  properties: Array<{ key: string; value: string }>
  todos: string[]
}

/** Links, tags, properties, and open to-dos for one saved note in this vault. */
export function noteReading(notes: readonly ReadingNote[], openedName: string, markdown: string): NoteReading {
  const outgoing: ReadingTarget[] = []
  const unresolved: UnresolvedTarget[] = []
  const seenPath = new Set<string>()
  const seenMissing = new Set<string>()
  for (const link of noteLinks(markdown)) {
    const found = findNote(notes, link.target)
    if (!found) {
      const label = noteTitle(link.target)
      const target = link.target.trim()
      if (!label || !target || seenMissing.has(label)) continue
      seenMissing.add(label)
      unresolved.push({ label, target })
      continue
    }
    if (seenPath.has(found.path)) continue
    seenPath.add(found.path)
    outgoing.push({ label: link.label, path: found.path })
  }
  return {
    outgoing,
    unresolved,
    backlinks: backlinkNotes(notes, openedName).map((note) => ({
      label: noteTitle(note.name),
      path: note.path,
      line: linkLine(note.markdown ?? '', openedName)
    })),
    tags: noteTags(markdown),
    properties: noteProperties(markdown).filter((item) => item.key !== 'title' && item.value !== ''),
    todos: openTodoLines(markdown)
  }
}

/** The source line that links to `targetName`, or an empty string. */
export function linkLine(markdown: string, targetName: string): string {
  const key = noteTitle(targetName)
  if (!key) return ''
  for (const line of markdown.split(/\r?\n/)) {
    if (noteLinks(line).some((link) => noteTitle(link.target) === key)) return line.trim()
  }
  return ''
}

/** ATX headings in reading order. A fenced code block does not contribute headings. */
export function noteHeadings(markdown: string): NoteHeading[] {
  const headings: NoteHeading[] = []
  let fence: string | null = null
  for (const line of markdown.split(/\r?\n/)) {
    const marker = /^(```+|~~~+)/.exec(line.trim())
    if (marker) {
      fence = fence === marker[1][0] ? null : (fence ?? marker[1][0])
      continue
    }
    if (fence) continue
    const match = /^(#{1,6})[ \t]+(\S.*)$/.exec(line)
    if (!match?.[1] || !match[2]) continue
    headings.push({ level: match[1].length, text: match[2].trim() })
  }
  return headings
}

const PASSAGE_LIMIT = 80

/** One saved line that contains the query, including notes whose file name also matches. */
export function bodySearchPassages(notes: readonly ReadingNote[], needle: string): BodyPassage[] {
  const query = needle.trim().toLowerCase()
  if (!query) return []
  const hits: BodyPassage[] = []
  for (const note of notes) {
    if (note.markdown == null) continue
    const line = note.markdown.split(/\r?\n/).find((item) => item.toLowerCase().includes(query))
    if (line == null) continue
    hits.push({ path: note.path, name: note.name, line: clipPassage(line.trim(), query) })
  }
  return hits
}

function clipPassage(line: string, query: string): string {
  if (line.length <= PASSAGE_LIMIT) return line
  const at = line.toLowerCase().indexOf(query)
  const start = Math.max(0, (at < 0 ? 0 : at) - 20)
  return line.slice(start, start + PASSAGE_LIMIT)
}

/** Notes whose saved text contains the query and whose file name does not. */
export function bodySearchHits(notes: readonly ReadingNote[], needle: string): ReadingNote[] {
  const query = needle.trim().toLowerCase()
  if (!query) return []
  return notes.filter((note) => {
    if (note.markdown == null) return false
    const name = note.name.toLowerCase()
    if (name.includes(query) || noteTitle(note.name).toLowerCase().includes(query)) return false
    return note.markdown.toLowerCase().includes(query)
  })
}

export function indexCoverage(notes: readonly ReadingNote[]): { read: number; listed: number } {
  return {
    listed: notes.length,
    read: notes.filter((note) => note.markdown != null).length
  }
}

function yamlUnquote(value: string): string {
  const trimmed = value.trim()
  if (trimmed.length >= 2 && trimmed.startsWith('"') && trimmed.endsWith('"')) {
    return trimmed.slice(1, -1).replace(/\\"/g, '"').replace(/\\\\/g, '\\')
  }
  if (trimmed.length >= 2 && trimmed.startsWith("'") && trimmed.endsWith("'")) {
    return trimmed.slice(1, -1).replace(/''/g, "'")
  }
  return trimmed
}

/** Frontmatter `title` when it is non-empty. Otherwise the file name without `.md`. */
export function noteDisplayName(name: string, markdown?: string | null): string {
  if (markdown) {
    const block = /^---\r?\n([\s\S]*?)\r?\n---/.exec(markdown)
    if (block) {
      for (const line of block[1].split(/\r?\n/)) {
        const match = /^title:\s*(.*)$/.exec(line)
        if (!match) continue
        const value = yamlUnquote(match[1]).trim()
        if (value) return value
      }
    }
  }
  return noteTitle(name)
}
