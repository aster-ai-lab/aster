const SOURCE_EXT = new Set([
  'md',
  'markdown',
  'txt',
  'csv',
  'tsv',
  'json',
  'html',
  'htm',
  'rtf',
  'docx',
  'xlsx',
  'pptx',
  'odt',
  'ods',
  'odp',
  'pdf'
])

function stripSourceExtension(path: string): string {
  const ext = path.split('.').pop()?.toLowerCase() ?? ''
  if (!SOURCE_EXT.has(ext)) return path
  const dot = path.lastIndexOf('.')
  return dot <= 0 ? path : path.slice(0, dot)
}

export function sourceStem(name: string): string {
  const base = name.split(/[/\\]/).pop() ?? name
  return stripSourceExtension(base) || base
}

/** A compiled concept has a type and the source generation that produced it. */
export function isOkfConcept(markdown: string): boolean {
  return (
    markdown.startsWith('---\n') &&
    /\ntype:\s+\S/.test(markdown) &&
    /\nsource_generation:\s+\S/.test(markdown) &&
    /\nparser_generation:\s+"builtin:1"/.test(markdown)
  )
}

/** Index frontmatter may contain only the OKF version. */
export function isOkfIndex(markdown: string): boolean {
  if (!markdown.startsWith('---\n')) return false
  const end = markdown.indexOf('\n---\n', 4)
  if (end < 0) return false
  return markdown.slice(4, end).trim() === 'okf_version: "0.2"'
}

/** Bundle root. Only `okf_version` belongs in this frontmatter. */
export function okfIndex(concepts: readonly { title: string; file: string }[]): string {
  const links = concepts
    .map((item) => `- [${item.title.replace(/[[\]()\r\n]/g, '').slice(0, 200)}](/${item.file})`)
    .join('\n')
  return `---\nokf_version: "0.2"\n---\n\n${links}\n`
}

/** Concept file name inside `wiki/`. Slashes in the source path stay inside one file name. */
export function conceptFileName(relative: string): string {
  const stem = stripSourceExtension(relative)
  return `${stem.replaceAll('/', '__')}.md`
}
