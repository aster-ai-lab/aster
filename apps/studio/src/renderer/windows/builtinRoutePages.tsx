import { getRouteApi } from '@tanstack/react-router'
import { lazy, useCallback } from 'react'

import { TeamDrivePage } from '@renderer/pages/documents/TeamDrivePage'
import type { CodeCli } from '@shared/types/codeCli'

const Files = lazy(() => import('@renderer/pages/files/FilesPage'))
const Knowledge = lazy(() => import('@renderer/pages/knowledge/KnowledgePage'))
const CodeTools = lazy(() => import('@renderer/pages/code/CodeCliPage'))
const filesRoute = getRouteApi('/app/files')
const codeRoute = getRouteApi('/app/code')

export function FilesPluginPage() {
  const { entryId, driveFile } = filesRoute.useSearch()
  const navigate = filesRoute.useNavigate()
  const onChange = useCallback(
    (nextEntryId?: string) =>
      void navigate({
        search: { ...(driveFile ? { driveFile } : {}), ...(nextEntryId ? { entryId: nextEntryId } : {}) },
        replace: true
      }),
    [navigate, driveFile]
  )
  return <Files entryId={entryId} driveFileId={driveFile} onEntryIdChange={onChange} />
}

export function KnowledgePluginPage() {
  return <Knowledge />
}

/** 团队侧栏云盘。百科在 ActivityBar 的知识页。 */
export function TeamKnowledgePage() {
  return <TeamDrivePage />
}

export function CodeToolsPluginPage() {
  const { tool } = codeRoute.useSearch()
  const navigate = codeRoute.useNavigate()
  const onChange = useCallback(
    (selectedTool: CodeCli) => void navigate({ search: { tool: selectedTool }, replace: true }),
    [navigate]
  )
  return <CodeTools initialTool={tool} onToolChange={onChange} />
}
