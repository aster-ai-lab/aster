/**
 * 云端文档数据源（documents 页 cloud 分区）：组织云盘文件 → LibraryDocument。
 * 页面接缝 = 用 useCloudDocuments().documents 替换 SEED_DOCUMENTS（登录态），
 * 未登录回退种子（与 IM S2 同款就绪门控）。
 */
import { useCallback, useEffect, useState } from 'react'

import { subscribeCenter } from '@renderer/data/memberOrgs'
import {
  formatForName,
  iconForName,
  kindForName,
  type LibraryDocument
} from '@renderer/pages/documents/documentLibrary'

import { driveApi, type DriveFile } from './driveTransport'
import { driveSessionReady } from './useDrive'

export interface CloudFolder {
  id: string
  name: string
}

export interface UseCloudDocuments {
  ready: boolean
  documents: LibraryDocument[]
  folders: CloudFolder[]
  folderDocuments: LibraryDocument[]
  reload: () => Promise<void>
  loadFolder: (nodeId: string) => Promise<void>
}

export function cloudDocumentFromFile(file: DriveFile): LibraryDocument {
  return {
    id: file.id,
    name: file.name,
    owner: '',
    place: 'cloud',
    folder: 'mine',
    kind: kindForName(file.name),
    icon: iconForName(file.name),
    format: formatForName(file.name),
    source: 'cloud_save',
    device: 'computer',
    activity: 'upload',
    locationPath: '',
    viewedAt: Date.parse(file.createdAt) || Date.now(),
    updatedAt: Date.parse(file.updatedAt ?? file.createdAt) || undefined,
    sizeBytes: file.sizeBytes,
    favorite: false,
    trashed: false
  }
}

export function useCloudDocuments(): UseCloudDocuments {
  const [documents, setDocuments] = useState<LibraryDocument[]>([])
  const [folders, setFolders] = useState<CloudFolder[]>([])
  const [folderDocuments, setFolderDocuments] = useState<LibraryDocument[]>([])
  const [ready, setReady] = useState(driveSessionReady)

  const reload = useCallback(async () => {
    try {
      const [rootFiles, rootNodes] = await Promise.all([driveApi.listFiles(), driveApi.listNodes()])
      setDocuments(rootFiles.map(cloudDocumentFromFile))
      setFolders(rootNodes.map((node) => ({ id: node.id, name: node.name })))
    } catch {
      setDocuments([])
      setFolders([])
    }
  }, [])

  const loadFolder = useCallback(async (nodeId: string) => {
    try {
      const rows = await driveApi.listFiles(nodeId)
      setFolderDocuments(rows.map((file) => ({ ...cloudDocumentFromFile(file), folder: nodeId })))
    } catch {
      setFolderDocuments([])
    }
  }, [])

  useEffect(() => subscribeCenter(() => setReady(driveSessionReady())), [])

  useEffect(() => {
    if (!ready) {
      setDocuments([])
      setFolders([])
      setFolderDocuments([])
      return
    }
    void reload()
  }, [ready, reload])

  return { ready, documents, folders, folderDocuments, reload, loadFolder }
}
