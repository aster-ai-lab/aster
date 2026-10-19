import { createLocalEntry, listLocalEntries, localEntryStatus } from '@renderer/pages/documents/useLocalDocuments'

const MARKDOWN = /\.(?:md|markdown)$/i
const MAX_DEPTH = 6

export interface VaultFile {
  name: string
  path: string
}

export interface VaultTreeNode {
  name: string
  path: string
  kind: 'file' | 'directory'
  children: VaultTreeNode[]
  /** Listing timestamp for a file. Recency does not read the note body. */
  modifiedMs?: number
}

/** Personal notes live here. The directory is the vault even before the first note. */
export function personalVaultPath(workspaceRoot: string): string {
  return `${workspaceRoot}/wiki/local`
}

/** Create `wiki/local` when a workspace has no personal notes yet. */
export async function ensurePersonalVault(workspaceRoot: string): Promise<void> {
  try {
    await createLocalEntry(workspaceRoot, workspaceRoot, 'wiki', 'folder')
  } catch (error) {
    if (localEntryStatus(error) !== 409) throw error
  }
  try {
    await createLocalEntry(workspaceRoot, `${workspaceRoot}/wiki`, 'local', 'folder')
  } catch (error) {
    if (localEntryStatus(error) !== 409) throw error
  }
}

/** A note file under the personal vault, rejecting paths that leave it. */
export function isPersonalNotePath(workspaceRoot: string, path: string): boolean {
  const vault = personalVaultPath(workspaceRoot)
  if (!path.startsWith(`${vault}/`)) return false
  return !path
    .slice(vault.length + 1)
    .split('/')
    .some((segment) => segment === '' || segment === '.' || segment === '..')
}

/** Installed cloud bundles live here, one directory per knowledge base. */
export function cloudVaultPath(workspaceRoot: string): string {
  return `${workspaceRoot}/wiki/cloud`
}

export function cloudBundlePath(workspaceRoot: string, id: string): string {
  return `${cloudVaultPath(workspaceRoot)}/${id}`
}

/** Direct children of one vault directory. A missing directory is an empty list. */
export async function listVaultTree(workspaceRoot: string, directory: string, depth = 0): Promise<VaultTreeNode[]> {
  let listing: Awaited<ReturnType<typeof listLocalEntries>>
  try {
    listing = await listLocalEntries(workspaceRoot, directory)
  } catch (error) {
    if (localEntryStatus(error) === 404) return []
    throw error
  }
  const nodes: VaultTreeNode[] = []
  for (const entry of listing.entries) {
    if (entry.kind === 'directory') {
      nodes.push({
        name: entry.name,
        path: entry.path,
        kind: 'directory',
        children: depth < MAX_DEPTH ? await listVaultTree(workspaceRoot, entry.path, depth + 1) : []
      })
      continue
    }
    if (MARKDOWN.test(entry.name)) {
      nodes.push({
        name: entry.name,
        path: entry.path,
        kind: 'file',
        children: [],
        modifiedMs: entry.modifiedMs
      })
    }
  }
  return nodes
}

export function flattenVaultFiles(nodes: readonly VaultTreeNode[]): VaultFile[] {
  const files: VaultFile[] = []
  for (const node of nodes) {
    if (node.kind === 'file') files.push({ name: node.name, path: node.path })
    else files.push(...flattenVaultFiles(node.children))
  }
  return files
}

/** Markdown files under one vault directory. A missing directory is an empty vault. */
export async function listMarkdownNotes(workspaceRoot: string, directory: string): Promise<VaultFile[]> {
  return flattenVaultFiles(await listVaultTree(workspaceRoot, directory))
}

/** Newest listing timestamps first. Equal times keep a stable path order. */
export function recentNotes(nodes: readonly VaultTreeNode[], limit = 5): VaultTreeNode[] {
  const files: VaultTreeNode[] = []
  const walk = (list: readonly VaultTreeNode[]) => {
    for (const node of list) {
      if (node.kind === 'file') files.push(node)
      else walk(node.children)
    }
  }
  walk(nodes)
  return files
    .sort((left, right) => (right.modifiedMs ?? 0) - (left.modifiedMs ?? 0) || left.path.localeCompare(right.path))
    .slice(0, Math.max(0, limit))
}
