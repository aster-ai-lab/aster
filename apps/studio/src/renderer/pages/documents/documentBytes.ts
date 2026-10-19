import { driveApi } from '@renderer/drive/driveTransport'

import type { LibraryDocument } from './documentLibrary'
import { FileTooLargeError, readLocalDocumentBytes } from './useLocalDocuments'

export async function readDocumentBytes(
  document: Pick<LibraryDocument, 'id' | 'place' | 'locationPath'>,
  workspaceRoot: string,
  cloudReady: boolean,
  onProgress?: (ratio: number) => void,
  maxBytes?: number,
  onSnapshot?: (revision: string) => void
): Promise<Uint8Array> {
  if (document.place === 'local') {
    if (!workspaceRoot || !document.locationPath) throw new Error('unavailable')
    return readLocalDocumentBytes(workspaceRoot, document.locationPath, onProgress, maxBytes, onSnapshot)
  }
  if (!cloudReady) throw new Error('unavailable')
  const blob = await driveApi.downloadBlob(document.id)
  if (maxBytes && blob.size > maxBytes) throw new FileTooLargeError(blob.size)
  const bytes = new Uint8Array(await blob.arrayBuffer())
  onProgress?.(1)
  return bytes
}
