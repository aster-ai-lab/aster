import { fileExtension, isWorkspaceFileEditorKind, previewKind } from '@renderer/pages/documents/documentPreview'
import { openFileEditorWindow } from '@renderer/pages/documents/openFileEditorWindow'
import { importLocalFile } from '@renderer/pages/documents/useLocalDocuments'

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

type PreviewSlot = { root?: unknown; directory?: unknown }

/** Raster pictures stay on the image preview. SVG and every other readable file use the editor. */
export function opensInMessageFileEditor(name: string): boolean {
  const kind = previewKind(name)
  if (kind === 'image' && fileExtension(name) !== 'svg') return false
  return isWorkspaceFileEditorKind(kind)
}

function previewKey(raw: string): string {
  const cleaned = raw.replace(/[\\/\u0000-\u001f]/g, '-').trim()
  if (!cleaned || cleaned === '.' || cleaned === '..') return 'file'
  return cleaned.slice(0, 120)
}

function filePath(directory: string, name: string): string {
  const separator = directory.includes('\\') ? '\\' : '/'
  return directory.endsWith('/') || directory.endsWith('\\') ? `${directory}${name}` : `${directory}${separator}${name}`
}

/**
 * Stage one chat file and open it in the documents file-editor window.
 * Returns false when the shell cannot open that window, so the caller can keep the in-page preview.
 */
export async function openMessageFileEditor(file: File, key: string): Promise<boolean> {
  if (!opensInMessageFileEditor(file.name)) return false
  const invoke = (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__
    ?.invoke
  if (!invoke) return false
  try {
    const slot = (await invoke('prepare_message_file_preview', { key: previewKey(key) })) as PreviewSlot
    const root = typeof slot.root === 'string' ? slot.root : ''
    const directory = typeof slot.directory === 'string' ? slot.directory : ''
    if (!root || !directory) return false
    await importLocalFile(root, directory, file)
    return openFileEditorWindow({
      root,
      path: filePath(directory, file.name),
      title: file.name,
      readOnly: true
    })
  } catch {
    return false
  }
}
