import { workspaceFileIconType, type DocumentFileIconType } from '@renderer/components/icons/DocumentFileIcon'
import { createComposerSecureRandomId } from '@renderer/utils/message/composerFileTokenSource'

import type { ComposerDraftToken } from './tokens'

export interface WorkspaceFileGroupEntry {
  path: string
  isDirectory: boolean
}

export interface WorkspaceFileTreeNode {
  name: string
  path: string
  isDirectory: boolean
  children: WorkspaceFileTreeNode[]
}

const normalizePath = (value: string) => value.replace(/\\/g, '/').replace(/\/+$/, '')

const baseName = (value: string) => {
  const normalized = normalizePath(value)
  return normalized.split('/').pop() || normalized
}

/** Icon family used by the workspace file tree, for the chip that stands in for this selection. */
export function workspaceGroupIcon(entries: readonly WorkspaceFileGroupEntry[]): 'directory' | DocumentFileIconType {
  const files = entries.filter((entry) => !entry.isDirectory)
  if (files.length === 0) return 'directory'
  if (entries.length === 1) return workspaceFileIconType(baseName(files[0].path))
  const types = files.map((entry) => workspaceFileIconType(baseName(entry.path)))
  return types.every((type) => type === types[0]) ? types[0] : 'file'
}

function dedupeEntries(choices: readonly WorkspaceFileGroupEntry[]): WorkspaceFileGroupEntry[] {
  const seen = new Set<string>()
  const entries: WorkspaceFileGroupEntry[] = []
  for (const choice of choices) {
    const path = normalizePath(choice.path)
    if (!path || seen.has(path)) continue
    seen.add(path)
    entries.push({ path, isDirectory: choice.isDirectory })
  }
  return entries
}

/** One chip per confirmed picker pass. Directories keep a trailing slash so the tree can be rebuilt from the prompt. */
export function createWorkspaceFileGroupToken(
  choices: readonly WorkspaceFileGroupEntry[],
  label: string
): ComposerDraftToken {
  const entries = dedupeEntries(choices)
  const promptText = entries.map((entry) => (entry.isDirectory ? `${entry.path}/` : entry.path)).join('\n')
  return {
    id: createComposerSecureRandomId('folder-token'),
    kind: 'folder',
    label,
    description: label,
    promptText,
    payload: { workspaceFileGroup: { entries } }
  }
}

export function workspaceFileGroupLabel(
  choices: readonly WorkspaceFileGroupEntry[],
  countLabel: (count: number) => string
): string {
  const entries = dedupeEntries(choices)
  if (entries.length === 0) return ''
  if (entries.length === 1) return baseName(entries[0].path)
  return countLabel(entries.length)
}

export function readWorkspaceFileGroup(token: {
  promptText?: string
  payload?: unknown
}): WorkspaceFileGroupEntry[] | null {
  const payload = token.payload
  if (payload && typeof payload === 'object' && 'workspaceFileGroup' in payload) {
    const group = (payload as { workspaceFileGroup?: { entries?: unknown } }).workspaceFileGroup
    if (group && Array.isArray(group.entries)) {
      const entries = dedupeEntries(
        group.entries.flatMap((entry) => {
          if (!entry || typeof entry !== 'object') return []
          const path = (entry as { path?: unknown }).path
          if (typeof path !== 'string' || !path.trim()) return []
          return [{ path, isDirectory: (entry as { isDirectory?: unknown }).isDirectory === true }]
        })
      )
      if (entries.length > 0) return entries
    }
  }

  const text = token.promptText ?? ''
  if (!text.includes('\n')) return null
  const entries = dedupeEntries(
    text
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => ({ path: line, isDirectory: line.endsWith('/') }))
  )
  return entries.length > 0 ? entries : null
}

function commonParent(paths: readonly string[]): string {
  if (paths.length === 0) return ''
  const split = paths.map((path) => path.split('/'))
  let shared = split[0].length
  for (const parts of split) {
    let index = 0
    while (index < shared && index < parts.length && parts[index] === split[0][index]) index += 1
    shared = index
  }
  const prefix = split[0].slice(0, shared).join('/')
  if (paths.length === 1 || paths.some((path) => path === prefix)) {
    return split[0].slice(0, Math.max(0, shared - 1)).join('/')
  }
  return prefix
}

function sortNodes(nodes: WorkspaceFileTreeNode[]) {
  nodes.sort((left, right) => {
    if (left.isDirectory !== right.isDirectory) return left.isDirectory ? -1 : 1
    return left.name.localeCompare(right.name)
  })
  for (const node of nodes) sortNodes(node.children)
}

/** Selected paths only. Unselected siblings stay out; missing parents stay so a nested file still has a folder. */
export function buildWorkspaceFileTree(entries: readonly WorkspaceFileGroupEntry[]): WorkspaceFileTreeNode[] {
  const normalized = dedupeEntries(entries)
  const prefix = commonParent(normalized.map((entry) => entry.path))
  const roots: WorkspaceFileTreeNode[] = []
  const index = new Map<string, WorkspaceFileTreeNode>()

  for (const entry of normalized) {
    const relative =
      prefix && (entry.path === prefix || entry.path.startsWith(`${prefix}/`))
        ? entry.path.slice(prefix.length).replace(/^\//, '')
        : baseName(entry.path)
    const parts = relative.split('/').filter(Boolean)
    if (parts.length === 0) parts.push(baseName(entry.path))

    let children = roots
    let accumulated = prefix
    parts.forEach((part, partIndex) => {
      accumulated = accumulated ? `${accumulated}/${part}` : part
      const last = partIndex === parts.length - 1
      let node = index.get(accumulated)
      if (!node) {
        node = {
          name: part,
          path: accumulated,
          isDirectory: last ? entry.isDirectory : true,
          children: []
        }
        index.set(accumulated, node)
        children.push(node)
      } else if (last && entry.isDirectory) {
        node.isDirectory = true
      }
      children = node.children
    })
  }

  sortNodes(roots)
  return roots
}
