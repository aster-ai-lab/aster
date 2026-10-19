import {
  createLocalEntry,
  listLocalEntries,
  localEntryStatus,
  writeLocalDocumentBytes
} from '@renderer/pages/documents/useLocalDocuments'

import { ensurePersonalVault, isPersonalNotePath, personalVaultPath } from './knowledgeVault'

export const OPEN_PERSONAL_NOTE_EVENT = 'studio:open-personal-note'

export interface OpenPersonalNoteDetail {
  path: string
}

let pendingPath: string | null = null

/** Ask the knowledge page to show this personal note. Safe to call before that page is mounted. */
export function requestOpenPersonalNote(path: string): void {
  pendingPath = path
  window.dispatchEvent(new CustomEvent<OpenPersonalNoteDetail>(OPEN_PERSONAL_NOTE_EVENT, { detail: { path } }))
}

export function hasPendingPersonalNote(): boolean {
  return pendingPath !== null
}

export function consumePendingPersonalNote(): string | null {
  const path = pendingPath
  pendingPath = null
  return path
}

export function clearPendingPersonalNote(path: string): void {
  if (pendingPath === path) pendingPath = null
}

export function resetPendingPersonalNote(): void {
  pendingPath = null
}

/** File stem for a saved message. Empty titles use the translated fallback. */
export function personalNoteStem(title: string, fallback: string): string {
  const source = title.trim() || fallback.trim()
  const stem = source
    .replace(/\.md$/i, '')
    .replace(/[<>:"/\\|?*]/g, ' ')
    .replace(/[\u0000-\u001f]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/^\.+/, '')
    .slice(0, 80)
    .trim()
  return stem || 'note'
}

function noteBody(markdown: string): string {
  const body = markdown.replace(/^## [^\n]*\n+/, '').replace(/\s+$/u, '')
  return `${body}\n`
}

async function takenNames(workspaceRoot: string, directory: string): Promise<Set<string>> {
  try {
    const listing = await listLocalEntries(workspaceRoot, directory)
    return new Set(listing.entries.map((entry) => entry.name))
  } catch (error) {
    if (localEntryStatus(error) === 404) return new Set()
    throw error
  }
}

/** Write one markdown note into the personal vault and return its path. */
export async function createPersonalNote(
  workspaceRoot: string,
  title: string,
  markdown: string,
  fallback: string
): Promise<string> {
  await ensurePersonalVault(workspaceRoot)
  const parent = personalVaultPath(workspaceRoot)
  const taken = await takenNames(workspaceRoot, parent)
  const stem = personalNoteStem(title, fallback)
  let name = `${stem}.md`
  let suffix = 2
  while (taken.has(name)) {
    name = `${stem} ${suffix}.md`
    suffix += 1
  }
  const created = await createLocalEntry(workspaceRoot, parent, name, 'file')
  if (!isPersonalNotePath(workspaceRoot, created.path)) throw new Error('personal note path')
  await writeLocalDocumentBytes(workspaceRoot, created.path, new TextEncoder().encode(noteBody(markdown)))
  return created.path
}
