import {
  getArtifactPaneSelectionPath,
  resolveArtifactPaneFileSelection
} from '@renderer/components/chat/panes/artifactPanePath'
import { resolveInlineFilePath } from '@renderer/utils/filePath'

export interface ConversationEditorTarget {
  path: string
  name: string
}

/** Editor target for a conversation file. An http(s) link is not a workspace file. */
export function conversationEditorTarget(
  workspacePath: string | undefined,
  rawPath: string
): ConversationEditorTarget | null {
  const trimmed = rawPath.trim()
  if (!trimmed || /^https?:\/\//i.test(trimmed)) return null
  const selection = resolveArtifactPaneFileSelection(workspacePath, resolveInlineFilePath(trimmed))
  if (!selection) return null
  const path = getArtifactPaneSelectionPath(selection)
  if (!path || /^https?:\/\//i.test(path)) return null
  const name = selection.filePath.split(/[/\\]/u).pop() || path.split(/[/\\]/u).pop() || path
  return { path, name }
}

/** Opens one conversation file through the workspace editor. Returns false for a web link. */
export function openConversationFileInWorkspace(
  rawPath: string,
  workspacePath: string | undefined,
  openEditor: (path: string, name: string) => void
): boolean {
  const target = conversationEditorTarget(workspacePath, rawPath)
  if (!target) return false
  openEditor(target.path, target.name)
  return true
}
