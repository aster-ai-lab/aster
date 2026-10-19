/**
 * 组织云盘数据编排（files 页 UI 的唯一数据依赖；DRIVE.md §4）。
 * 目录导航 + 文件列表 + 上传/下载 + 就绪门控（未登录回退隐藏组织空间）。
 */
import { useCallback, useEffect, useRef, useState } from 'react'

import { PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { readStudioRuntime } from '@shared/studioRuntime'

import { driveApi, DriveApiError, type DriveFile, type DriveNode } from './driveTransport'

export interface UseDrive {
  ready: boolean
  nodes: DriveNode[]
  files: DriveFile[]
  loading: boolean
  error: string | null
  openFolder: (parentId: string | null) => Promise<void>
  createFolder: (name: string, deptId?: string) => Promise<void>
  renameNode: (nodeId: string, name: string) => Promise<void>
  moveNode: (nodeId: string, parentId: string | null) => Promise<void>
  deleteNode: (nodeId: string) => Promise<void>
  setFolderEncyclopedia: (nodeId: string, excluded: boolean) => Promise<void>
  upload: (file: File) => Promise<void>
  refresh: () => Promise<void>
}

export function driveSessionReady(): boolean {
  try {
    if (!readStudioRuntime()?.apiBaseUrl) return false
  } catch {
    return false
  }
  const center = readCenterId()
  return Boolean(center && center !== PERSONAL_CENTER)
}

export function useDrive(): UseDrive {
  const [parentId, setParentId] = useState<string | null>(null)
  const [nodes, setNodes] = useState<DriveNode[]>([])
  const [files, setFiles] = useState<DriveFile[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [ready, setReady] = useState(driveSessionReady)
  const inflight = useRef(0)

  useEffect(() => subscribeCenter(() => setReady(driveSessionReady())), [])

  const load = useCallback(async (parent: string | null) => {
    inflight.current += 1
    const ticket = inflight.current
    setLoading(true)
    setError(null)
    try {
      const [nextNodes, nextFiles] = await Promise.all([
        driveApi.listNodes(parent ?? undefined),
        driveApi.listFiles(parent ?? undefined)
      ])
      if (ticket !== inflight.current) return // 过期响应丢弃（快速切换目录）
      setNodes(nextNodes)
      setFiles(nextFiles)
    } catch (cause) {
      if (ticket !== inflight.current) return
      setError(cause instanceof DriveApiError ? cause.message : 'drive load failed')
    } finally {
      if (ticket === inflight.current) setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (ready) void load(parentId)
  }, [ready, parentId, load])

  const mutate = useCallback(
    async (operation: () => Promise<void>) => {
      try {
        await operation()
        await load(parentId)
      } catch (cause) {
        setError(cause instanceof DriveApiError ? cause.message : 'drive operation failed')
      }
    },
    [load, parentId]
  )

  const openFolder = useCallback(async (next: string | null) => {
    setParentId(next)
  }, [])

  return {
    ready,
    nodes,
    files,
    loading,
    error,
    openFolder,
    createFolder: (name, deptId) =>
      mutate(async () => {
        await driveApi.createFolder(name, parentId ?? undefined, deptId)
      }),
    renameNode: (nodeId, name) => mutate(() => driveApi.renameNode(nodeId, name)),
    moveNode: (nodeId, target) => mutate(() => driveApi.moveNode(nodeId, target)),
    deleteNode: (nodeId) => mutate(() => driveApi.deleteNode(nodeId)),
    setFolderEncyclopedia: (nodeId, excluded) => mutate(() => driveApi.setFolderEncyclopedia(nodeId, excluded)),
    upload: (file) => mutate(() => driveApi.upload(file, parentId ?? undefined)),
    refresh: () => load(parentId)
  }
}

/** 面包屑路径（由全量节点缓存推导；UI 辅助）。 */
export function breadcrumb(nodes: DriveNode[], leafId: string | null): DriveNode[] {
  const map = new Map(nodes.map((node) => [node.id, node]))
  const path: DriveNode[] = []
  let current = leafId ? map.get(leafId) : undefined
  let guard = 0
  while (current && guard < 32) {
    path.unshift(current)
    current = current.parentId ? map.get(current.parentId) : undefined
    guard += 1
  }
  return path
}
