import { createLocalEntry, localEntryStatus, writeLocalDocumentBytes } from '@renderer/pages/documents/useLocalDocuments'

import type { CloudKnowledgeBundle } from './cloudKnowledgeCatalog'
import { cloudBundlePath, cloudVaultPath } from './knowledgeVault'

const PLAIN_NAME = /^[^./\\][^/\\]{0,254}$/

async function ensureFolder(root: string, parent: string, name: string): Promise<void> {
  try {
    await createLocalEntry(root, parent, name, 'folder')
  } catch (error) {
    if (localEntryStatus(error) !== 409) throw error
  }
}

async function ensureFile(root: string, parent: string, name: string): Promise<void> {
  try {
    await createLocalEntry(root, parent, name, 'file')
  } catch (error) {
    if (localEntryStatus(error) !== 409) throw error
  }
}

export interface CloudDownloadProgress {
  completed: number
  total: number
  name: string
}

/** Download one OKF bundle into `wiki/cloud/<id>`. Existing files are replaced. */
export async function installCloudKnowledge(
  workspaceRoot: string,
  bundle: CloudKnowledgeBundle,
  onProgress?: (progress: CloudDownloadProgress) => void
): Promise<void> {
  if (!PLAIN_NAME.test(bundle.id) || bundle.files.some((file) => !PLAIN_NAME.test(file.name))) {
    throw new Error('cloud knowledge name')
  }
  const total = bundle.files.length
  onProgress?.({ completed: 0, total, name: '' })
  await ensureFolder(workspaceRoot, workspaceRoot, 'wiki')
  const cloudParent = `${workspaceRoot}/wiki`
  await ensureFolder(workspaceRoot, cloudParent, 'cloud')
  const cloud = cloudVaultPath(workspaceRoot)
  await ensureFolder(workspaceRoot, cloud, bundle.id)
  const destination = cloudBundlePath(workspaceRoot, bundle.id)
  const encoder = new TextEncoder()
  for (let index = 0; index < bundle.files.length; index += 1) {
    const file = bundle.files[index]
    if (!file) continue
    onProgress?.({ completed: index, total, name: file.name })
    await ensureFile(workspaceRoot, destination, file.name)
    await writeLocalDocumentBytes(workspaceRoot, `${destination}/${file.name}`, encoder.encode(file.markdown))
    onProgress?.({ completed: index + 1, total, name: file.name })
  }
}
