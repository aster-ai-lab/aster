import { BookOpen } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import { createLocalEntry, localEntryStatus } from '@renderer/pages/documents/useLocalDocuments'
import { KnowledgeNotesPage } from '@renderer/pages/knowledge/KnowledgeNotesPage'
import { seedPluginOkf } from '@renderer/pages/plugins/pluginOkf'
import { displayedWorkspacePath } from '@renderer/pages/settings/GeneralSettings/appWorkspace'
import { readStudioHomePath } from '@shared/studioRuntime'

import { agentWikiDirectory } from './agentWiki'
import { AppletWindowBar } from './AppletWindowBar'

async function ensureDirectory(root: string, parent: string, name: string) {
  try {
    await createLocalEntry(root, parent, name, 'folder')
  } catch (error) {
    if (localEntryStatus(error) !== 409) throw error
  }
}

async function ensureAgentWiki(root: string, directory: string) {
  await ensureDirectory(root, root, 'wiki')
  await ensureDirectory(root, `${root}/wiki`, 'agents')
  const id = directory.split('/').pop()
  if (!id) return
  await ensureDirectory(root, `${root}/wiki/agents`, id)
}

/** Native encyclopedia window: applet chrome, knowledge file tree, knowledge editor. */
export function AgentWikiWindow({ agentId, title }: { agentId: string; title: string }) {
  const { t } = useTranslation()
  const [workspaceStored] = usePreference('app.workspace.path')
  const workspaceRoot = displayedWorkspacePath(
    typeof workspaceStored === 'string' ? workspaceStored : '',
    readStudioHomePath()
  )
  const directory = workspaceRoot ? agentWikiDirectory(workspaceRoot, agentId) : null
  const [ready, setReady] = useState(false)
  const [reloadKey, setReloadKey] = useState(0)

  useEffect(() => {
    if (!workspaceRoot || !directory) return
    let cancelled = false
    setReady(false)
    void ensureAgentWiki(workspaceRoot, directory)
      .then(() => seedPluginOkf(agentId, directory))
      .catch(() => undefined)
      .finally(() => {
        if (!cancelled) setReady(true)
      })
    return () => {
      cancelled = true
    }
  }, [workspaceRoot, directory, reloadKey])

  const reload = () => setReloadKey((current) => current + 1)

  return (
    <div className="fixed inset-0 flex w-full flex-col overflow-hidden bg-white">
      <AppletWindowBar
        title={title}
        icon={<BookOpen className="size-4 text-[#171717]" strokeWidth={1.7} aria-hidden />}
        onHome={reload}
        onRefresh={reload}
      />
      <div className="flex min-h-0 w-full flex-1 flex-col">
        {ready && directory ? (
          <KnowledgeNotesPage key={reloadKey} vaultDirectory={directory} />
        ) : (
          <p className="m-0 px-6 py-8 text-[14px] text-[#8a8a8a]">{t('capabilities.expert.wiki_empty')}</p>
        )}
      </div>
    </div>
  )
}
