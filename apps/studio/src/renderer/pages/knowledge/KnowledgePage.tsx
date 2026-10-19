import type { ReactElement } from 'react'

import { KnowledgeNotesPage } from './KnowledgeNotesPage'

/** 知识页只管理本地工作区笔记。团队云盘在自己的页面，不在这里切换。 */
const KnowledgePage = (): ReactElement => {
  return <KnowledgeNotesPage />
}

export default KnowledgePage
