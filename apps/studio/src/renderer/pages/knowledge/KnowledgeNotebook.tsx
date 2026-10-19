import type { ReactElement } from 'react'
import { useTranslation } from 'react-i18next'

import { CompiledEncyclopediaPage } from './CompiledEncyclopediaPage'

/** 个人笔记。版式沿用知识页原来的百科栏：左侧页面列表，右侧笔记编辑器。 */
export function KnowledgeNotebook(): ReactElement {
  const { t } = useTranslation()

  return (
    <div className="flex h-full min-h-0 flex-col bg-white" data-ui="knowledge.notebook">
      <header className="shrink-0 px-6 pt-4 pb-2">
        <h1 className="m-0 min-w-0 truncate text-[24px] leading-tight font-bold text-foreground">
          {t('knowledge.notes.title')}
        </h1>
        <p className="m-0 mt-1 text-[15px] text-[#8a8a8a]">{t('knowledge.flowix.subtitle')}</p>
      </header>
      <div className="min-h-0 flex-1">
        <CompiledEncyclopediaPage embedded />
      </div>
    </div>
  )
}
