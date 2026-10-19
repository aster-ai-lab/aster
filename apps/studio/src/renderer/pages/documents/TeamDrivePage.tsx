import { useEffect, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { listMemberOrgs, PERSONAL_CENTER, readCenterId, subscribeCenter } from '@renderer/data/memberOrgs'
import { cloudWorkspaceRoot } from '@renderer/pages/documents/cloudWorkspace'
import { TeamPassageSearch } from '@renderer/pages/knowledge/TeamPassageSearch'

import DocumentsPage from './DocumentsPage'

/** 团队侧栏云盘。只列出团队云盘文件，不提供百科切换；右上角检索团队百科段落。 */
export function TeamDrivePage() {
  const { t } = useTranslation()
  const centerId = useSyncExternalStore(subscribeCenter, readCenterId, () => PERSONAL_CENTER)
  const [ready, setReady] = useState(centerId === PERSONAL_CENTER)

  useEffect(() => {
    if (centerId === PERSONAL_CENTER) {
      setReady(true)
      return
    }
    let live = true
    setReady(false)
    void listMemberOrgs()
      .then(() => {
        if (live) setReady(true)
      })
      .catch(() => {
        if (live) setReady(true)
      })
    return () => {
      live = false
    }
  }, [centerId])

  if (!ready || centerId === PERSONAL_CENTER) {
    return (
      <div className="flex h-full min-h-0 flex-col bg-white">
        <div className="shrink-0 bg-white px-6 pt-4 pb-4">
          <h1 className="m-0 truncate text-[24px] leading-tight font-bold text-foreground">
            {t('documents.section.drive')}
          </h1>
        </div>
      </div>
    )
  }

  return <DocumentsPage root={cloudWorkspaceRoot(centerId)} corner={<TeamPassageSearch key={centerId} />} />
}
