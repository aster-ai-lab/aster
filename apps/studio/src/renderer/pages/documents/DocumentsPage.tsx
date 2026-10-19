import { type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { WorkbenchSessionFiles } from '@renderer/pages/agents/WorkbenchSessionFiles'
import { useDisplayedWorkspacePath } from '@renderer/pages/settings/GeneralSettings/useDisplayedWorkspacePath'

import { isCloudWorkspace } from './cloudWorkspace'

const COMPILED_BUNDLE = ['wiki']

/** Documents page. The file manager is the same for a local workspace and a cloud drive root. */
export default function DocumentsPage({ corner, root }: { corner?: ReactNode; root?: string } = {}) {
  const { t } = useTranslation()
  const workspaceRoot = useDisplayedWorkspacePath()
  const filesRoot = root || workspaceRoot
  const cloud = Boolean(root && isCloudWorkspace(root))
  const rootLabel = t(cloud ? 'documents.section.shared' : 'documents.section.tree')

  return (
    <div className="flex h-full min-h-0 flex-col bg-white">
      <div className="shrink-0 bg-white">
        <div className="flex flex-col gap-4 px-6 pt-4 pb-4">
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-4">
              <h1 className="m-0 min-w-0 truncate text-[24px] leading-tight font-bold text-foreground">
                {t(cloud ? 'documents.section.drive' : 'documents.title')}
              </h1>
              <div className="flex shrink-0 items-center gap-2">{corner}</div>
            </div>
            <p className="m-0 text-[15px] text-muted-foreground">
              {t(cloud ? 'documents.subtitle.shared' : 'documents.subtitle')}
            </p>
          </div>
        </div>
      </div>
      <div className="flex min-h-0 flex-1">
        <WorkbenchSessionFiles layout="split" root={filesRoot} rootLabel={rootLabel} omitRootNames={COMPILED_BUNDLE} />
      </div>
    </div>
  )
}
