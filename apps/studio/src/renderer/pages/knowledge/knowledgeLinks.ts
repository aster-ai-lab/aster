export interface NoteLink {
  target: string
  label: string
}

const WIKI_LINK = /\[\[([^\]|#]+)(?:#[^\]|]+)?(?:\|([^\]]+))?\]\]/g
const MARKDOWN_LINK = /\[([^\]]*)\]\(([^)\s]+)\)/g

/** The note editor writes a typed bracket as a backslash escape. */
export function typedMarkdown(markdown: string): string {
  return markdown.replace(/\\([[\]])/g, '$1')
}

export function noteTitle(name: string): string {
  const base = name.split(/[/\\]/).pop() ?? name
  return base.replace(/\.(?:markdown|md)$/i, '')
}

function isExternal(target: string): boolean {
  return /^[a-z][a-z0-9+.-]*:/i.test(target)
}

function linkLabel(target: string, label: string): string {
  const trimmed = label.trim()
  return trimmed || noteTitle(target)
}

/** Wikilinks and markdown links in one note. External URLs are not note targets. */
export function noteLinks(markdown: string): NoteLink[] {
  const links: NoteLink[] = []
  const source = typedMarkdown(markdown)
  for (const match of source.matchAll(WIKI_LINK)) {
    const target = match[1]?.trim() ?? ''
    if (!target) continue
    links.push({ target, label: linkLabel(target, match[2] ?? '') })
  }
  for (const match of source.matchAll(MARKDOWN_LINK)) {
    const target = match[2]?.trim() ?? ''
    if (!target || isExternal(target)) continue
    links.push({ target, label: linkLabel(target, match[1] ?? '') })
  }
  return links
}

export function findNote<T extends { name: string }>(notes: readonly T[], target: string): T | undefined {
  const path = target.split('#')[0]?.split('?')[0] ?? ''
  let decoded = path
  try {
    decoded = decodeURIComponent(path)
  } catch {
    decoded = path
  }
  const key = noteTitle(decoded)
  if (!key) return undefined
  return notes.find((note) => noteTitle(note.name) === key)
}

/** Notes in this library whose text links to `targetName`. Other libraries are not searched. */
export function backlinkNotes<T extends { name: string; markdown: string | null }>(
  notes: readonly T[],
  targetName: string
): T[] {
  const key = noteTitle(targetName)
  if (!key) return []
  return notes.filter((note) => {
    if (note.markdown == null || noteTitle(note.name) === key) return false
    return noteLinks(note.markdown).some((link) => noteTitle(link.target) === key)
  })
}
