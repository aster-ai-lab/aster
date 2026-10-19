import type { VaultTreeNode } from './knowledgeVault'

const FRONTMATTER = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/
const SCHEME = /^[a-z][a-z0-9+.-]*:/i
const UNSAFE_SCHEME = /^(?:javascript|data|file):/i
const MARKDOWN_LINK = /\[([^\]]*)\]\(([^)\s]+)\)/g

export interface CompileCounts {
  compiled: number
  unchanged: number
  skipped: number
  failed: number
}

export interface IndexLink {
  title: string
  file: string
}

export interface IndexIssue {
  path: string
  reason: string
}

export interface IndexCatalog {
  concepts: IndexLink[]
  skipped: IndexIssue[]
  failed: IndexIssue[]
}

export interface BundleLink {
  label: string
  href: string
}

/** `wiki/local`, case-sensitive, matching the compile command. */
export function isPersonalVaultDirectory(path: string): boolean {
  const segments = path.split('/')
  return segments.at(-1) === 'local' && segments.at(-2) === 'wiki'
}

/** OKF reserves these file names at any depth. */
export function isReservedNoteName(name: string): boolean {
  return name === 'index.md' || name === 'log.md'
}

/** Index, log, and the compiler-owned concepts directory are not handwritten notes. */
export function isVaultArtifact(vaultRoot: string, path: string): boolean {
  const relative = bundleRelative(vaultRoot, path)
  if (relative == null) return false
  const name = relative.slice(relative.lastIndexOf('/') + 1)
  return isReservedNoteName(name) || relative === 'concepts' || relative.startsWith('concepts/')
}

export function bundleRelative(vaultRoot: string, path: string): string | null {
  const prefix = `${vaultRoot}/`
  if (!path.startsWith(prefix)) return null
  const relative = path.slice(prefix.length)
  if (!relative || relative.split('/').some((segment) => segment === '' || segment === '.' || segment === '..')) {
    return null
  }
  return relative
}

/** Hide the root concepts directory and every reserved note name. */
export function visibleNoteTree(nodes: readonly VaultTreeNode[]): VaultTreeNode[] {
  return hideReserved(nodes, true)
}

function hideReserved(nodes: readonly VaultTreeNode[], dropConcepts: boolean): VaultTreeNode[] {
  return nodes.flatMap((node) => {
    if (dropConcepts && node.kind === 'directory' && node.name === 'concepts') return []
    if (node.kind === 'file' && isReservedNoteName(node.name)) return []
    if (node.kind === 'directory') return [{ ...node, children: hideReserved(node.children, false) }]
    return [node]
  })
}

/** Keep files whose bundle-relative path is in `allowed`, plus the folders that hold them. */
export function treeMatchingPaths(
  nodes: readonly VaultTreeNode[],
  vaultRoot: string,
  allowed: ReadonlySet<string>
): VaultTreeNode[] {
  return nodes.flatMap((node) => {
    if (node.kind === 'file') {
      const relative = bundleRelative(vaultRoot, node.path)
      return relative && allowed.has(relative) ? [node] : []
    }
    const children = treeMatchingPaths(node.children, vaultRoot, allowed)
    return children.length > 0 ? [{ ...node, children }] : []
  })
}

/** `compiled=1 unchanged=0 skipped=2 failed=3` */
export function parseCompileReport(report: string): CompileCounts | null {
  const match = /^compiled=(\d+) unchanged=(\d+) skipped=(\d+) failed=(\d+)$/.exec(report.trim())
  if (!match) return null
  return {
    compiled: Number(match[1]),
    unchanged: Number(match[2]),
    skipped: Number(match[3]),
    failed: Number(match[4])
  }
}

/**
 * How far this compile has moved since it started.
 * Only a concept file written after the start counts. A newer index does not.
 */
export function compileRunProgress(input: {
  noteTotal: number
  startedAt: number
  conceptModifiedMs: readonly number[]
  pendingTotal?: number | null
  pendingModifiedMs?: readonly number[] | null
}): { done: number; total: number } {
  const totalSource = input.pendingTotal == null ? input.noteTotal : input.pendingTotal
  const total = Math.max(0, totalSource)
  const samples = input.pendingTotal == null ? input.conceptModifiedMs : (input.pendingModifiedMs ?? [])
  const written = samples.filter((modifiedMs) => modifiedMs >= input.startedAt).length
  return { done: Math.min(total, written), total }
}

/** Concept links, then the skipped and failed source lists from one bundle index. */
export function parseIndexCatalog(markdown: string): IndexCatalog {
  const concepts: IndexLink[] = []
  const skipped: IndexIssue[] = []
  const failed: IndexIssue[] = []
  let section: 'concepts' | 'skipped' | 'failed' = 'concepts'
  for (const line of markdown.split(/\r?\n/)) {
    if (line === '## Skipped') {
      section = 'skipped'
      continue
    }
    if (line === '## Failed') {
      section = 'failed'
      continue
    }
    if (section === 'concepts') {
      const link = /^- \[([^\]]*)\]\((\/concepts\/[^)\s]+)\)\s*$/.exec(line)
      if (!link) continue
      const file = link[2].slice(1)
      if (file.split('/').includes('..')) continue
      concepts.push({ title: link[1], file })
      continue
    }
    const issue = /^- `([^`]+)` — (.*)$/.exec(line)
    if (!issue) continue
    const item = { path: issue[1], reason: issue[2].trim() }
    if (section === 'skipped') skipped.push(item)
    else failed.push(item)
  }
  return { concepts, skipped, failed }
}

interface NoteEnvelope {
  body: string
  typeLine: string | null
}

/**
 * Frontmatter that is only `type` and `title` is the note envelope.
 * Any other key keeps the original text, fence included.
 */
function noteEnvelope(markdown: string): NoteEnvelope | null {
  const match = FRONTMATTER.exec(markdown)
  if (!match) return null
  const meaningful = match[1].split(/\r?\n/).filter((line) => line.trim() !== '')
  if (meaningful.length === 0) return null
  let typeLine: string | null = null
  for (const line of meaningful) {
    if (/^type:\s*\S/.test(line)) {
      if (typeLine) return null
      typeLine = line
      continue
    }
    if (/^title:\s*/.test(line)) continue
    return null
  }
  return { body: markdown.slice(match[0].length), typeLine }
}

/** Editor text. A type/title envelope stays out of the writing surface. */
export function noteEditorBody(markdown: string): string {
  return noteEnvelope(markdown)?.body ?? markdown
}

/** The type line to put back on save. Absent when the file had no simple envelope. */
export function noteTypeLine(markdown: string): string | null {
  return noteEnvelope(markdown)?.typeLine ?? null
}

/** Put an existing type line back. A file that already has `type` is left as written. */
export function withNoteType(markdown: string, typeLine: string | null): string {
  if (!typeLine) return markdown
  const match = FRONTMATTER.exec(markdown)
  if (!match) return `---\n${typeLine}\n---\n${markdown}`
  const lines = match[1].split(/\r?\n/)
  if (lines.some((line) => /^type:\s*/.test(line))) return markdown
  const inner = [typeLine, ...lines.filter((line) => line.trim() !== '')]
  return `---\n${inner.join('\n')}\n---\n${markdown.slice(match[0].length)}`
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

export interface ConceptDocument {
  type: string | null
  title: string | null
  sources: Array<{ title: string; resource: string }>
}

/** Top-level type and title, plus the `sources` list. Nested keys are not a second type. */
export function conceptDocument(markdown: string): ConceptDocument {
  const match = FRONTMATTER.exec(markdown)
  if (!match) return { type: null, title: null, sources: [] }
  let type: string | null = null
  let title: string | null = null
  let inSources = false
  let current: { title?: string; resource?: string } | null = null
  const sources: Array<{ title: string; resource: string }> = []
  const flush = () => {
    if (current?.resource) sources.push({ title: current.title ?? '', resource: current.resource })
    current = null
  }
  for (const line of match[1].split(/\r?\n/)) {
    if (/^sources:\s*$/.test(line)) {
      flush()
      inSources = true
      continue
    }
    if (inSources && /^\S/.test(line)) {
      flush()
      inSources = false
    }
    if (inSources) {
      if (/^\s+-\s+/.test(line)) flush()
      const resource = /^\s+resource:\s*(.*)$/.exec(line)
      if (resource) {
        current = current ?? {}
        current.resource = yamlUnquote(resource[1])
        continue
      }
      const sourceTitle = /^\s+title:\s*(.*)$/.exec(line)
      if (sourceTitle) {
        current = current ?? {}
        current.title = yamlUnquote(sourceTitle[1])
      }
      continue
    }
    if (/^\s/.test(line)) continue
    const typeMatch = /^type:\s*(.*)$/.exec(line)
    if (typeMatch && type == null) type = yamlUnquote(typeMatch[1])
    const titleMatch = /^title:\s*(.*)$/.exec(line)
    if (titleMatch && title == null) {
      const value = yamlUnquote(titleMatch[1])
      title = value || null
    }
  }
  flush()
  return { type, title, sources }
}

/** Body shown for a concept. The sources section is the rail, not the page. */
export function conceptProse(markdown: string): string {
  const match = FRONTMATTER.exec(markdown)
  const body = match ? markdown.slice(match[0].length) : markdown
  const lines = body.split(/\r?\n/)
  const cut = lines.findIndex((line) => line === '## Sources')
  return (cut >= 0 ? lines.slice(0, cut) : lines).join('\n')
}

export function markdownLinks(markdown: string): BundleLink[] {
  const links: BundleLink[] = []
  for (const match of markdown.matchAll(MARKDOWN_LINK)) {
    const href = match[2]?.trim() ?? ''
    if (!href) continue
    links.push({ label: (match[1] ?? '').trim(), href })
  }
  return links
}

/**
 * Resolve a markdown link against the bundle.
 * `../甲.md` from `concepts/甲.md` is `甲.md`. A leading slash is bundle-root.
 * Schemes and a `..` that leaves the bundle return null.
 */
export function resolveBundleReference(fromFile: string, href: string): string | null {
  const bare = href.split('#')[0]?.split('?')[0]?.trim() ?? ''
  if (!bare || SCHEME.test(bare)) return null
  let decoded = bare
  try {
    decoded = decodeURIComponent(bare)
  } catch {
    return null
  }
  const fromDir = fromFile.includes('/') ? fromFile.slice(0, fromFile.lastIndexOf('/')) : ''
  const combined = decoded.startsWith('/') ? decoded.slice(1) : fromDir ? `${fromDir}/${decoded}` : decoded
  const parts: string[] = []
  for (const segment of combined.split('/')) {
    if (!segment || segment === '.') continue
    if (segment === '..') {
      if (parts.length === 0) return null
      parts.pop()
      continue
    }
    parts.push(segment)
  }
  return parts.length > 0 ? parts.join('/') : null
}

export function conceptOutlinks(markdown: string, fromFile: string, files: ReadonlySet<string>): BundleLink[] {
  const links: BundleLink[] = []
  const seen = new Set<string>()
  for (const link of markdownLinks(conceptProse(markdown))) {
    if (SCHEME.test(link.href)) continue
    const resolved = resolveBundleReference(fromFile, link.href)
    if (!resolved || !files.has(resolved) || seen.has(resolved)) continue
    seen.add(resolved)
    links.push({ label: link.label || resolved, href: resolved })
  }
  return links
}

/** Dangling file links and unsafe schemes. External web URLs are not diagnostics. */
export function conceptDiagnostics(markdown: string, fromFile: string, files: ReadonlySet<string>): string[] {
  const labels: string[] = []
  const seen = new Set<string>()
  const match = FRONTMATTER.exec(markdown)
  const body = match ? markdown.slice(match[0].length) : markdown
  for (const link of markdownLinks(body)) {
    const unsafe = UNSAFE_SCHEME.test(link.href)
    if (!unsafe && SCHEME.test(link.href)) continue
    const resolved = unsafe ? null : resolveBundleReference(fromFile, link.href)
    if (resolved && files.has(resolved)) continue
    const label = link.label || link.href
    if (!label || seen.has(label)) continue
    seen.add(label)
    labels.push(label)
  }
  return labels
}

/** Concept pages whose saved text matches and whose title and file name do not. */
export function conceptPassageHits<T extends { name: string; title: string; markdown: string | null }>(
  concepts: readonly T[],
  needle: string
): T[] {
  const query = needle.trim().toLowerCase()
  if (!query) return []
  return concepts.filter((concept) => {
    if (concept.markdown == null) return false
    const name = concept.name.toLowerCase()
    if (name.includes(query) || concept.title.toLowerCase().includes(query)) return false
    return concept.markdown.toLowerCase().includes(query)
  })
}
