import { listLocalEntries, localEntryStatus, readLocalDocumentBytes } from '@renderer/pages/documents/useLocalDocuments'

import { typedMarkdown } from './knowledgeLinks'
import { personalVaultPath } from './knowledgeVault'
import { createPersonalNote, personalNoteStem } from './personalNote'

const SKIP_DIRS = new Set(['wiki', '.git', 'node_modules'])
const MAX_NOTES = 200
const MAX_DEPTH = 6
const MAX_BYTES = 256 * 1024

export interface NoteProperty {
  key: string
  value: string
}

export interface FlowixNote {
  path: string
  name: string
  title: string
  markdown: string
  tags: string[]
  properties: NoteProperty[]
  openTodo: boolean
  modifiedMs: number
}

/** First readable line under the front matter, for the note card. */
export function noteExcerpt(markdown: string): string {
  const body = markdown.replace(/^---\n[\s\S]*?\n---\n?/u, '')
  const line = body
    .split('\n')
    .map((item) => item.trim())
    .find((item) => item.length > 0 && !item.startsWith('#'))
  return (line ?? '')
    .replace(/#[\p{L}\p{N}_-]+/gu, '')
    .replace(/[*_`>#-]/gu, '')
    .replace(/\s+/gu, ' ')
    .trim()
    .slice(0, 96)
}

/** Body tags such as `#idea`. Headings stay headings because they have a space after `#`. */
export function noteTags(markdown: string): string[] {
  const tags = new Set<string>()
  for (const match of markdown.matchAll(/(?:^|[\s([{])#([\p{L}\p{N}_-]{1,40})/gu)) {
    const tag = match[1]
    if (tag) tags.add(tag)
  }
  return [...tags].sort((left, right) => left.localeCompare(right))
}

/** YAML front matter keys. Only flat `key: value` lines are properties. */
export function noteProperties(markdown: string): NoteProperty[] {
  if (!markdown.startsWith('---\n')) return []
  const end = markdown.indexOf('\n---', 4)
  if (end < 0) return []
  const properties: NoteProperty[] = []
  for (const line of markdown.slice(4, end).split('\n')) {
    const match = /^([A-Za-z_][\w-]*)\s*:\s*(.*)$/u.exec(line.trim())
    if (!match?.[1]) continue
    properties.push({ key: match[1], value: match[2]?.replace(/^["']|["']$/gu, '') ?? '' })
  }
  return properties
}

export function noteHasOpenTodo(markdown: string): boolean {
  return /(?:^|\n)\s*[-*] \[ \]/u.test(typedMarkdown(markdown))
}

/** Text after each open `- [ ]` or `* [ ]` marker. Checked items stay out. */
export function openTodoLines(markdown: string): string[] {
  const lines: string[] = []
  for (const line of typedMarkdown(markdown).split('\n')) {
    const match = /^\s*[-*] \[ \](?:\s+(.*))?$/u.exec(line)
    if (!match) continue
    const text = match[1]?.trim() ?? ''
    if (text) lines.push(text)
  }
  return lines
}

export function noteTitle(name: string, properties: readonly NoteProperty[]): string {
  const titled = properties.find((item) => item.key === 'title')?.value.trim()
  return titled || name.replace(/\.md$/iu, '')
}

async function readText(root: string, path: string): Promise<string | null> {
  try {
    const bytes = await readLocalDocumentBytes(root, path, undefined, MAX_BYTES)
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes)
  } catch {
    return null
  }
}

/** Personal notes live only in the workspace `wiki/local` folder. */
export function personalNotesDirectory(workspaceRoot: string): string {
  return personalVaultPath(workspaceRoot)
}

/** Markdown notes under `wiki/local`. */
export async function loadFlowixNotes(workspaceRoot: string): Promise<FlowixNote[]> {
  const notes: FlowixNote[] = []
  const visit = async (directory: string, depth: number) => {
    if (notes.length >= MAX_NOTES || depth > MAX_DEPTH) return
    let listing: Awaited<ReturnType<typeof listLocalEntries>>
    try {
      listing = await listLocalEntries(workspaceRoot, directory)
    } catch (error) {
      if (localEntryStatus(error) === 404) return
      throw error
    }
    for (const entry of listing.entries) {
      if (notes.length >= MAX_NOTES) return
      if (entry.kind === 'directory') {
        if (!SKIP_DIRS.has(entry.name)) await visit(entry.path, depth + 1)
        continue
      }
      if (!entry.name.toLowerCase().endsWith('.md')) continue
      const markdown = await readText(workspaceRoot, entry.path)
      if (markdown == null) continue
      const properties = noteProperties(markdown)
      notes.push({
        path: entry.path,
        name: entry.name,
        title: noteTitle(entry.name, properties),
        markdown,
        tags: noteTags(markdown),
        properties,
        openTodo: noteHasOpenTodo(markdown),
        modifiedMs: entry.modifiedMs
      })
    }
  }
  await visit(personalNotesDirectory(workspaceRoot), 0)
  notes.sort((left, right) => left.title.localeCompare(right.title, 'zh'))
  return notes
}

/** Create one Markdown note under `wiki/local`. */
export async function createFlowixNote(workspaceRoot: string, fallback: string): Promise<string> {
  const stem = personalNoteStem('', fallback)
  return createPersonalNote(workspaceRoot, stem, `# ${stem}\n`, fallback)
}
